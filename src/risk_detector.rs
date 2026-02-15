//! Risk detector service — known risks database + web news scanning.
//!
//! Ported from ts:src/services/risk-detector.service.ts
//!
//! Categories: criminal, investigation, financial, reputation, legal
//! Severity: low, medium, high, critical

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::web_search::WebSearchService;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCategory {
    Criminal,
    Investigation,
    Financial,
    Reputation,
    Legal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskSeverity {
    fn multiplier(&self) -> f64 {
        match self {
            Self::Low => 0.5,
            Self::Medium => 1.0,
            Self::High => 1.5,
            Self::Critical => 2.0,
        }
    }
}

impl RiskCategory {
    fn weight(&self) -> f64 {
        match self {
            Self::Criminal => 30.0,
            Self::Investigation => 25.0,
            Self::Financial => 25.0,
            Self::Reputation => 15.0,
            Self::Legal => 10.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn from_score(score: u32) -> Self {
        match score {
            0 => Self::None,
            1..=19 => Self::Low,
            20..=39 => Self::Medium,
            40..=69 => Self::High,
            _ => Self::Critical,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub category: RiskCategory,
    pub severity: RiskSeverity,
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub name: String,
    pub score: u32,
    pub level: RiskLevel,
    pub alerts: Vec<RiskAlert>,
    pub has_known_risk: bool,
    pub recommendation: String,
}

// ---------------------------------------------------------------------------
// Known risks database
// ---------------------------------------------------------------------------

struct KnownRisk {
    category: RiskCategory,
    severity: RiskSeverity,
    title: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
}

fn known_risks() -> HashMap<&'static str, KnownRisk> {
    let mut map = HashMap::new();
    map.insert(
        "fernando oliveira lima",
        KnownRisk {
            category: RiskCategory::Investigation,
            severity: RiskSeverity::Critical,
            title: "CPI das Bets - Indiciado",
            description: "Indiciado pela CPI das Bets do Senado por lavagem de dinheiro e associação criminosa. Apontado como responsável pelo Jogo do Tigrinho.",
            keywords: &["CPI", "tigrinho", "lavagem de dinheiro"],
        },
    );
    map.insert(
        "fernandin oig",
        KnownRisk {
            category: RiskCategory::Investigation,
            severity: RiskSeverity::Critical,
            title: "CPI das Bets - Investigado",
            description: "Investigado pela CPI das Bets. Movimentação financeira suspeita de R$ 110 milhões.",
            keywords: &["CPI", "tigrinho", "bet"],
        },
    );
    map
}

// ---------------------------------------------------------------------------
// Risk keywords by category
// ---------------------------------------------------------------------------

fn risk_keywords() -> HashMap<RiskCategory, Vec<&'static str>> {
    let mut map = HashMap::new();
    map.insert(RiskCategory::Criminal, vec![
        "prisão", "preso", "condenado", "crime", "tráfico",
        "assassinato", "homicídio", "roubo", "furto", "sequestro",
    ]);
    map.insert(RiskCategory::Investigation, vec![
        "CPI", "investigação", "investigado", "indiciado", "operação",
        "PF", "Polícia Federal", "inquérito", "Ministério Público", "denúncia",
    ]);
    map.insert(RiskCategory::Financial, vec![
        "lavagem", "lavagem de dinheiro", "fraude", "sonegação", "evasão fiscal",
        "pirâmide", "esquema", "desvio", "corrupção", "propina", "caixa 2",
    ]);
    map.insert(RiskCategory::Reputation, vec![
        "tigrinho", "bet", "apostas ilegais", "jogo ilegal", "escândalo",
        "polêmica", "acusação", "denunciado", "cancelado",
    ]);
    map.insert(RiskCategory::Legal, vec![
        "processo", "ação judicial", "falência", "recuperação judicial",
        "dívida", "execução fiscal", "protesto", "negativado", "SPC", "Serasa",
    ]);
    map
}

// ---------------------------------------------------------------------------
// RiskDetectorService
// ---------------------------------------------------------------------------

pub struct RiskDetectorService;

impl RiskDetectorService {
    /// Quick check against known risks DB (no web search).
    pub fn quick_check(name: &str) -> Option<RiskAlert> {
        let normalized = name.to_lowercase().trim().to_string();
        let known = known_risks();

        // Exact match
        if let Some(risk) = known.get(normalized.as_str()) {
            return Some(RiskAlert {
                category: risk.category,
                severity: risk.severity,
                title: risk.title.to_string(),
                description: risk.description.to_string(),
                keywords: risk.keywords.iter().map(|s| s.to_string()).collect(),
            });
        }

        // Partial match
        for (risk_name, risk) in &known {
            if normalized.contains(risk_name) || risk_name.contains(normalized.as_str()) {
                return Some(RiskAlert {
                    category: risk.category,
                    severity: risk.severity,
                    title: risk.title.to_string(),
                    description: risk.description.to_string(),
                    keywords: risk.keywords.iter().map(|s| s.to_string()).collect(),
                });
            }
        }

        None
    }

    /// Full risk assessment with web search.
    pub async fn assess_risk(
        name: &str,
        web_search: &WebSearchService,
    ) -> RiskAssessment {
        let mut score: f64 = 0.0;
        let mut alerts = Vec::new();
        let mut has_known_risk = false;

        // 1. Check known risks
        if let Some(alert) = Self::quick_check(name) {
            score += alert.category.weight() * alert.severity.multiplier();
            has_known_risk = true;
            alerts.push(alert);
        }

        // 2. Search news for negative mentions
        if web_search.is_enabled() {
            let news = web_search.search_news(name).await;
            for article in &news {
                if article.is_negative {
                    let category = Self::categorize_keywords(&article.keywords);
                    let severity = Self::determine_severity(&article.keywords);
                    score += category.weight() * severity.multiplier();
                    alerts.push(RiskAlert {
                        category,
                        severity,
                        title: article.title.clone(),
                        description: article.snippet.clone(),
                        keywords: article.keywords.clone(),
                    });
                }
            }
        }

        let final_score = (score.round() as u32).min(100);
        let level = RiskLevel::from_score(final_score);
        let recommendation = Self::recommendation(level, &alerts);

        RiskAssessment {
            name: name.to_string(),
            score: final_score,
            level,
            alerts,
            has_known_risk,
            recommendation,
        }
    }

    fn categorize_keywords(keywords: &[String]) -> RiskCategory {
        let risk_kw = risk_keywords();
        let mut counts: HashMap<RiskCategory, usize> = HashMap::new();

        for kw in keywords {
            let lower = kw.to_lowercase();
            for (cat, cat_keywords) in &risk_kw {
                if cat_keywords.iter().any(|ck| lower.contains(&ck.to_lowercase())) {
                    *counts.entry(*cat).or_default() += 1;
                }
            }
        }

        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(cat, _)| cat)
            .unwrap_or(RiskCategory::Reputation)
    }

    fn determine_severity(keywords: &[String]) -> RiskSeverity {
        let critical = ["prisão", "preso", "condenado", "cpi", "lavagem"];
        let high = ["investigação", "investigado", "indiciado", "fraude", "crime"];
        let medium = ["processo", "denúncia", "acusação", "polêmica"];

        for kw in keywords {
            let lower = kw.to_lowercase();
            if critical.iter().any(|c| lower.contains(c)) {
                return RiskSeverity::Critical;
            }
        }
        for kw in keywords {
            let lower = kw.to_lowercase();
            if high.iter().any(|h| lower.contains(h)) {
                return RiskSeverity::High;
            }
        }
        for kw in keywords {
            let lower = kw.to_lowercase();
            if medium.iter().any(|m| lower.contains(m)) {
                return RiskSeverity::Medium;
            }
        }
        RiskSeverity::Low
    }

    fn recommendation(level: RiskLevel, alerts: &[RiskAlert]) -> String {
        match level {
            RiskLevel::None => "Nenhum risco identificado. Prosseguir normalmente.".to_string(),
            RiskLevel::Low => "Risco baixo. Verificação adicional recomendada.".to_string(),
            RiskLevel::Medium => "Risco moderado. Análise manual recomendada.".to_string(),
            RiskLevel::High => "Risco alto. Não recomendado sem aprovação da gestão.".to_string(),
            RiskLevel::Critical => {
                let title = alerts
                    .iter()
                    .filter(|a| a.severity == RiskSeverity::Critical)
                    .map(|a| a.title.as_str())
                    .next()
                    .unwrap_or("Risco crítico");
                format!("RISCO CRÍTICO: {}. NÃO PROSSEGUIR.", title)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_check_exact_match() {
        let result = RiskDetectorService::quick_check("Fernando Oliveira Lima");
        assert!(result.is_some());
        let alert = result.unwrap();
        assert_eq!(alert.severity, RiskSeverity::Critical);
        assert!(alert.title.contains("CPI"));
    }

    #[test]
    fn test_quick_check_no_match() {
        let result = RiskDetectorService::quick_check("João da Silva");
        assert!(result.is_none());
    }

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(RiskLevel::from_score(0), RiskLevel::None);
        assert_eq!(RiskLevel::from_score(10), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(25), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(50), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(80), RiskLevel::Critical);
    }

    #[test]
    fn test_determine_severity() {
        assert_eq!(
            RiskDetectorService::determine_severity(&["CPI".to_string()]),
            RiskSeverity::Critical
        );
        assert_eq!(
            RiskDetectorService::determine_severity(&["investigação".to_string()]),
            RiskSeverity::High
        );
        assert_eq!(
            RiskDetectorService::determine_severity(&["processo".to_string()]),
            RiskSeverity::Medium
        );
        assert_eq!(
            RiskDetectorService::determine_severity(&["outro".to_string()]),
            RiskSeverity::Low
        );
    }

    #[test]
    fn test_categorize_keywords() {
        let kw = vec!["CPI".to_string(), "investigado".to_string()];
        assert_eq!(
            RiskDetectorService::categorize_keywords(&kw),
            RiskCategory::Investigation
        );
    }

    #[test]
    fn test_recommendation() {
        assert!(RiskDetectorService::recommendation(RiskLevel::None, &[]).contains("Prosseguir"));
        assert!(RiskDetectorService::recommendation(RiskLevel::Critical, &[
            RiskAlert {
                category: RiskCategory::Investigation,
                severity: RiskSeverity::Critical,
                title: "CPI".to_string(),
                description: "test".to_string(),
                keywords: vec![],
            }
        ]).contains("NÃO PROSSEGUIR"));
    }
}
