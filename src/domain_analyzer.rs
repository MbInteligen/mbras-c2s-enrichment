//! Domain analyzer — company identification and trust scoring from email domains.
//!
//! Ported from ts:src/services/domain-analyzer.service.ts

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAnalysis {
    pub domain: String,
    pub is_personal_email: bool,
    pub is_business_email: bool,
    pub trust_score: u32,
    pub company_name: Option<String>,
    pub sector: Option<String>,
    pub insights: Vec<String>,
}

// ---------------------------------------------------------------------------
// Personal email domains
// ---------------------------------------------------------------------------

fn personal_domains() -> HashSet<&'static str> {
    [
        "gmail.com", "hotmail.com", "outlook.com", "yahoo.com", "yahoo.com.br",
        "icloud.com", "live.com", "msn.com", "uol.com.br", "bol.com.br",
        "terra.com.br", "globo.com", "ig.com.br", "r7.com", "protonmail.com",
        "pm.me", "tutanota.com", "zoho.com", "aol.com", "mail.com",
        "globomail.com", "zipmail.com.br",
    ]
    .into_iter()
    .collect()
}

// ---------------------------------------------------------------------------
// Known corporate domains (high-value)
// ---------------------------------------------------------------------------

struct KnownDomain {
    name: &'static str,
    sector: &'static str,
}

fn known_domains() -> Vec<(&'static str, KnownDomain)> {
    vec![
        // Banks
        ("itau.com.br", KnownDomain { name: "Itaú Unibanco", sector: "Banco" }),
        ("bradesco.com.br", KnownDomain { name: "Bradesco", sector: "Banco" }),
        ("btgpactual.com", KnownDomain { name: "BTG Pactual", sector: "Banco de Investimentos" }),
        ("santander.com.br", KnownDomain { name: "Santander", sector: "Banco" }),
        ("safra.com.br", KnownDomain { name: "Banco Safra", sector: "Banco" }),
        ("xpi.com.br", KnownDomain { name: "XP Investimentos", sector: "Investimentos" }),
        // Tech
        ("google.com", KnownDomain { name: "Google", sector: "Tecnologia" }),
        ("microsoft.com", KnownDomain { name: "Microsoft", sector: "Tecnologia" }),
        ("amazon.com", KnownDomain { name: "Amazon", sector: "E-commerce/Cloud" }),
        ("meta.com", KnownDomain { name: "Meta", sector: "Tecnologia" }),
        ("apple.com", KnownDomain { name: "Apple", sector: "Tecnologia" }),
        // VC / Investment
        ("allievocapital.com", KnownDomain { name: "Allievo Capital", sector: "Venture Capital" }),
        ("softbank.com", KnownDomain { name: "SoftBank", sector: "Venture Capital" }),
        ("a16z.com", KnownDomain { name: "Andreessen Horowitz", sector: "Venture Capital" }),
        // Real estate
        ("mbras.com.br", KnownDomain { name: "MBRAS", sector: "Imobiliário" }),
        ("lopes.com.br", KnownDomain { name: "Lopes", sector: "Imobiliário" }),
        ("incorp.com.br", KnownDomain { name: "InCorp", sector: "Imobiliário" }),
        // Law
        ("machadomeyer.com.br", KnownDomain { name: "Machado Meyer", sector: "Advocacia" }),
        ("pfrlaw.com.br", KnownDomain { name: "PFR Law", sector: "Advocacia" }),
        ("mattosfilho.com.br", KnownDomain { name: "Mattos Filho", sector: "Advocacia" }),
        // Consulting
        ("mckinsey.com", KnownDomain { name: "McKinsey", sector: "Consultoria" }),
        ("bcg.com", KnownDomain { name: "Boston Consulting Group", sector: "Consultoria" }),
        ("bain.com", KnownDomain { name: "Bain & Company", sector: "Consultoria" }),
        ("deloitte.com", KnownDomain { name: "Deloitte", sector: "Consultoria" }),
        ("ey.com", KnownDomain { name: "Ernst & Young", sector: "Consultoria" }),
        ("kpmg.com", KnownDomain { name: "KPMG", sector: "Consultoria" }),
        ("pwc.com", KnownDomain { name: "PwC", sector: "Consultoria" }),
    ]
}

// ---------------------------------------------------------------------------
// High-trust patterns
// ---------------------------------------------------------------------------

