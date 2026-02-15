//! Tier Calculator — multi-factor lead classification.
//! Port of ts:src/services/tier-calculator.service.ts

use std::collections::HashSet;
use std::sync::LazyLock;

use super::families::analyze_full_name;
use super::neighborhoods::is_noble_neighborhood;
use super::quality::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TierLevel {
    Platinum,
    Gold,
    Silver,
    Bronze,
    Risk,
}

impl TierLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TierLevel::Platinum => "platinum",
            TierLevel::Gold => "gold",
            TierLevel::Silver => "silver",
            TierLevel::Bronze => "bronze",
            TierLevel::Risk => "risk",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TierLevel::Platinum => "Platinum",
            TierLevel::Gold => "Gold",
            TierLevel::Silver => "Silver",
            TierLevel::Bronze => "Bronze",
            TierLevel::Risk => "Alto Risco",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Default)]
pub struct CompanyInfo {
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct TierEnrichmentData {
    pub income: Option<f64>,
    pub addresses: Vec<Address>,
    pub property_count: Option<u32>,
    pub total_company_capital: Option<f64>,
    pub is_company_administrator: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct TierAnalysisData {
    pub risk_level: Option<RiskLevel>,
    pub discovered_companies: Vec<CompanyInfo>,
    pub domain_sector: Option<String>,
    pub person_role: Option<String>,
    pub person_education: Option<String>,
    pub company_sector: Option<String>,
}

pub struct TierResult {
    pub tier: TierLevel,
    pub tier_label: String,
    pub score: i32,
    pub highlights: Vec<String>,
    pub recommendation_action: String,
    pub recommendation_title: String,
    pub recommendation_description: String,
}

// Score weights
const W_HIGH_INCOME: i32 = 15;
const W_VERY_HIGH_INCOME: i32 = 25;
const W_MANAGED_CAPITAL: i32 = 35;
const W_HIGH_VALUE_SECTOR: i32 = 15;
const W_HIGH_VALUE_ROLE: i32 = 15;
const W_BUSINESS_OWNER: i32 = 10;
const W_MULTIPLE_COMPANIES: i32 = 10;
const W_ELITE_EDUCATION: i32 = 20;
const W_BRAZILIAN_ELITE: i32 = 10;
const W_NOBLE_NEIGHBORHOOD: i32 = 15;
const W_NOTABLE_FAMILY: i32 = 25;
const W_RARE_SURNAME: i32 = 10;
const W_INTERNATIONAL: i32 = 10;
const W_MULTIPLE_PROPERTIES: i32 = 5;
const W_RISK_LOW: i32 = -10;
const W_RISK_MEDIUM: i32 = -30;
const W_RISK_HIGH: i32 = -50;
const W_RISK_CRITICAL: i32 = -100;

static ELITE_EDUCATION: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "harvard", "stanford", "mit", "yale", "princeton", "columbia",
        "wharton", "insead", "london business school", "oxford", "cambridge",
        "hbs", "gsb",
    ].into_iter().collect()
});

static BRAZILIAN_ELITE_EDUCATION: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "usp", "fgv", "insper", "puc", "unicamp", "fea", "poli", "fea-usp", "ibmec",
    ].into_iter().collect()
});

static HIGH_VALUE_SECTORS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "venture capital", "private equity", "banco", "banco de investimentos",
        "investimentos", "fintech", "tecnologia", "imobiliário",
    ].into_iter().collect()
});

static HIGH_VALUE_ROLES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "ceo", "cfo", "coo", "cto", "fundador", "co-fundador", "founder",
        "co-founder", "sócio", "partner", "managing partner", "diretor",
        "presidente", "vice-presidente", "vp",
    ].into_iter().collect()
});

