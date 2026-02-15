//! Fixture-driven scoring tests for parity with ts-c2s-api.
//! Reads shared JSON fixtures and validates Rust scoring output matches TS.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use rust_c2s_api::scoring::quality::{
    calculate_lead_quality_score, Address, LeadQualityInput,
};
use rust_c2s_api::scoring::high_value::{
    detect_high_value_lead, HighValueCriteria,
};
use rust_c2s_api::scoring::tier::{
    calculate_tier, CompanyInfo, TierEnrichmentData,
};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/parity/fixtures")
}

/// Decode tri-state income: { "state": "missing" } → None, { "state": "value", "value": N } → Some(N)
fn decode_income(v: &Value) -> Option<f64> {
    match v.get("state").and_then(|s| s.as_str()) {
        Some("missing") => None,
        Some("value") => v.get("value").and_then(|v| v.as_f64()),
        _ => None,
    }
}

fn parse_addresses(v: &Value) -> Vec<Address> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| Address {
                    neighborhood: a.get("neighborhood").and_then(|n| n.as_str()).map(String::from),
                    city: a.get("city").and_then(|c| c.as_str()).map(String::from),
                    state: a.get("state").and_then(|s| s.as_str()).map(String::from),
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── Lead Quality ──

#[test]
fn test_lead_quality_fixtures() {
    let fixture_path = fixtures_dir().join("lead-quality-scoring.json");
    let data: Value = serde_json::from_str(&fs::read_to_string(&fixture_path).unwrap()).unwrap();
    let cases = data["cases"].as_array().unwrap();

    for case in cases {
        let id = case["id"].as_str().unwrap();
        let input = &case["input"];
        let expected = &case["expected"];

        let lead_input = LeadQualityInput {
            name: input.get("name").and_then(|v| v.as_str()).map(String::from),
            phone: input.get("phone").and_then(|v| v.as_str()).map(String::from),
            email: input.get("email").and_then(|v| v.as_str()).map(String::from),
            cpf: input.get("cpf").and_then(|v| v.as_str()).map(String::from),
            enriched_name: input.get("enrichedName").and_then(|v| v.as_str()).map(String::from),
            income: input.get("income").and_then(|v| decode_income(v)),
            presumed_income: input.get("presumedIncome").and_then(|v| decode_income(v)),
            addresses: input.get("addresses").map(|a| parse_addresses(a)).unwrap_or_default(),
            company_count: input.get("companyCount").and_then(|v| v.as_u64()).map(|v| v as u32),
            total_company_capital: input.get("totalCompanyCapital").and_then(|v| v.as_f64()),
            is_company_administrator: input.get("isCompanyAdministrator").and_then(|v| v.as_bool()).unwrap_or(false),
            has_real_estate_sector: input.get("hasRealEstateSector").and_then(|v| v.as_bool()).unwrap_or(false),
        };

        let result = calculate_lead_quality_score(&lead_input);
        let tolerance = expected.get("score_tolerance").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let expected_score = expected["score"].as_u64().unwrap() as u32;

        assert!(
            result.score.abs_diff(expected_score) <= tolerance,
            "Case '{}': score {} != expected {} (tolerance {})",
            id, result.score, expected_score, tolerance
        );

        if let Some(grade) = expected.get("grade").and_then(|v| v.as_str()) {
            assert_eq!(result.grade.as_str(), grade, "Case '{}': grade mismatch", id);
        }

        if let Some(method) = expected.get("scoreMethod").and_then(|v| v.as_str()) {
            assert_eq!(result.score_method.as_str(), method, "Case '{}': scoreMethod mismatch", id);
        }

        if let Some(breakdown) = expected.get("breakdown") {
            if let Some(v) = breakdown.get("dataCompleteness").and_then(|v| v.as_u64()) {
                assert_eq!(result.breakdown.data_completeness, v as u32, "Case '{}': dataCompleteness", id);
            }
            if let Some(v) = breakdown.get("incomeScore").and_then(|v| v.as_u64()) {
                assert_eq!(result.breakdown.income_score, v as u32, "Case '{}': incomeScore", id);
            }
            if let Some(v) = breakdown.get("locationScore").and_then(|v| v.as_u64()) {
                assert_eq!(result.breakdown.location_score, v as u32, "Case '{}': locationScore", id);
            }
            if let Some(v) = breakdown.get("contactValidity").and_then(|v| v.as_u64()) {
                assert_eq!(result.breakdown.contact_validity, v as u32, "Case '{}': contactValidity", id);
            }
            if let Some(v) = breakdown.get("enrichmentBonus").and_then(|v| v.as_u64()) {
                assert_eq!(result.breakdown.enrichment_bonus, v as u32, "Case '{}': enrichmentBonus", id);
            }
        }

        if let Some(flags) = expected.get("flags_include").and_then(|v| v.as_array()) {
            for flag in flags {
                let f = flag.as_str().unwrap();
                assert!(result.flags.contains(&f.to_string()), "Case '{}': missing flag '{}'", id, f);
            }
        }

        if let Some(flags) = expected.get("flags_exclude").and_then(|v| v.as_array()) {
            for flag in flags {
                let f = flag.as_str().unwrap();
                assert!(!result.flags.contains(&f.to_string()), "Case '{}': unexpected flag '{}'", id, f);
            }
        }

        // Emit PARITY_JSON for cross-repo parity check
        println!(
            "PARITY_JSON:{}",
            serde_json::json!({
                "feature": "lead-quality-scoring",
                "case_id": id,
                "score": result.score,
                "grade": result.grade.as_str(),
                "category": result.category.as_str(),
                "scoreMethod": result.score_method.as_str(),
                "breakdown": {
                    "dataCompleteness": result.breakdown.data_completeness,
                    "incomeScore": result.breakdown.income_score,
                    "locationScore": result.breakdown.location_score,
                    "contactValidity": result.breakdown.contact_validity,
                    "enrichmentBonus": result.breakdown.enrichment_bonus
                }
            })
        );
    }
}

// ── High-Value Detector ──

#[test]
fn test_high_value_fixtures() {
    let fixture_path = fixtures_dir().join("high-value-detector.json");
    let data: Value = serde_json::from_str(&fs::read_to_string(&fixture_path).unwrap()).unwrap();
    let cases = data["cases"].as_array().unwrap();

    for case in cases {
        let id = case["id"].as_str().unwrap();
        let input = &case["input"];
        let expected = &case["expected"];

        let criteria = HighValueCriteria {
            income: input.get("income").and_then(|v| decode_income(v)),
            presumed_income: input.get("presumedIncome").and_then(|v| decode_income(v)),
            neighborhood: input.get("neighborhood").and_then(|v| v.as_str()).map(String::from),
            addresses: input.get("addresses").map(|a| parse_addresses(a)).unwrap_or_default(),
            company_count: input.get("companyCount").and_then(|v| v.as_u64()).map(|v| v as u32),
            lead_name: input.get("leadName").and_then(|v| v.as_str()).map(String::from),
            enriched_name: input.get("enrichedName").and_then(|v| v.as_str()).map(String::from),
            property_count: input.get("propertyCount").and_then(|v| v.as_u64()).map(|v| v as u32),
            property_value: input.get("propertyValue").and_then(|v| v.as_f64()),
            net_worth: input.get("netWorth").and_then(|v| v.as_f64()),
            occupation: input.get("occupation").and_then(|v| v.as_str()).map(String::from),
            education: input.get("education").and_then(|v| v.as_str()).map(String::from),
            total_company_capital: input.get("totalCompanyCapital").and_then(|v| v.as_f64()),
            is_company_administrator: input.get("isCompanyAdministrator").and_then(|v| v.as_bool()).unwrap_or(false),
            has_real_estate_sector: input.get("hasRealEstateSector").and_then(|v| v.as_bool()).unwrap_or(false),
        };

        let result = detect_high_value_lead(&criteria);
        let tolerance = expected.get("score_tolerance").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let expected_score = expected["score"].as_i64().unwrap() as i32;

        assert!(
            (result.score - expected_score).abs() <= tolerance,
            "Case '{}': score {} != expected {} (tolerance {})",
            id, result.score, expected_score, tolerance
        );

        if let Some(tier) = expected.get("tier").and_then(|v| v.as_str()) {
            assert_eq!(result.tier.as_str(), tier, "Case '{}': tier mismatch", id);
        }

        if let Some(hv) = expected.get("isHighValue").and_then(|v| v.as_bool()) {
            assert_eq!(result.is_high_value, hv, "Case '{}': isHighValue mismatch", id);
        }

        if let Some(inferred) = expected.get("incomeInferred").and_then(|v| v.as_bool()) {
            let actual = result.details.income_inferred.unwrap_or(false);
            assert_eq!(actual, inferred, "Case '{}': incomeInferred mismatch", id);
        }

        println!(
            "PARITY_JSON:{}",
            serde_json::json!({
                "feature": "high-value-detector",
                "case_id": id,
                "score": result.score,
                "tier": result.tier.as_str(),
                "isHighValue": result.is_high_value,
                "incomeInferred": result.details.income_inferred.unwrap_or(false)
            })
        );
    }
}

// ── Tier Calculator ──

#[test]
fn test_tier_calculator_fixtures() {
    let fixture_path = fixtures_dir().join("tier-calculator.json");
    let data: Value = serde_json::from_str(&fs::read_to_string(&fixture_path).unwrap()).unwrap();
    let cases = data["cases"].as_array().unwrap();

    for case in cases {
        let id = case["id"].as_str().unwrap();
        let input = &case["input"];
        let expected = &case["expected"];

        let name = input["name"].as_str().unwrap();
        let phone = input.get("phone").and_then(|v| v.as_str());
        let email = input.get("email").and_then(|v| v.as_str());

        let enrichment = TierEnrichmentData {
            income: input.get("income").and_then(|v| decode_income(v)),
            addresses: input.get("addresses").map(|a| parse_addresses(a)).unwrap_or_default(),
            property_count: input.get("propertyCount").and_then(|v| v.as_u64()).map(|v| v as u32),
            total_company_capital: input.get("totalCompanyCapital").and_then(|v| v.as_f64()),
            is_company_administrator: input.get("isCompanyAdministrator").and_then(|v| v.as_bool()),
        };

        let companies: Vec<CompanyInfo> = input
            .get("discoveredCompanies")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|c| CompanyInfo {
                        name: c.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let analysis = rust_c2s_api::scoring::tier::TierAnalysisData {
            discovered_companies: companies,
            ..Default::default()
        };

        let has_enrichment = enrichment.income.is_some()
            || !enrichment.addresses.is_empty()
            || enrichment.total_company_capital.is_some()
            || enrichment.is_company_administrator.is_some();
        let has_analysis = !analysis.discovered_companies.is_empty();

        let result = calculate_tier(
            name,
            phone,
            email,
            if has_enrichment { Some(&enrichment) } else { Option::None },
            if has_analysis { Some(&analysis) } else { Option::None },
        );

        let tolerance = expected.get("score_tolerance").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let expected_score = expected["score"].as_i64().unwrap() as i32;

        assert!(
            (result.score - expected_score).abs() <= tolerance,
            "Case '{}': score {} != expected {} (tolerance {})",
            id, result.score, expected_score, tolerance
        );

        if let Some(tier) = expected.get("tier").and_then(|v| v.as_str()) {
            assert_eq!(result.tier.as_str(), tier, "Case '{}': tier mismatch", id);
        }

        if let Some(highlights) = expected.get("highlights_include").and_then(|v| v.as_array()) {
            for h in highlights {
                let expected_h = h.as_str().unwrap();
                assert!(
                    result.highlights.iter().any(|rh| rh.contains(expected_h)),
                    "Case '{}': missing highlight containing '{}'",
                    id, expected_h
                );
            }
        }

        if let Some(highlights) = expected.get("highlights_exclude").and_then(|v| v.as_array()) {
            for h in highlights {
                let excluded = h.as_str().unwrap();
                assert!(
                    !result.highlights.iter().any(|rh| rh.contains(excluded)),
                    "Case '{}': unexpected highlight containing '{}'",
                    id, excluded
                );
            }
        }

        println!(
            "PARITY_JSON:{}",
            serde_json::json!({
                "feature": "tier-calculator",
                "case_id": id,
                "score": result.score,
                "tier": result.tier.as_str()
            })
        );
    }
}
