//! Lead Quality Score (0-100).
//! Port of ts:src/services/lead-quality.service.ts

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::neighborhoods::find_noble_neighborhood;

/// Valid Brazilian DDDs.
static VALID_DDDS: LazyLock<HashSet<u32>> = LazyLock::new(|| {
    [
        11, 12, 13, 14, 15, 16, 17, 18, 19, // SP
        21, 22, 24, // RJ
        27, 28, // ES
        31, 32, 33, 34, 35, 37, 38, // MG
        41, 42, 43, 44, 45, 46, // PR
        47, 48, 49, // SC
        51, 53, 54, 55, // RS
        61, // DF
        62, 64, // GO
        63, // TO
        65, 66, // MT
        67, // MS
        68, // AC
        69, // RO
        71, 73, 74, 75, 77, // BA
        79, // SE
        81, 82, 83, 84, 85, 86, 87, 88, 89, // NE
        91, 92, 93, 94, 95, 96, 97, 98, 99, // Norte
    ]
    .into_iter()
    .collect()
});

static SPAM_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)painel\s*fama",
        r"(?i)sucesso\s*com\s*vendas",
        r"(?i)ganhe\s*dinheiro",
        r"(?i)renda\s*extra",
        r"(?i)trabalhe\s*em\s*casa",
        r"(?i)marketing\s*digital",
        r"(?i)afiliado",
        r"(?i)curso\s*online",
        r"(?i)investimento",
        r"(?i)cripto",
        r"(?i)bitcoin",
        r"(?i)forex",
        r"(?i)teste\s*teste",
        r"(?i)^teste$",
        r"(?i)^test$",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grade {
    A,
    B,
    C,
    D,
    F,
}