/// Calculate tier for a lead.
pub fn calculate_tier(
    name: &str,
    phone: Option<&str>,
    _email: Option<&str>,
    enrichment: Option<&TierEnrichmentData>,
    analysis: Option<&TierAnalysisData>,
) -> TierResult {
    let mut highlights: Vec<String> = Vec::new();
    let mut score: i32 = 0;

    // 1. Surname analysis
    let surname_analysis = analyze_full_name(name);
    for a in &surname_analysis {
        if a.is_notable_family {
            if let Some(ctx) = &a.family_context {
                score += W_NOTABLE_FAMILY;
                highlights.push(format!("Família notável: {}", ctx));
            }
        } else if a.is_rare && a.confidence > 60 {
            score += W_RARE_SURNAME;
            highlights.push(format!("Sobrenome raro: {}", a.surname));
        }
    }

    // 2. International phone
    if let Some(ph) = phone {
        if is_international_phone(ph) {
            score += W_INTERNATIONAL;
            highlights.push("Lead internacional".into());
        }
    }

    // 3. Income
    let _has_income_data = enrichment
        .map(|e| e.income.is_some())
        .unwrap_or(false);
    if let Some(enr) = enrichment {
        if let Some(income) = enr.income {
            if income >= 30_000.0 {
                score += W_VERY_HIGH_INCOME;
                highlights.push(format!("Renda muito alta: R$ {:.0}/mês", income));
            } else if income >= 15_000.0 {
                score += W_HIGH_INCOME;
                highlights.push(format!("Renda alta: R$ {:.0}/mês", income));
            }
        } else {
            // Inferred financial
            let mut inferred = 0i32;
            if let Some(capital) = enr.total_company_capital {
                if capital >= 5_000_000.0 {
                    inferred += 20;
                } else if capital >= 1_000_000.0 {
                    inferred += 15;
                } else if capital >= 100_000.0 {
                    inferred += 5;
                }
            }
            if enr.is_company_administrator.unwrap_or(false) {
                inferred += 5;
            }
            inferred = inferred.min(W_VERY_HIGH_INCOME);
            if inferred > 0 {
                score += inferred;
                highlights.push(format!(
                    "Perfil financeiro inferido ({} pts, renda não disponível)",
                    inferred
                ));
            }
        }
    }

    // 4. Neighborhood
    if let Some(enr) = enrichment {
        for addr in &enr.addresses {
            if let Some(n) = &addr.neighborhood {
                if is_noble_neighborhood(n) {
                    score += W_NOBLE_NEIGHBORHOOD;
                    highlights.push(format!("Bairro nobre: {}", n));
                    break;
                }
            }
        }
    }

    // 5. Properties
    if let Some(enr) = enrichment {
        if let Some(count) = enr.property_count {
            if count > 2 {
                score += W_MULTIPLE_PROPERTIES;
                highlights.push(format!("{} imóveis registrados", count));
            }
        }
    }

    // 6. Domain/company sector
    if let Some(an) = analysis {
        if let Some(sector) = &an.domain_sector {
            if HIGH_VALUE_SECTORS.contains(sector.to_lowercase().as_str()) {
                score += W_HIGH_VALUE_SECTOR;
                highlights.push(format!("Setor de alto valor: {}", sector));
            }
        }
    }

    // 7. Person info
    if let Some(an) = analysis {
        if let Some(role) = &an.person_role {
            if HIGH_VALUE_ROLES.contains(role.to_lowercase().as_str()) {
                score += W_HIGH_VALUE_ROLE;
                highlights.push(format!("Cargo de alto valor: {}", role));
            }
        }
        if let Some(edu) = &an.person_education {
            let lower = edu.to_lowercase();
            if ELITE_EDUCATION.iter().any(|e| lower.contains(e)) {
                score += W_ELITE_EDUCATION;
                highlights.push(format!("Formação de elite: {}", edu));
            } else if BRAZILIAN_ELITE_EDUCATION.iter().any(|e| lower.contains(e)) {
                score += W_BRAZILIAN_ELITE;
                highlights.push(format!("Formação de destaque: {}", edu));
            }
        }
    }

    // 8. Discovered companies
    if let Some(an) = analysis {
        let count = an.discovered_companies.len();
        if count >= 2 {
            score += W_BUSINESS_OWNER + W_MULTIPLE_COMPANIES;
            highlights.push(format!("Sócio de {} empresas", count));
        } else if count == 1 {
            score += W_BUSINESS_OWNER;
            highlights.push(format!("Empresário: {}", an.discovered_companies[0].name));
        }
    }

    // 9. Managed capital
    if let Some(an) = analysis {
        if let Some(sector) = &an.company_sector {
            let lower = sector.to_lowercase();
            if lower.contains("capital") || lower.contains("venture") || lower.contains("private equity") {
                score += W_MANAGED_CAPITAL;
                highlights.push("Gestor de capital/investimentos".into());
            }
        }
    }

    // 10. Risk adjustments
    let risk_level = analysis.and_then(|a| a.risk_level.as_ref());
    if let Some(risk) = risk_level {
        match risk {
            RiskLevel::Critical => score += W_RISK_CRITICAL,
            RiskLevel::High => score += W_RISK_HIGH,
            RiskLevel::Medium => score += W_RISK_MEDIUM,
            RiskLevel::Low => score += W_RISK_LOW,
            RiskLevel::None => {}
        }
    }

    // Calculate tier
    let tier = get_tier_from_score(score, risk_level);
    let final_score = score.max(0).min(100);

    let (action, title, description) = get_recommendation(&tier);

    TierResult {
        tier_label: tier.label().to_string(),
        tier,
        score: final_score,
        highlights,
        recommendation_action: action.to_string(),
        recommendation_title: title.to_string(),
        recommendation_description: description.to_string(),
    }
}

