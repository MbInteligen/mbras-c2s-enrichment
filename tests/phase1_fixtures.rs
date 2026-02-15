//! Phase 1 fixture-driven tests — CPF validation, name matching, retry policy.
//!
//! Reads shared fixtures from docs/parity/fixtures/phase1-cpf-discovery.json
//! and emits PARITY_JSON: lines for cross-repo comparison.

use rust_c2s_api::cpf::{is_valid_cpf, normalize_cpf};
use rust_c2s_api::name_matcher::match_names;
use serde::Deserialize;
use serde_json::Value;
use std::fs;

#[derive(Deserialize)]
struct Fixtures {
    groups: Groups,
}

#[derive(Deserialize)]
struct Groups {
    cpf_mod11: CaseGroup,
    name_matching: CaseGroup,
    retry_policy: CaseGroup,
}

#[derive(Deserialize)]
struct CaseGroup {
    cases: Vec<Value>,
}

fn load_fixtures() -> Fixtures {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/parity/fixtures/phase1-cpf-discovery.json"
    );
    let data = fs::read_to_string(path).expect("Failed to read fixtures");
    serde_json::from_str(&data).expect("Failed to parse fixtures")
}

// --- CPF mod-11 validation ---

#[test]
fn test_cpf_mod11_fixtures() {
    let fixtures = load_fixtures();

    for case in &fixtures.groups.cpf_mod11.cases {
        let id = case["id"].as_str().unwrap();
        let raw = case["input"]["cpf"].as_str().unwrap();
        let expected_valid = case["expected"]["valid"].as_bool().unwrap();

        let normalized = normalize_cpf(raw);

        // Test normalization if expected
        if let Some(exp_norm) = case["expected"]["normalized"].as_str() {
            assert_eq!(
                normalized, exp_norm,
                "case {}: normalize_cpf({:?}) = {:?}, expected {:?}",
                id, raw, normalized, exp_norm
            );
        }

        // For CNPJ-length inputs, test raw digits
        let valid = if raw.chars().filter(|c| c.is_ascii_digit()).count() == 14
            && id == "invalid-cnpj-length"
        {
            let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
            is_valid_cpf(&digits)
        } else {
            is_valid_cpf(&normalized)
        };

        assert_eq!(
            valid, expected_valid,
            "case {}: is_valid_cpf({:?}) = {}, expected {}",
            id, normalized, valid, expected_valid
        );

        // Emit parity JSON
        println!(
            "PARITY_JSON:{}",
            serde_json::json!({
                "case_id": id,
                "input": raw,
                "normalized": normalized,
                "valid": valid,
            })
        );
    }
}

// --- Name matching ---

#[test]
fn test_name_matching_fixtures() {
    let fixtures = load_fixtures();

    for case in &fixtures.groups.name_matching.cases {
        let id = case["id"].as_str().unwrap();
        let lead_name = case["input"]["leadName"].as_str().unwrap();
        let db_name = case["input"]["dbName"].as_str().unwrap();

        let result = match_names(lead_name, db_name);

        let expected_matches = case["expected"]["matches"].as_bool().unwrap();
        assert_eq!(
            result.matches, expected_matches,
            "case {}: matches = {}, expected {}",
            id, result.matches, expected_matches
        );

        // Exact score check
        if let Some(exp_score) = case["expected"]["score"].as_f64() {
            assert!(
                (result.score - exp_score).abs() < 0.01,
                "case {}: score = {:.4}, expected {:.4}",
                id,
                result.score,
                exp_score
            );
        }

        // Minimum score bound
        if let Some(score_min) = case["expected"]["score_min"].as_f64() {
            assert!(
                result.score >= score_min,
                "case {}: score = {:.4}, expected >= {:.4}",
                id,
                result.score,
                score_min
            );
        }

        // Maximum score bound
        if let Some(score_max) = case["expected"]["score_max"].as_f64() {
            assert!(
                result.score <= score_max,
                "case {}: score = {:.4}, expected <= {:.4}",
                id,
                result.score,
                score_max
            );
        }

        // Method check
        if let Some(exp_method) = case["expected"]["method"].as_str() {
            assert_eq!(
                result.method, exp_method,
                "case {}: method = {:?}, expected {:?}",
                id, result.method, exp_method
            );
        }

        // Method one-of check
        if let Some(methods) = case["expected"]["method_oneof"].as_array() {
            let valid_methods: Vec<&str> = methods.iter().map(|v| v.as_str().unwrap()).collect();
            assert!(
                valid_methods.contains(&result.method.as_str()),
                "case {}: method = {:?}, expected one of {:?}",
                id,
                result.method,
                valid_methods
            );
        }

        // Emit parity JSON
        println!(
            "PARITY_JSON:{}",
            serde_json::json!({
                "case_id": id,
                "matches": result.matches,
                "score": format!("{:.4}", result.score).parse::<f64>().unwrap(),
                "method": result.method,
            })
        );
    }
}

