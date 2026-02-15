//! Lead analysis service — orchestrates web intelligence, risk, domain analysis.
//!
//! Ported from ts:src/services/lead-analysis.service.ts
//!
//! Pipeline: quick risk → domain analysis → web search → Meilisearch → tier calculation
//! Results cached in `analytics.lead_analyses` table (7-day reuse).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::domain_analyzer::DomainAnalyzerService;
use crate::risk_detector::{RiskDetectorService, RiskAssessment, RiskLevel};
use crate::web_search::{WebSearchService, PersonInfo};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadAnalysisInput {
    pub lead_id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub cpf: Option<String>,
    pub income: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredInfo {
    pub full_name: Option<String>,
    pub company: Option<String>,
    pub role: Option<String>,
    pub education: Option<String>,
    pub linkedin: Option<String>,
    pub instagram: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadAnalysisResult {
    pub lead_id: String,
    pub tier: String,
    pub score: u32,
    pub discovered: DiscoveredInfo,
    pub alerts: Vec<String>,
    pub highlights: Vec<String>,
    pub sources: Vec<String>,
    pub recommendation: AnalysisRecommendation,
    pub risk_assessment: Option<RiskAssessment>,
    pub domain_trust_score: Option<u32>,
    pub duration_ms: u64,
    pub analyzed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRecommendation {
    pub action: String, // "avoid", "priority", "qualify", "contact"
    pub title: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// LeadAnalysisService
// ---------------------------------------------------------------------------

pub struct LeadAnalysisService {
    web_search: WebSearchService,
    db: PgPool,
}

impl LeadAnalysisService {
    pub fn new(db: PgPool) -> Self {
        Self {
            web_search: WebSearchService::new(),
            db,
        }
    }

    /// Analyze a lead — full pipeline.
    pub async fn analyze(&self, input: &LeadAnalysisInput) -> LeadAnalysisResult {
        let start = std::time::Instant::now();
        let mut alerts = Vec::new();
        let mut highlights = Vec::new();
        let mut sources = Vec::new();
        let mut discovered = DiscoveredInfo {
            full_name: Some(input.name.clone()),
            company: None,
            role: None,
            education: None,
            linkedin: None,
            instagram: None,
        };

        // Step 1: Quick risk check (no web search, instant)
        let quick_risk = RiskDetectorService::quick_check(&input.name);
        if let Some(ref alert) = quick_risk {
            alerts.push(format!("{}: {}", alert.title, alert.description));
            highlights.push(format!("⚠️ RISCO: {}", alert.title));
        }

        // Step 2: Domain analysis
        let mut domain_trust = None;
        if let Some(ref email) = input.email {
            let domain = DomainAnalyzerService::analyze(email);
            domain_trust = Some(domain.trust_score);
            if let Some(ref company) = domain.company_name {
                discovered.company = Some(company.clone());
                highlights.push(format!("🏢 Email corporativo: {}", company));
            }
            if let Some(ref sector) = domain.sector {
                highlights.push(format!("📊 Setor: {}", sector));
            }
        }

        // Step 3: Web search (if enabled and quota available)
        let mut risk_assessment = None;
        if self.web_search.is_enabled() && self.web_search.quota_remaining().await > 5 {
            // 3a. Person search
            let person = self.web_search.search_person(&input.name).await;
            merge_person_info(&mut discovered, &person);
            if person.linkedin_url.is_some() {
                sources.push("linkedin".to_string());
            }

            // 3b. Full risk assessment
            let risk = RiskDetectorService::assess_risk(&input.name, &self.web_search).await;
            if risk.score > 0 {
                for alert in &risk.alerts {
                    alerts.push(format!("{}: {}", alert.title, alert.description));
                }
            }
            risk_assessment = Some(risk);
        } else if quick_risk.is_some() {
            // Build risk assessment from quick check only
            risk_assessment = Some(RiskAssessment {
                name: input.name.clone(),
                score: 50,
                level: RiskLevel::High,
                alerts: vec![quick_risk.unwrap()],
                has_known_risk: true,
                recommendation: "RISCO CRÍTICO. NÃO PROSSEGUIR.".to_string(),
            });
        }

        // Step 4: Calculate tier and recommendation
        let (tier, score) = Self::calculate_tier(
            input,
            &discovered,
            domain_trust,
            risk_assessment.as_ref(),
        );

        let recommendation = Self::recommendation(&tier, score, &alerts);

        let duration_ms = start.elapsed().as_millis() as u64;

        let result = LeadAnalysisResult {
            lead_id: input.lead_id.clone(),
            tier,
            score,
            discovered,
            alerts,
            highlights,
            sources,
            recommendation,
            risk_assessment,
            domain_trust_score: domain_trust,
            duration_ms,
            analyzed_at: Utc::now(),
        };

        // Save to DB (fire and forget)
        self.save_analysis(&result).await;

        result
    }

    /// Retrieve cached analysis from database.
    pub async fn get_cached(&self, lead_id: &str) -> Option<LeadAnalysisResult> {
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            "SELECT analysis_data FROM analytics.lead_analyses WHERE lead_id = $1 AND analyzed_at > NOW() - INTERVAL '7 days' ORDER BY analyzed_at DESC LIMIT 1"
        )
        .bind(lead_id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()?;

        serde_json::from_value(row.0).ok()
    }

    fn calculate_tier(
        input: &LeadAnalysisInput,
        discovered: &DiscoveredInfo,
        domain_trust: Option<u32>,
        risk: Option<&RiskAssessment>,
    ) -> (String, u32) {
        // Risk override
        if let Some(r) = risk {
            if r.level == RiskLevel::Critical || r.level == RiskLevel::High {
                return ("risk".to_string(), 0);
            }
        }

        let mut score: u32 = 0;

        // Income
        if let Some(income) = input.income {
            score += match income as u64 {
                50000.. => 35,
                30000..=49999 => 30,
                15000..=29999 => 20,
                8000..=14999 => 10,
                4000..=7999 => 5,
                _ => 0,
            };
        }

        // Domain trust
        if let Some(trust) = domain_trust {
            score += match trust {
                80.. => 15,
                50..=79 => 5,
                _ => 0,
            };
        }

        // LinkedIn found
        if discovered.linkedin.is_some() {
            score += 10;
        }

        // Company discovered
        if discovered.company.is_some() {
            score += 10;
        }

        // Education
        if discovered.education.is_some() {
            score += 5;
        }

        // Has CPF (enrichment indicator)
        if input.cpf.is_some() {
            score += 5;
        }

        // Has email
        if input.email.is_some() {
            score += 5;
        }

        let tier = match score {
            60.. => "platinum",
            40..=59 => "gold",
            25..=39 => "silver",
            _ => "bronze",
        }
        .to_string();

        (tier, score.min(100))
    }

    fn recommendation(tier: &str, score: u32, alerts: &[String]) -> AnalysisRecommendation {
        if !alerts.is_empty() && tier == "risk" {
            return AnalysisRecommendation {
                action: "avoid".to_string(),
                title: "Risco Identificado".to_string(),
                description: "Lead com risco identificado. Não prosseguir sem aprovação.".to_string(),
            };
        }

        match tier {
            "platinum" => AnalysisRecommendation {
                action: "priority".to_string(),
                title: "Lead Premium".to_string(),
                description: format!("Lead de alto valor (score: {}). Contato prioritário.", score),
            },
            "gold" => AnalysisRecommendation {
                action: "priority".to_string(),
                title: "Lead Qualificado".to_string(),
                description: format!("Lead qualificado (score: {}). Agendar contato.", score),
            },
            "silver" => AnalysisRecommendation {
                action: "qualify".to_string(),
                title: "Qualificação Necessária".to_string(),
                description: format!("Lead com potencial (score: {}). Qualificar antes do contato.", score),
            },
            _ => AnalysisRecommendation {
                action: "contact".to_string(),
                title: "Contato Padrão".to_string(),
                description: format!("Lead padrão (score: {}). Seguir fluxo normal.", score),
            },
        }
    }

    async fn save_analysis(&self, result: &LeadAnalysisResult) {
        let data = match serde_json::to_value(result) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to serialize analysis: {}", e);
                return;
            }
        };

        let query = r#"
            INSERT INTO analytics.lead_analyses (lead_id, tier, score, analysis_data, analyzed_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (lead_id) DO UPDATE SET
                tier = EXCLUDED.tier,
                score = EXCLUDED.score,
                analysis_data = EXCLUDED.analysis_data,
                analyzed_at = EXCLUDED.analyzed_at
        "#;

        if let Err(e) = sqlx::query(query)
            .bind(&result.lead_id)
            .bind(&result.tier)
            .bind(result.score as i32)
            .bind(&data)
            .bind(&result.analyzed_at)
            .execute(&self.db)
            .await
        {
            tracing::warn!("Failed to save lead analysis: {}", e);
        }
    }
}

fn merge_person_info(discovered: &mut DiscoveredInfo, person: &PersonInfo) {
    if discovered.company.is_none() {
        discovered.company = person.company.clone();
    }
    if discovered.role.is_none() {
        discovered.role = person.role.clone();
    }
    if discovered.education.is_none() {
        discovered.education = person.education.clone();
    }
    if discovered.linkedin.is_none() {
        discovered.linkedin = person.linkedin_url.clone();
    }
    if discovered.instagram.is_none() {
        discovered.instagram = person.instagram_url.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_calculation_platinum() {
        let input = LeadAnalysisInput {
            lead_id: "test".to_string(),
            name: "Test Lead".to_string(),
            email: Some("user@btgpactual.com".to_string()),
            phone: None,
            cpf: Some("12345678901".to_string()),
            income: Some(50000.0),
        };
        let discovered = DiscoveredInfo {
            full_name: Some("Test".to_string()),
            company: Some("BTG".to_string()),
            role: Some("CEO".to_string()),
            education: Some("USP".to_string()),
            linkedin: Some("https://linkedin.com/in/test".to_string()),
            instagram: None,
        };
        let (tier, score) = LeadAnalysisService::calculate_tier(&input, &discovered, Some(90), None);
        assert_eq!(tier, "platinum");
        assert!(score >= 60);
    }

    #[test]
    fn test_tier_calculation_risk_override() {
        let input = LeadAnalysisInput {
            lead_id: "test".to_string(),
            name: "Test".to_string(),
            email: None,
            phone: None,
            cpf: None,
            income: Some(100000.0),
        };
        let discovered = DiscoveredInfo {
            full_name: None, company: None, role: None,
            education: None, linkedin: None, instagram: None,
        };
        let risk = RiskAssessment {
            name: "Test".to_string(),
            score: 80,
            level: RiskLevel::Critical,
            alerts: vec![],
            has_known_risk: true,
            recommendation: "NÃO PROSSEGUIR".to_string(),
        };
        let (tier, score) = LeadAnalysisService::calculate_tier(
            &input, &discovered, None, Some(&risk),
        );
        assert_eq!(tier, "risk");
        assert_eq!(score, 0);
    }

    #[test]
    fn test_tier_calculation_bronze() {
        let input = LeadAnalysisInput {
            lead_id: "test".to_string(),
            name: "Test".to_string(),
            email: None,
            phone: None,
            cpf: None,
            income: None,
        };
        let discovered = DiscoveredInfo {
            full_name: None, company: None, role: None,
            education: None, linkedin: None, instagram: None,
        };
        let (tier, _) = LeadAnalysisService::calculate_tier(
            &input, &discovered, None, None,
        );
        assert_eq!(tier, "bronze");
    }

    #[test]
    fn test_recommendation_risk() {
        let rec = LeadAnalysisService::recommendation(
            "risk", 0, &["CPI alert".to_string()],
        );
        assert_eq!(rec.action, "avoid");
    }

    #[test]
    fn test_recommendation_platinum() {
        let rec = LeadAnalysisService::recommendation("platinum", 85, &[]);
        assert_eq!(rec.action, "priority");
        assert!(rec.description.contains("85"));
    }
}
