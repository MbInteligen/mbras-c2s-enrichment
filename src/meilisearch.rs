//! Meilisearch Company Service
//!
//! Integration with Meilisearch IBVI (65.2M Brazilian companies).
//! Searches companies by CPF of partner, CNPJ, or text query.
//!
//! Base: https://ibvi-meilisearch-v2.fly.dev
//! Port of: ts-c2s-api/src/services/meilisearch-company.service.ts

use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Receita Federal partner qualification codes -> human-readable labels
fn qualificacao_label(code: &str) -> &'static str {
    match code {
        "05" => "Administrador",
        "08" => "Conselheiro de Administração",
        "10" => "Diretor",
        "16" => "Presidente",
        "22" => "Sócio",
        "28" | "49" => "Sócio-Administrador",
        "29" => "Sócio-Gerente",
        "54" => "Fundador",
        "65" => "Titular",
        _ => "Sócio",
    }
}

/// Whether a qualification code indicates an administrator role
fn is_admin_code(code: &str) -> bool {
    matches!(code, "05" | "08" | "10" | "16" | "49")
}

// ── Meilisearch response types ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeilisearchSocio {
    pub cpf: Option<String>,
    pub nome: Option<String>,
    pub qualificacao: Option<String>,
    pub data_entrada: Option<String>,
    pub percentual: Option<f64>,
    pub faixa_etaria: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeilisearchCompany {
    pub cnpj: Option<String>,
    pub razao_social: Option<String>,
    pub nome_fantasia: Option<String>,
    pub capital_social: Option<f64>,
    pub situacao_cadastral: Option<String>,
    pub uf: Option<String>,
    pub socios: Option<Vec<MeilisearchSocio>>,
    pub socios_cpfs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct MeilisearchSearchResponse {
    hits: Option<Vec<MeilisearchCompany>>,
}

// ── Output types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanySummaryEntry {
    pub cnpj: String,
    pub razao_social: String,
    pub nome_fantasia: Option<String>,
    pub capital_social: f64,
    pub situacao: String,
    pub uf: Option<String>,
    pub is_administrador: bool,
    pub cargo: String,
    pub total_socios: usize,
    pub participacao_estimada: u32, // 1/total_socios as % (0-100)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanySummary {
    pub total_companies: usize,
    pub total_capital_social: f64,
    pub companies: Vec<CompanySummaryEntry>,
}

impl CompanySummary {
    pub fn empty() -> Self {
        Self {
            total_companies: 0,
            total_capital_social: 0.0,
            companies: vec![],
        }
    }
}

// ── Service ─────────────────────────────────────────────────────────

pub struct MeilisearchCompanyService {
    base_url: String,
    api_key: String,
    enabled: bool,
    client: reqwest::Client,
    cpf_cache: Cache<String, CompanySummary>,
    cnpj_cache: Cache<String, MeilisearchCompany>,
}