// --- Retry policy ---

const RETRYABLE_STATUSES: &[&str] = &["partial", "unenriched", "basic"];
const MAX_RETRY_ATTEMPTS: u32 = 5;
const RETRY_DELAYS_HOURS: &[u32] = &[1, 2, 4, 8, 16];

fn get_retry_delay_hours(retry_count: u32) -> u32 {
    if retry_count as usize >= RETRY_DELAYS_HOURS.len() {
        *RETRY_DELAYS_HOURS.last().unwrap()
    } else {
        RETRY_DELAYS_HOURS[retry_count as usize]
    }
}

#[derive(Debug)]
struct RetryResult {
    eligible: bool,
    delay_hours: Option<u32>,
    reason: Option<String>,
}

fn is_retry_eligible(
    status: &str,
    retry_count: u32,
    last_retry_at: Option<&str>,
    now: Option<&str>,
) -> RetryResult {
    if !RETRYABLE_STATUSES.contains(&status) {
        return RetryResult {
            eligible: false,
            delay_hours: None,
            reason: Some("status_not_retryable".to_string()),
        };
    }
    if retry_count >= MAX_RETRY_ATTEMPTS {
        return RetryResult {
            eligible: false,
            delay_hours: None,
            reason: Some("max_retries_exceeded".to_string()),
        };
    }
    let delay_hours = get_retry_delay_hours(retry_count);
    if let Some(last_str) = last_retry_at {
        let last = chrono::DateTime::parse_from_rfc3339(last_str)
            .expect("parse last_retry_at")
            .with_timezone(&chrono::Utc);
        let now_dt = if let Some(now_str) = now {
            chrono::DateTime::parse_from_rfc3339(now_str)
                .expect("parse now")
                .with_timezone(&chrono::Utc)
        } else {
            chrono::Utc::now()
        };
        let elapsed = now_dt - last;
        let required = chrono::Duration::hours(delay_hours as i64);
        if elapsed < required {
            return RetryResult {
                eligible: false,
                delay_hours: None,
                reason: Some("backoff_not_elapsed".to_string()),
            };
        }
    }
    RetryResult {
        eligible: true,
        delay_hours: Some(delay_hours),
        reason: None,
    }
}

#[test]
fn test_retry_policy_fixtures() {
    let fixtures = load_fixtures();

    for case in &fixtures.groups.retry_policy.cases {
        let id = case["id"].as_str().unwrap();
        let status = case["input"]["status"].as_str().unwrap();
        let retry_count = case["input"]["retry_count"].as_u64().unwrap() as u32;
        let last_retry_at = case["input"]["last_retry_at"].as_str();
        let now = case["input"]["now"].as_str();

        let result = is_retry_eligible(status, retry_count, last_retry_at, now);

        let expected_eligible = case["expected"]["eligible"].as_bool().unwrap();
        assert_eq!(
            result.eligible, expected_eligible,
            "case {}: eligible = {}, expected {}",
            id, result.eligible, expected_eligible
        );

        if let Some(exp_reason) = case["expected"]["reason"].as_str() {
            assert_eq!(
                result.reason.as_deref(),
                Some(exp_reason),
                "case {}: reason = {:?}, expected {:?}",
                id,
                result.reason,
                exp_reason
            );
        }

        if let Some(exp_delay) = case["expected"]["delay_hours"].as_u64() {
            if result.eligible {
                assert_eq!(
                    result.delay_hours,
                    Some(exp_delay as u32),
                    "case {}: delay_hours = {:?}, expected {}",
                    id,
                    result.delay_hours,
                    exp_delay
                );
            }
        }

        // Emit parity JSON
        println!(
            "PARITY_JSON:{}",
            serde_json::json!({
                "case_id": id,
                "eligible": result.eligible,
                "delay_hours": result.delay_hours,
                "reason": result.reason,
            })
        );
    }
}