fn is_high_trust_domain(domain: &str) -> bool {
    let patterns = [
        ".gov.br", ".edu.br", ".edu", ".org.br", ".mil.br",
    ];
    let keywords = ["bank", "capital", "invest", "ventures"];

    for p in &patterns {
        if domain.ends_with(p) {
            return true;
        }
    }
    let lower = domain.to_lowercase();
    for kw in &keywords {
        if lower.contains(kw) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// DomainAnalyzerService
// ---------------------------------------------------------------------------

pub struct DomainAnalyzerService;

impl DomainAnalyzerService {
    /// Extract domain from email address.
    pub fn extract_domain(email: &str) -> Option<String> {
        email
            .rsplit_once('@')
            .map(|(_, domain)| domain.to_lowercase())
            .filter(|d| d.contains('.'))
    }

    /// Analyze email domain for company info and trust score.
    pub fn analyze(email: &str) -> DomainAnalysis {
        let domain = match Self::extract_domain(email) {
            Some(d) => d,
            None => {
                return DomainAnalysis {
                    domain: String::new(),
                    is_personal_email: true,
                    is_business_email: false,
                    trust_score: 0,
                    company_name: None,
                    sector: None,
                    insights: vec!["Email inválido".to_string()],
                };
            }
        };

        let personal = personal_domains();
        let is_personal = personal.contains(domain.as_str());

        // Check known domains
        for (d, info) in known_domains() {
            if domain == d {
                return DomainAnalysis {
                    domain: domain.clone(),
                    is_personal_email: false,
                    is_business_email: true,
                    trust_score: 90,
                    company_name: Some(info.name.to_string()),
                    sector: Some(info.sector.to_string()),
                    insights: vec![format!("Domínio conhecido: {}", info.name)],
                };
            }
        }

        // Personal email
        if is_personal {
            return DomainAnalysis {
                domain,
                is_personal_email: true,
                is_business_email: false,
                trust_score: 30,
                company_name: None,
                sector: None,
                insights: vec!["Email pessoal - sem informação de empresa".to_string()],
            };
        }

        // High-trust pattern
        let trust_score = if is_high_trust_domain(&domain) {
            80
        } else {
            50 // Unknown corporate domain
        };

        // Try to extract sector hint from domain name
        let sector = Self::sector_hint(&domain);
        let mut insights = vec!["Domínio corporativo".to_string()];
        if let Some(ref s) = sector {
            insights.push(format!("Setor identificado: {}", s));
        }

        DomainAnalysis {
            domain,
            is_personal_email: false,
            is_business_email: true,
            trust_score,
            company_name: None,
            sector,
            insights,
        }
    }

    /// Check if domain indicates a high-value company.
    pub fn is_high_value_domain(domain: &str) -> bool {
        let high_value_sectors = ["Venture Capital", "Banco", "Banco de Investimentos", "Private Equity"];
        for (d, info) in known_domains() {
            if domain == d && high_value_sectors.contains(&info.sector) {
                return true;
            }
        }
        false
    }

    /// Extract sector hint from domain name.
    fn sector_hint(domain: &str) -> Option<String> {
        let lower = domain.to_lowercase();
        let hints = [
            ("capital", "Investimentos"),
            ("bank", "Banco"),
            ("invest", "Investimentos"),
            ("imob", "Imobiliário"),
            ("constru", "Construção"),
            ("tech", "Tecnologia"),
            ("digital", "Tecnologia"),
            ("law", "Advocacia"),
            ("adv", "Advocacia"),
            ("consult", "Consultoria"),
            ("seguros", "Seguros"),
            ("saude", "Saúde"),
            ("health", "Saúde"),
            ("farm", "Farmacêutico"),
            ("agro", "Agronegócio"),
        ];
        for (kw, sector) in &hints {
            if lower.contains(kw) {
                return Some(sector.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            DomainAnalyzerService::extract_domain("user@example.com"),
            Some("example.com".to_string())
        );
        assert_eq!(
            DomainAnalyzerService::extract_domain("USER@ITAU.COM.BR"),
            Some("itau.com.br".to_string())
        );
        assert_eq!(DomainAnalyzerService::extract_domain("invalid"), None);
    }

    #[test]
    fn test_personal_email() {
        let result = DomainAnalyzerService::analyze("user@gmail.com");
        assert!(result.is_personal_email);
        assert_eq!(result.trust_score, 30);
    }

    #[test]
    fn test_known_domain() {
        let result = DomainAnalyzerService::analyze("user@btgpactual.com");
        assert!(!result.is_personal_email);
        assert_eq!(result.trust_score, 90);
        assert_eq!(result.company_name.unwrap(), "BTG Pactual");
        assert_eq!(result.sector.unwrap(), "Banco de Investimentos");
    }

    #[test]
    fn test_high_trust_pattern() {
        let result = DomainAnalyzerService::analyze("user@usp.edu.br");
        assert_eq!(result.trust_score, 80);
    }

    #[test]
    fn test_unknown_corporate() {
        let result = DomainAnalyzerService::analyze("user@somecompany.com.br");
        assert!(!result.is_personal_email);
        assert!(result.is_business_email);
        assert_eq!(result.trust_score, 50);
    }

    #[test]
    fn test_sector_hint() {
        let result = DomainAnalyzerService::analyze("user@acmecapital.com");
        assert_eq!(result.sector.unwrap(), "Investimentos");
    }

    #[test]
    fn test_is_high_value() {
        assert!(DomainAnalyzerService::is_high_value_domain("btgpactual.com"));
        assert!(DomainAnalyzerService::is_high_value_domain("softbank.com"));
        assert!(!DomainAnalyzerService::is_high_value_domain("gmail.com"));
    }
}