fn get_tier_from_score(score: i32, risk_level: Option<&RiskLevel>) -> TierLevel {
    if matches!(risk_level, Some(RiskLevel::Critical | RiskLevel::High)) {
        return TierLevel::Risk;
    }
    if score >= 70 {
        TierLevel::Platinum
    } else if score >= 50 {
        TierLevel::Gold
    } else if score >= 30 {
        TierLevel::Silver
    } else if score < 0 || matches!(risk_level, Some(RiskLevel::Medium)) {
        TierLevel::Risk
    } else {
        TierLevel::Bronze
    }
}

fn get_recommendation(tier: &TierLevel) -> (&'static str, &'static str, &'static str) {
    match tier {
        TierLevel::Platinum => ("priority", "Prioridade Máxima", "Lead de altíssimo valor. Abordagem premium e personalizada recomendada."),
        TierLevel::Gold => ("priority", "Alta Prioridade", "Lead de alto valor. Contato prioritário com abordagem personalizada recomendada."),
        TierLevel::Silver => ("qualify", "Qualificar", "Lead com potencial. Necessário qualificar interesse e capacidade antes de prosseguir."),
        TierLevel::Bronze => ("contact", "Contatar", "Lead padrão. Seguir processo normal de contato e qualificação."),
        TierLevel::Risk => ("avoid", "Evitar", "Lead com alto risco. Não recomendado prosseguir sem análise adicional."),
    }
}

/// Simple international phone check (Brazilian DDDs are domestic).
fn is_international_phone(phone: &str) -> bool {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

    // Brazilian with country code 55
    if digits.starts_with("55") && (digits.len() == 12 || digits.len() == 13) {
        return false;
    }

    // Brazilian without country code (valid DDD range)
    if (digits.len() == 10 || digits.len() == 11) && digits.len() >= 2 {
        let ddd = digits[..2].parse::<u32>().unwrap_or(0);
        if (11..=99).contains(&ddd) {
            return false;
        }
    }

    // Too short or too long → assume international
    if digits.len() > 13 || digits.len() < 10 {
        return true;
    }

    false
}