impl MeilisearchCompanyService {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let enabled = !api_key.is_empty();
        if !enabled {
            tracing::warn!("MeilisearchCompanyService disabled — no API key");
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build reqwest client");

        // CPF cache: 5K entries, 4h TTL
        let cpf_cache = Cache::builder()
            .max_capacity(5_000)
            .time_to_live(Duration::from_secs(4 * 3600))
            .build();

        // CNPJ cache: 2K entries, 4h TTL
        let cnpj_cache = Cache::builder()
            .max_capacity(2_000)
            .time_to_live(Duration::from_secs(4 * 3600))
            .build();

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            enabled,
            client,
            cpf_cache,
            cnpj_cache,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Find all companies where a CPF is a partner (sócio).
    /// Uses Meilisearch filter: `socios_cpfs = {cpf}`
    pub async fn find_companies_by_cpf(&self, cpf: &str) -> CompanySummary {
        if !self.enabled {
            return CompanySummary::empty();
        }

        let normalized = cpf.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
        if normalized.len() != 11 {
            tracing::warn!("Invalid CPF length for Meilisearch lookup: {}", normalized.len());
            return CompanySummary::empty();
        }

        // Check cache
        if let Some(cached) = self.cpf_cache.get(&normalized).await {
            tracing::debug!("Meilisearch CPF cache hit: {}", &normalized[..3]);
            return cached;
        }

        tracing::info!("Searching companies by CPF via Meilisearch");

        let url = format!("{}/indexes/companies/search", self.base_url);
        let body = serde_json::json!({
            "filter": format!("socios_cpfs = {}", normalized),
            "limit": 50
        });

        let result = match self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::error!("Meilisearch CPF search failed: HTTP {}", resp.status());
                    return CompanySummary::empty();
                }
                match resp.json::<MeilisearchSearchResponse>().await {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::error!("Failed to parse Meilisearch response: {}", e);
                        return CompanySummary::empty();
                    }
                }
            }
            Err(e) => {
                tracing::error!("Meilisearch request failed: {}", e);
                return CompanySummary::empty();
            }
        };

        let companies = result.hits.unwrap_or_default();

        // Filter active companies (situacao_cadastral = "02")
        let active: Vec<_> = companies
            .into_iter()
            .filter(|c| c.situacao_cadastral.as_deref() == Some("02"))
            .collect();

        let total_capital: f64 = active.iter().map(|c| c.capital_social.unwrap_or(0.0)).sum();

        let mut entries: Vec<CompanySummaryEntry> = active
            .iter()
            .map(|c| {
                let socios = c.socios.as_deref().unwrap_or(&[]);
                let socio = socios.iter().find(|s| s.cpf.as_deref() == Some(&normalized));
                let qual_code = socio
                    .and_then(|s| s.qualificacao.as_deref())
                    .unwrap_or("");
                let cargo = qualificacao_label(qual_code).to_string();
                let total_socios = socios.len();
                let participacao = if total_socios > 0 {
                    (100.0 / total_socios as f64).round() as u32
                } else {
                    0
                };

                CompanySummaryEntry {
                    cnpj: c.cnpj.clone().unwrap_or_default(),
                    razao_social: c.razao_social.clone().unwrap_or_default(),
                    nome_fantasia: c.nome_fantasia.clone(),
                    capital_social: c.capital_social.unwrap_or(0.0),
                    situacao: c.situacao_cadastral.clone().unwrap_or_default(),
                    uf: c.uf.clone(),
                    is_administrador: is_admin_code(qual_code),
                    cargo,
                    total_socios,
                    participacao_estimada: participacao,
                }
            })
            .collect();

        // Sort by capital (descending)
        entries.sort_by(|a, b| b.capital_social.partial_cmp(&a.capital_social).unwrap_or(std::cmp::Ordering::Equal));

        let summary = CompanySummary {
            total_companies: entries.len(),
            total_capital_social: total_capital,
            companies: entries,
        };

        tracing::info!(
            "Found {} active companies for CPF, total capital: {:.2}",
            summary.total_companies,
            summary.total_capital_social,
        );

        self.cpf_cache.insert(normalized, summary.clone()).await;
        summary
    }

    /// Get a single company by CNPJ
    pub async fn get_company_by_cnpj(&self, cnpj: &str) -> Option<MeilisearchCompany> {
        if !self.enabled {
            return None;
        }

        let normalized = cnpj.chars().filter(|c| c.is_ascii_digit()).collect::<String>();

        // Check cache
        if let Some(cached) = self.cnpj_cache.get(&normalized).await {
            return Some(cached);
        }

        let url = format!("{}/indexes/companies/search", self.base_url);
        let body = serde_json::json!({
            "filter": format!("cnpj = {}", normalized),
            "limit": 1
        });

        let result = match self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<MeilisearchSearchResponse>().await.ok()?
            }
            Ok(resp) => {
                tracing::error!("Meilisearch CNPJ lookup failed: HTTP {}", resp.status());
                return None;
            }
            Err(e) => {
                tracing::error!("Meilisearch CNPJ request failed: {}", e);
                return None;
            }
        };

        let company = result.hits?.into_iter().next()?;
        self.cnpj_cache.insert(normalized, company.clone()).await;
        Some(company)
    }

    /// Search companies by text query (name or CNPJ)
    pub async fn search_companies(&self, query: &str, limit: usize) -> Vec<MeilisearchCompany> {
        if !self.enabled {
            return vec![];
        }

        let url = format!("{}/indexes/companies/search", self.base_url);
        let body = serde_json::json!({ "q": query, "limit": limit });

        match self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<MeilisearchSearchResponse>()
                    .await
                    .ok()
                    .and_then(|r| r.hits)
                    .unwrap_or_default()
            }
            Ok(resp) => {
                tracing::error!("Meilisearch search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::error!("Meilisearch search request failed: {}", e);
                vec![]
            }
        }
    }

    /// Format company summary for C2S message (matches TS format)
    pub fn format_companies_for_message(summary: &CompanySummary) -> String {
        if summary.total_companies == 0 {
            return String::new();
        }

        let mut lines = Vec::new();
        let plural = if summary.total_companies > 1 { "s" } else { "" };
        lines.push(format!(
            "\n🏢 EMPRESÁRIO ({} empresa{})",
            summary.total_companies, plural
        ));

        if summary.total_capital_social > 0.0 {
            lines.push(format!(
                "   Capital total: R$ {}",
                format_brl(summary.total_capital_social)
            ));
        }

        // Show top 5 companies
        for company in summary.companies.iter().take(5) {
            lines.push(format!("   • {}", company.razao_social));
            let mut detail = "     ".to_string();
            if company.capital_social > 0.0 {
                detail.push_str(&format!("Capital: R$ {}", format_brl(company.capital_social)));
            }
            detail.push_str(&format!(" | {}", company.cargo));
            if company.total_socios > 0 {
                detail.push_str(&format!(
                    " (1/{} sócios, ~{}%)",
                    company.total_socios, company.participacao_estimada
                ));
            }
            if let Some(ref uf) = company.uf {
                detail.push_str(&format!(" [{}]", uf));
            }
            lines.push(detail);
        }

        if summary.companies.len() > 5 {
            lines.push(format!(
                "   ... e mais {} empresa(s)",
                summary.companies.len() - 5
            ));
        }

        lines.join("\n")
    }
}