impl Grade {
    pub fn as_str(&self) -> &'static str {
        match self {
            Grade::A => "A",
            Grade::B => "B",
            Grade::C => "C",
            Grade::D => "D",
            Grade::F => "F",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Category {
    Premium,
    High,
    Standard,
    Low,
    Poor,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Premium => "premium",
            Category::High => "high",
            Category::Standard => "standard",
            Category::Low => "low",
            Category::Poor => "poor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScoreMethod {
    Direct,
    Inferred,
    None,
}

impl ScoreMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScoreMethod::Direct => "direct",
            ScoreMethod::Inferred => "inferred",
            ScoreMethod::None => "none",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Address {
    pub neighborhood: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LeadQualityInput {
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub cpf: Option<String>,
    pub enriched_name: Option<String>,
    pub income: Option<f64>,
    pub presumed_income: Option<f64>,
    pub addresses: Vec<Address>,
    pub company_count: Option<u32>,
    pub total_company_capital: Option<f64>,
    pub is_company_administrator: bool,
    pub has_real_estate_sector: bool,
}

#[derive(Debug, Clone)]
pub struct Breakdown {
    pub data_completeness: u32,
    pub income_score: u32,
    pub location_score: u32,
    pub contact_validity: u32,
    pub enrichment_bonus: u32,
}

#[derive(Debug, Clone)]
pub struct LeadQualityResult {
    pub score: u32,
    pub grade: Grade,
    pub category: Category,
    pub score_method: ScoreMethod,
    pub breakdown: Breakdown,
    pub flags: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Calculate lead quality score (0-100).
pub fn calculate_lead_quality_score(input: &LeadQualityInput) -> LeadQualityResult {
    let mut breakdown = Breakdown {
        data_completeness: 0,
        income_score: 0,
        location_score: 0,
        contact_validity: 0,
        enrichment_bonus: 0,
    };
    let mut flags: Vec<String> = Vec::new();
    let mut recommendations: Vec<String> = Vec::new();

    // Check for spam
    if let Some(name) = &input.name {
        if SPAM_PATTERNS.iter().any(|p| p.is_match(name)) {
            flags.push("spam_detected".into());
            return LeadQualityResult {
                score: 0,
                grade: Grade::F,
                category: Category::Poor,
                score_method: ScoreMethod::None,
                breakdown,
                flags,
                recommendations: vec!["Lead appears to be spam/bot - do not contact".into()],
            };
        }
    }

    // 1. Data Completeness (max 30)
    // Name quality (0-10)
    let name_str = input.enriched_name.as_ref().or(input.name.as_ref());
    if let Some(name) = name_str {
        let parts: Vec<&str> = name.trim().split_whitespace().collect();
        if parts.len() >= 3 && name.len() >= 10 {
            breakdown.data_completeness += 10;
        } else if parts.len() >= 2 && name.len() >= 5 {
            breakdown.data_completeness += 7;
        } else if name.len() >= 3 {
            breakdown.data_completeness += 3;
        }
    } else {
        flags.push("missing_name".into());
        recommendations.push("Missing customer name".into());
    }

    // Phone (0-10)
    if let Some(phone) = &input.phone {
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 10 && digits.len() <= 13 {
            let ddd = digits[..2].parse::<u32>().unwrap_or(0);
            if VALID_DDDS.contains(&ddd) {
                breakdown.data_completeness += 10;
            } else {
                breakdown.data_completeness += 3;
                flags.push("invalid_ddd".into());
            }
        } else if digits.len() >= 8 {
            breakdown.data_completeness += 5;
            flags.push("short_phone".into());
        }
    } else {
        flags.push("missing_phone".into());
        recommendations.push("No phone number provided".into());
    }

    // Email (0-5)
    if let Some(email) = &input.email {
        if email.contains('@') && email.contains('.') {
            breakdown.data_completeness += 5;
        } else {
            breakdown.data_completeness += 2;
            flags.push("invalid_email_format".into());
        }
    }

    // CPF (0-5)
    if input.cpf.is_some() {
        breakdown.data_completeness += 5;
    }

    // 2. Income Score (max 25)
    let has_income_data = input.income.is_some();
    let has_presumed_data = input.presumed_income.is_some();
    let effective_income = input.income.or(input.presumed_income);
    let mut score_method = ScoreMethod::None;

    if let Some(inc) = effective_income {
        if has_income_data || has_presumed_data {
            score_method = ScoreMethod::Direct;
            breakdown.income_score = if inc >= 20_000.0 {
                25
            } else if inc >= 15_000.0 {
                20
            } else if inc >= 10_000.0 {
                15
            } else if inc >= 5_000.0 {
                10
            } else if inc >= 3_000.0 {
                5
            } else {
                0
            };
        }
    } else {
        // Inferred income from proxy signals
        let mut inferred = 0u32;

        if let Some(capital) = input.total_company_capital {
            if capital >= 5_000_000.0 {
                inferred += 15;
            } else if capital >= 1_000_000.0 {
                inferred += 10;
            } else if capital >= 100_000.0 {
                inferred += 5;
            }
        }

        if input.is_company_administrator {
            inferred += 5;
        }
        if input.has_real_estate_sector {
            inferred += 5;
        }

        breakdown.income_score = inferred.min(25);
        if breakdown.income_score > 0 {
            score_method = ScoreMethod::Inferred;
            flags.push("inferred_income".into());
        }
    }

    // 3. Location Score (max 15)
    if !input.addresses.is_empty() {
        let mut best = 0u32;
        for addr in &input.addresses {
            let mut addr_score = 5u32;
            if let Some(neighborhood) = &addr.neighborhood {
                if find_noble_neighborhood(neighborhood).is_some() {
                    addr_score = 15;
                    if !flags.contains(&"noble_neighborhood".to_string()) {
                        flags.push("noble_neighborhood".into());
                    }
                } else {
                    addr_score = 8;
                }
            }
            // SP/RJ capital bonus
            if let Some(city) = &addr.city {
                let lower = city.to_lowercase();
                if lower.contains("são paulo") || lower.contains("rio de janeiro") {
                    addr_score = addr_score.saturating_add(2).min(15);
                }
            }
            best = best.max(addr_score);
        }
        breakdown.location_score = best;
    }

    // 4. Contact Validity (max 20)
    if let Some(phone) = &input.phone {
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 2 {
            let ddd = digits[..2].parse::<u32>().unwrap_or(0);
            let has_nine_digit = digits.len() >= 11 && digits.as_bytes().get(2) == Some(&b'9');
            if VALID_DDDS.contains(&ddd) && has_nine_digit {
                breakdown.contact_validity += 15;
            } else if VALID_DDDS.contains(&ddd) {
                breakdown.contact_validity += 10;
            }
        }
    }

    if let Some(email) = &input.email {
        if let Some(domain) = email.split('@').nth(1) {
            let domain_lower = domain.to_lowercase();
            let premium = ["gmail.com", "outlook.com", "hotmail.com", "yahoo.com", "icloud.com"];
            let corporate = [".com.br", ".com", ".net", ".org"];
            if premium.contains(&domain_lower.as_str()) {
                breakdown.contact_validity += 5;
            } else if corporate.iter().any(|d| domain_lower.ends_with(d)) {
                breakdown.contact_validity += 3;
            }
        }
    }

    // 5. Enrichment Bonus (max 10)
    if input.cpf.is_some() && input.enriched_name.is_some() {
        breakdown.enrichment_bonus += 5;
    }
    if let Some(count) = input.company_count {
        if count >= 1 {
            breakdown.enrichment_bonus += 3;
            if count >= 3 {
                breakdown.enrichment_bonus += 2;
                flags.push("multiple_companies".into());
            }
        }
    }

    // Calculate total
    let score = (breakdown.data_completeness
        + breakdown.income_score
        + breakdown.location_score
        + breakdown.contact_validity
        + breakdown.enrichment_bonus)
        .min(100);

    let (grade, category) = if score >= 90 {
        (Grade::A, Category::Premium)
    } else if score >= 70 {
        (Grade::B, Category::High)
    } else if score >= 50 {
        (Grade::C, Category::Standard)
    } else if score >= 30 {
        (Grade::D, Category::Low)
    } else {
        (Grade::F, Category::Poor)
    };

    // Recommendations
    if input.cpf.is_none() && input.phone.is_some() {
        recommendations.push("Try CPF discovery via phone".into());
    }
    if input.email.is_none() && score < 70 {
        recommendations.push("Request email for follow-up".into());
    }
    if effective_income.is_none() && input.cpf.is_some() {
        recommendations.push("Enrich to get income data".into());
    }

    LeadQualityResult {
        score,
        grade,
        category,
        score_method,
        breakdown,
        flags,
        recommendations,
    }
}
