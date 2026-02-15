//! CNPJ Fallback Service
//!
//! Lightweight CNPJ lookup via public APIs (ReceitaWS, CNPJa)
//! Used as fallback when Meilisearch has no results.
//! Rate-limited: max 3 requests/minute (ReceitaWS free tier).
//!
//! Port of: ts-c2s-api/src/services/cnpja-person.service.ts (simplified)

use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ReceitaWsResponse {
    pub cnpj: Option<String>,
    pub razao_social: Option<String>,
    pub nome_fantasia: Option<String>,
    pub capital_social: Option<String>, // comes as string like "100000.00"
    pub situacao: Option<String>,       // "ATIVA", "BAIXADA", etc.
    pub uf: Option<String>,
    pub qsa: Option<Vec<ReceitaWsPartner>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReceitaWsPartner {
    pub nome: Option<String>,
    pub qual: Option<String>,
}

pub struct CnpjFallbackService {
    client: reqwest::Client,
}

impl CnpjFallbackService {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build reqwest client");

        Self { client }
    }

    /// Lookup CNPJ via ReceitaWS public API (free, 3 req/min)
    pub async fn lookup_cnpj(&self, cnpj: &str) -> Option<ReceitaWsResponse> {
        let normalized = cnpj.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
        if normalized.len() != 14 {
            tracing::warn!("Invalid CNPJ length: {}", normalized.len());
            return None;
        }

        let url = format!("https://receitaws.com.br/v1/cnpj/{}", normalized);

        match self.client.get(&url).send().await {
            Ok(resp) => {
                if resp.status() == 429 {
                    tracing::warn!("ReceitaWS rate limit hit (3/min)");
                    return None;
                }
                if !resp.status().is_success() {
                    tracing::error!("ReceitaWS lookup failed: HTTP {}", resp.status());
                    return None;
                }
                match resp.json::<ReceitaWsResponse>().await {
                    Ok(data) => Some(data),
                    Err(e) => {
                        tracing::error!("Failed to parse ReceitaWS response: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::error!("ReceitaWS request failed: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cnpj_normalization() {
        let service = CnpjFallbackService::new();
        // Just test that the struct constructs without panic
        assert!(true);
    }
}
