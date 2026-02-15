//! High-Value Lead Detector.
//! Port of ts:src/utils/high-value-detector.ts

use super::families::analyze_full_name;
use super::neighborhoods::find_noble_neighborhood;
use super::quality::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HighValueTier {
    Platinum,
    Gold,
    Silver,
    None,
}

impl HighValueTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            HighValueTier::Platinum => "platinum",
            HighValueTier::Gold => "gold",
            HighValueTier::Silver => "silver",
            HighValueTier::None => "none",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HighValueCriteria {
    pub income: Option<f64>,
    pub presumed_income: Option<f64>,
    pub neighborhood: Option<String>,
    pub addresses: Vec<Address>,
    pub company_count: Option<u32>,
    pub lead_name: Option<String>,
    pub enriched_name: Option<String>,
    pub property_count: Option<u32>,
    pub property_value: Option<f64>,
    pub net_worth: Option<f64>,
    pub occupation: Option<String>,
    pub education: Option<String>,
    pub total_company_capital: Option<f64>,
    pub is_company_administrator: bool,
    pub has_real_estate_sector: bool,
}

#[derive(Debug, Clone)]
pub struct HighValueDetails {
    pub income: Option<f64>,
    pub neighborhood: Option<String>,
    pub companies: Option<u32>,
    pub family_name: Option<String>,
    pub family_context: Option<String>,
    pub properties: Option<u32>,
    pub property_value: Option<f64>,
    pub net_worth: Option<f64>,
    pub occupation: Option<String>,
    pub total_company_capital: Option<f64>,
    pub income_inferred: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct HighValueResult {
    pub is_high_value: bool,
    pub tier: HighValueTier,
    pub score: i32,
    pub reasons: Vec<String>,
    pub details: HighValueDetails,
}

const EXECUTIVE_KEYWORDS: &[&str] = &[
    "ceo", "diretor", "presidente", "vice-presidente", "sócio",
    "partner", "fundador", "founder", "empresário", "empresaria",
    "chairman", "cfo", "cto", "coo", "cmo",
];

const PROFESSIONAL_KEYWORDS: &[&str] = &[
    "médico", "medico", "advogado", "engenheiro", "arquiteto",
    "dentista", "cirurgião", "cirurgiao", "juiz", "desembargador",
    "promotor", "procurador", "investidor", "banker", "private banker",
];

pub fn detect_high_value_lead(criteria: &HighValueCriteria) -> HighValueResult {
    let mut reasons: Vec<String> = Vec::new();
    let mut details = HighValueDetails {
        income: None,
        neighborhood: None,
        companies: None,
        family_name: None,
        family_context: None,
        properties: None,
        property_value: None,
        net_worth: None,
        occupation: None,
        total_company_capital: None,
        income_inferred: None,
    };
    let mut score: i32 = 0;

    // Income scoring
    let has_income_data = criteria.income.is_some() || criteria.presumed_income.is_some();
    let income = criteria.income.or(criteria.presumed_income);
    if let Some(inc) = income {
        details.income = Some(inc);
        if inc >= 20_000.0 {
            score += 50;
            reasons.push(format!("Renda muito alta: R$ {:.0}/mês", inc));
        } else if inc >= 15_000.0 {
            score += 36;
            reasons.push(format!("Renda alta: R$ {:.0}/mês", inc));
        } else if inc >= 10_000.0 {
            score += 10;
        }
    }

    // Neighborhood
    let mut noble: Option<&str> = None;
    if let Some(n) = &criteria.neighborhood {
        noble = find_noble_neighborhood(n);
    }
    if noble.is_none() {
        for addr in &criteria.addresses {
            if let Some(n) = &addr.neighborhood {
                noble = find_noble_neighborhood(n);
                if noble.is_some() {
                    break;
                }
            }
        }
    }
    if let Some(n) = noble {
        score += 15;
        let cap: String = n.split_whitespace()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().to_string() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        reasons.push(format!("Bairro nobre: {}", cap));
        details.neighborhood = Some(cap);
    }

    // Companies (3+)
    if let Some(count) = criteria.company_count {
        if count >= 3 {
            score += 20;
            reasons.push(format!("{} empresas ativas", count));
            details.companies = Some(count);
        }
    }

    // Surname analysis
    let name_to_analyze = criteria.enriched_name.as_ref().or(criteria.lead_name.as_ref());
    if let Some(name) = name_to_analyze {
        for analysis in analyze_full_name(name) {
            if analysis.is_notable_family {
                if let Some(ctx) = &analysis.family_context {
                    score += 50;
                    reasons.push(format!("Família notável: {}", ctx));
                    details.family_name = Some(analysis.surname.clone());
                    details.family_context = Some(ctx.clone());
                    break;
                }
            } else if analysis.is_rare && analysis.confidence >= 80 {
                score += 10;
                reasons.push(format!("Sobrenome raro: {}", analysis.surname));
                details.family_name = Some(analysis.surname.clone());
                break;
            }
        }
    }

    // Properties
    if let Some(count) = criteria.property_count {
        if count >= 2 {
            score += 15;
            reasons.push(format!("{} imóveis no cadastro", count));
            details.properties = Some(count);
        }
    }

    if let Some(value) = criteria.property_value {
        if value >= 5_000_000.0 {
            score += 40;
        } else if value >= 2_000_000.0 {
            score += 25;
        }
        if value >= 2_000_000.0 {
            reasons.push(format!("Patrimônio imobiliário: R$ {:.0}", value));
            details.property_value = Some(value);
        }
    }

    // Net worth
    if let Some(nw) = criteria.net_worth {
        if nw >= 5_000_000.0 {
            score += 45;
        } else if nw >= 1_000_000.0 {
            score += 30;
        }
        if nw >= 1_000_000.0 {
            reasons.push(format!("Patrimônio líquido: R$ {:.0}", nw));
            details.net_worth = Some(nw);
        }
    }

    // Occupation
    if let Some(occ) = &criteria.occupation {
        let lower = occ.to_lowercase();
        if EXECUTIVE_KEYWORDS.iter().any(|k| lower.contains(k)) {
            score += 15;
            reasons.push(format!("Cargo executivo: {}", occ));
            details.occupation = Some(occ.clone());
        } else if PROFESSIONAL_KEYWORDS.iter().any(|k| lower.contains(k)) {
            score += 10;
            reasons.push(format!("Profissão de alto valor: {}", occ));
            details.occupation = Some(occ.clone());
        }
    }

    // Education
    if let Some(edu) = &criteria.education {
        let lower = edu.to_lowercase();
        let advanced = ["pós", "pos", "mestrado", "doutorado", "mba", "especialização", "especializacao"];
        if advanced.iter().any(|k| lower.contains(k)) {
            score += 5;
        }
    }

    // Company capital
    if let Some(capital) = criteria.total_company_capital {
        if capital >= 5_000_000.0 {
            score += 40;
        } else if capital >= 1_000_000.0 {
            score += 25;
        } else if capital >= 500_000.0 {
            score += 15;
        }
        if capital >= 500_000.0 {
            reasons.push(format!("Empresário - Capital social: R$ {:.0}", capital));
            details.total_company_capital = Some(capital);
        }
    }

    if criteria.is_company_administrator {
        score += 10;
    }

    // Missing-income adjustment
    if !has_income_data && score >= 25 {
        let signal_categories = [
            noble.is_some(),
            criteria.company_count.unwrap_or(0) >= 2,
            criteria.property_value.unwrap_or(0.0) >= 2_000_000.0,
            criteria.net_worth.unwrap_or(0.0) >= 1_000_000.0,
        ]
        .iter()
        .filter(|&&b| b)
        .count() as i32;

        if signal_categories >= 2 {
            let adjustment = (signal_categories * 5).min(15);
            score += adjustment;
            reasons.push(format!(
                "Renda provável ({} indicadores independentes, +{} pts)",
                signal_categories, adjustment
            ));
            details.income_inferred = Some(true);
        }
    }

    let tier = if score >= 60 {
        HighValueTier::Platinum
    } else if score >= 50 {
        HighValueTier::Gold
    } else if score >= 25 {
        HighValueTier::Silver
    } else {
        HighValueTier::None
    };

    HighValueResult {
        is_high_value: score >= 50,
        tier,
        score,
        reasons,
        details,
    }
}