/// Format a number as Brazilian Real currency (e.g., 1.234.567,89)
fn format_brl(value: f64) -> String {
    let abs = value.abs();
    let cents = ((abs * 100.0).round() as u64) % 100;
    let whole = (abs.round() as u64) / 1; // integer part

    // Format integer part with dots as thousands separator
    let int_part = abs as u64;
    let int_str = int_part.to_string();
    let mut formatted = String::new();
    for (i, ch) in int_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            formatted.push('.');
        }
        formatted.push(ch);
    }
    let formatted: String = formatted.chars().rev().collect();
    format!("{},{:02}", formatted, cents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qualificacao_labels() {
        assert_eq!(qualificacao_label("10"), "Diretor");
        assert_eq!(qualificacao_label("16"), "Presidente");
        assert_eq!(qualificacao_label("28"), "Sócio-Administrador");
        assert_eq!(qualificacao_label("49"), "Sócio-Administrador");
        assert_eq!(qualificacao_label("22"), "Sócio");
        assert_eq!(qualificacao_label("99"), "Sócio"); // unknown defaults to Sócio
    }

    #[test]
    fn test_is_admin_code() {
        assert!(is_admin_code("05"));
        assert!(is_admin_code("10"));
        assert!(is_admin_code("16"));
        assert!(!is_admin_code("22"));
        assert!(!is_admin_code("28"));
    }

    #[test]
    fn test_format_brl() {
        assert_eq!(format_brl(1234.56), "1.234,56");
        assert_eq!(format_brl(1000000.0), "1.000.000,00");
        assert_eq!(format_brl(0.0), "0,00");
        assert_eq!(format_brl(99.99), "99,99");
    }

    #[test]
    fn test_format_companies_empty() {
        let summary = CompanySummary::empty();
        assert_eq!(MeilisearchCompanyService::format_companies_for_message(&summary), "");
    }

    #[test]
    fn test_format_companies_single() {
        let summary = CompanySummary {
            total_companies: 1,
            total_capital_social: 100_000.0,
            companies: vec![CompanySummaryEntry {
                cnpj: "12345678000190".to_string(),
                razao_social: "EMPRESA TESTE LTDA".to_string(),
                nome_fantasia: None,
                capital_social: 100_000.0,
                situacao: "02".to_string(),
                uf: Some("SP".to_string()),
                is_administrador: true,
                cargo: "Diretor".to_string(),
                total_socios: 3,
                participacao_estimada: 33,
            }],
        };
        let msg = MeilisearchCompanyService::format_companies_for_message(&summary);
        assert!(msg.contains("EMPRESÁRIO (1 empresa)"));
        assert!(msg.contains("EMPRESA TESTE LTDA"));
        assert!(msg.contains("Diretor"));
        assert!(msg.contains("1/3 sócios"));
        assert!(msg.contains("[SP]"));
    }

    #[test]
    fn test_company_summary_sorting() {
        // Verify entries sort by capital descending
        let mut entries = vec![
            CompanySummaryEntry {
                cnpj: "1".to_string(),
                razao_social: "Small".to_string(),
                nome_fantasia: None,
                capital_social: 1_000.0,
                situacao: "02".to_string(),
                uf: None,
                is_administrador: false,
                cargo: "Sócio".to_string(),
                total_socios: 2,
                participacao_estimada: 50,
            },
            CompanySummaryEntry {
                cnpj: "2".to_string(),
                razao_social: "Big".to_string(),
                nome_fantasia: None,
                capital_social: 1_000_000.0,
                situacao: "02".to_string(),
                uf: None,
                is_administrador: true,
                cargo: "Presidente".to_string(),
                total_socios: 4,
                participacao_estimada: 25,
            },
        ];
        entries.sort_by(|a, b| b.capital_social.partial_cmp(&a.capital_social).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(entries[0].razao_social, "Big");
        assert_eq!(entries[1].razao_social, "Small");
    }
}
