//! CPF Discovery Service — 5-tier phone + 2-tier email fallback.
//!
//! Port of ts-c2s-api `src/services/cpf-discovery.service.ts`.
//! Uses Work API phone/name/mail modules with mod-11 validation
//! and Levenshtein-based name matching.

use crate::config::Config;
use crate::cpf::{is_valid_cpf, normalize_cpf};
use crate::errors::AppError;
use crate::name_matcher::{find_best_match, match_names_with_threshold};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Minimum milliseconds between Work API requests (global rate limit).
const WORK_API_RATE_LIMIT_MS: u64 = 2000;

/// Timeout for Work API phone/name/mail modules.
const WORK_API_DISCOVERY_TIMEOUT_S: u64 = 15;

/// Minimum name length for name-based discovery (Tier 2/3).
const NAME_MIN_LENGTH: usize = 5;

/// Maximum results from Work API name module before rejecting as ambiguous.
const WORK_NAME_MAX_RESULTS: usize = 20;

/// Minimum match score for CPF discovery acceptance.
const NAME_MATCH_THRESHOLD: f64 = 0.7;

/// Global timestamp of last Work API request (ms since epoch).
static LAST_REQUEST_TIME: AtomicU64 = AtomicU64::new(0);

/// Result of a CPF discovery operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpfDiscoveryResult {
    /// Normalized 11-digit CPF.
    pub cpf: String,
    /// Name as found in the data source.
    pub found_name: String,
    /// Whether lead name matched the found name.
    pub name_matches: bool,
    /// Similarity score (0.0-1.0).
    pub match_score: f64,
    /// Match method used (e.g., "exact", "fuzzy-full").
    pub match_method: String,
    /// Discovery source (e.g., "work-api", "work-api-name").
    pub source: String,
}

/// Work API phone module response item.
#[derive(Debug, Deserialize)]
struct PhoneResult {
    cpf_cnpj: Option<String>,
    nome: Option<String>,
}

/// Work API phone module response wrapper.
#[derive(Debug, Deserialize)]
struct PhoneResponse {
    msg: Option<Vec<PhoneResult>>,
}

/// Work API name module response item.
#[derive(Debug, Deserialize)]
struct NameResult {
    cpf: Option<String>,
    nome: Option<String>,
    #[serde(rename = "dataNascimento")]
    _data_nascimento: Option<String>,
    #[serde(rename = "nomeMae")]
    _nome_mae: Option<String>,
}

/// Work API name module response wrapper.
#[derive(Debug, Deserialize)]
struct NameResponse {
    data: Option<Vec<NameResult>>,
}

/// CPF Lookup API (DuckDB) response item.
#[derive(Debug, Deserialize)]
struct DuckDbResult {
    cpf: Option<String>,
    nome_completo: Option<String>,
}

/// CPF Lookup API (DuckDB) response wrapper.
#[derive(Debug, Deserialize)]
struct DuckDbResponse {
    count: Option<usize>,
    results: Option<Vec<DuckDbResult>>,
}

/// Work API mail module response item (same structure as phone).
#[derive(Debug, Deserialize)]
struct MailResult {
    cpf_cnpj: Option<String>,
    nome: Option<String>,
}

/// Work API mail module response wrapper.
#[derive(Debug, Deserialize)]
struct MailResponse {
    msg: Option<Vec<MailResult>>,
}

pub struct CpfDiscoveryService {
    client: reqwest::Client,
    base_url: String,
    api_token: String,
    duckdb_url: String,
    duckdb_timeout_ms: u64,
}

impl CpfDiscoveryService {
    pub fn new(config: &Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(WORK_API_DISCOVERY_TIMEOUT_S))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            base_url: "https://completa.workbuscas.com".to_string(),
            api_token: config.worker_api_key.clone(),
            duckdb_url: config.cpf_lookup_api_url.clone(),
            duckdb_timeout_ms: config.cpf_lookup_timeout_ms,
        }
    }

    /// Enforce global rate limit: wait if needed to maintain 2s gap.
    async fn enforce_rate_limit() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last = LAST_REQUEST_TIME.load(Ordering::Relaxed);
        if last > 0 {
            let elapsed = now_ms.saturating_sub(last);
            if elapsed < WORK_API_RATE_LIMIT_MS {
                let wait = WORK_API_RATE_LIMIT_MS - elapsed;
                tracing::debug!("Rate limit: waiting {}ms before Work API call", wait);
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
            }
        }
        let actual_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        LAST_REQUEST_TIME.store(actual_now, Ordering::Relaxed);
    }

    /// Tier 1: Find CPF by phone via Work API `phone` module.
    pub async fn find_cpf_by_phone_work_api(
        &self,
        phone: &str,
    ) -> Result<Option<CpfDiscoveryResult>, AppError> {
        Self::enforce_rate_limit().await;
        tracing::info!("Tier 1: Work API phone module for {}", phone);

        let url = reqwest::Url::parse_with_params(
            &format!("{}/api", self.base_url),
            &[
                ("token", self.api_token.as_str()),
                ("modulo", "phone"),
                ("consulta", phone),
            ],
        )
        .map_err(|e| AppError::ExternalApiError(format!("URL build failed: {}", e)))?;

        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Tier 1 Work API phone failed: {}", e);
                return Ok(None);
            }
        };

        if !resp.status().is_success() {
            tracing::warn!("Tier 1 Work API phone status: {}", resp.status());
            return Ok(None);
        }

        let body: PhoneResponse = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Tier 1 Work API phone parse error: {}", e);
                return Ok(None);
            }
        };

        // Find first valid CPF in results
        if let Some(results) = body.msg {
            for r in &results {
                if let Some(ref raw_cpf) = r.cpf_cnpj {
                    let normalized = normalize_cpf(raw_cpf);
                    if normalized.len() == 11 && is_valid_cpf(&normalized) {
                        return Ok(Some(CpfDiscoveryResult {
                            cpf: normalized,
                            found_name: r.nome.clone().unwrap_or_default(),
                            name_matches: true,
                            match_score: 1.0,
                            match_method: "no-validation".to_string(),
                            source: "work-api".to_string(),
                        }));
                    }
                }
            }
        }

        tracing::info!("Tier 1: No valid CPF found via phone module");
        Ok(None)
    }

    /// Tier 2: Find CPF by name via Work API `name` module.
    ///
    /// Only attempted when lead_name >= 5 chars and results <= 20.
    pub async fn find_cpf_by_name_work_api(
        &self,
        lead_name: &str,
    ) -> Result<Option<CpfDiscoveryResult>, AppError> {
        if lead_name.len() < NAME_MIN_LENGTH {
            tracing::debug!("Tier 2: name too short ({} chars), skipping", lead_name.len());
            return Ok(None);
        }

        Self::enforce_rate_limit().await;
        tracing::info!("Tier 2: Work API name module for {:?}", lead_name);

        let url = reqwest::Url::parse_with_params(
            &format!("{}/api", self.base_url),
            &[
                ("token", self.api_token.as_str()),
                ("modulo", "name"),
                ("consulta", lead_name),
            ],
        )
        .map_err(|e| AppError::ExternalApiError(format!("URL build failed: {}", e)))?;

        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Tier 2 Work API name failed: {}", e);
                return Ok(None);
            }
        };

        if !resp.status().is_success() {
            tracing::warn!("Tier 2 Work API name status: {}", resp.status());
            return Ok(None);
        }

        let body: NameResponse = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Tier 2 Work API name parse error: {}", e);
                return Ok(None);
            }
        };

        let results = match body.data {
            Some(r) if !r.is_empty() => r,
            _ => {
                tracing::info!("Tier 2: No results from name module");
                return Ok(None);
            }
        };

        // Ambiguity guard
        if results.len() > WORK_NAME_MAX_RESULTS {
            tracing::info!(
                "Tier 2: Too many results ({}), skipping",
                results.len()
            );
            return Ok(None);
        }

        // Build candidates: (name, cpf) pairs with valid CPFs only
        let candidates: Vec<(String, String)> = results
            .iter()
            .filter_map(|r| {
                let cpf = normalize_cpf(r.cpf.as_deref()?);
                if cpf.len() == 11 && is_valid_cpf(&cpf) {
                    Some((r.nome.clone().unwrap_or_default(), cpf))
                } else {
                    None
                }
            })
            .collect();

        if candidates.is_empty() {
            tracing::info!("Tier 2: No valid CPFs in name results");
            return Ok(None);
        }

        // Find best name match
        if let Some((name, cpf, score, method)) =
            find_best_match(lead_name, &candidates, NAME_MATCH_THRESHOLD)
        {
            tracing::info!(
                "Tier 2: Best match {:?} (score={:.2}, method={})",
                name,
                score,
                method
            );
            return Ok(Some(CpfDiscoveryResult {
                cpf,
                found_name: name,
                name_matches: true,
                match_score: score,
                match_method: method,
                source: "work-api-name".to_string(),
            }));
        }

        tracing::info!("Tier 2: No name match above threshold");
        Ok(None)
    }

    /// Tier 1 (email): Find CPF by email via Work API `mail` module.
    pub async fn find_cpf_by_email_work_api(
        &self,
        email: &str,
        lead_name: Option<&str>,
    ) -> Result<Option<CpfDiscoveryResult>, AppError> {
        Self::enforce_rate_limit().await;
        tracing::info!("Email Tier 1: Work API mail module for {}", email);

        let url = reqwest::Url::parse_with_params(
            &format!("{}/api", self.base_url),
            &[
                ("token", self.api_token.as_str()),
                ("modulo", "mail"),
                ("consulta", email),
            ],
        )
        .map_err(|e| AppError::ExternalApiError(format!("URL build failed: {}", e)))?;

        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Email Tier 1 Work API mail failed: {}", e);
                return Ok(None);
            }
        };

        if !resp.status().is_success() {
            tracing::warn!("Email Tier 1 Work API mail status: {}", resp.status());
            return Ok(None);
        }

        let body: MailResponse = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Email Tier 1 Work API mail parse error: {}", e);
                return Ok(None);
            }
        };

        let results = match body.msg {
            Some(r) if !r.is_empty() => r,
            _ => {
                tracing::info!("Email Tier 1: No results from mail module");
                return Ok(None);
            }
        };

        // Filter to valid CPFs
        let valid: Vec<(String, String)> = results
            .iter()
            .filter_map(|r| {
                let cpf = normalize_cpf(r.cpf_cnpj.as_deref()?);
                if cpf.len() == 11 && is_valid_cpf(&cpf) {
                    Some((r.nome.clone().unwrap_or_default(), cpf))
                } else {
                    None
                }
            })
            .collect();

        if valid.is_empty() {
            tracing::info!("Email Tier 1: No valid CPFs in mail results");
            return Ok(None);
        }

        // If single result or no lead_name, return first
        if valid.len() == 1 || lead_name.is_none() {
            let (name, cpf) = valid.into_iter().next().unwrap();
            return Ok(Some(CpfDiscoveryResult {
                cpf,
                found_name: name,
                name_matches: lead_name.is_none(),
                match_score: if lead_name.is_some() { 0.0 } else { 1.0 },
                match_method: "no-validation".to_string(),
                source: "work-api-mail".to_string(),
            }));
        }

        // Multiple results: disambiguate by name
        if let Some(ln) = lead_name {
            if let Some((name, cpf, score, method)) =
                find_best_match(ln, &valid, NAME_MATCH_THRESHOLD)
            {
                return Ok(Some(CpfDiscoveryResult {
                    cpf,
                    found_name: name,
                    name_matches: true,
                    match_score: score,
                    match_method: method,
                    source: "work-api-mail".to_string(),
                }));
            }
        }

        // Fallback to first valid result
        let (name, cpf) = valid.into_iter().next().unwrap();
        Ok(Some(CpfDiscoveryResult {
            cpf,
            found_name: name,
            name_matches: false,
            match_score: 0.0,
            match_method: "no-validation".to_string(),
            source: "work-api-mail".to_string(),
        }))
    }

    /// Tier 3: Search CPF by name via DuckDB API (223M records).
    ///
    /// Slow (~2 minutes) but has excellent coverage for rare names.
    /// Only called if Tiers 1-2 (Work API) fail.
    pub async fn find_cpf_by_name_duckdb(
        &self,
        lead_name: &str,
    ) -> Result<Option<CpfDiscoveryResult>, AppError> {
        if lead_name.len() < NAME_MIN_LENGTH {
            return Ok(None);
        }

        let url = format!(
            "{}/search/{}",
            self.duckdb_url,
            urlencoding::encode(&lead_name.trim().to_uppercase())
        );

        tracing::info!("Tier 3 (DuckDB): Searching CPF by name: {}", lead_name);

        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_millis(self.duckdb_timeout_ms))
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("DuckDB API error: {}", e)))?;

        if !resp.status().is_success() {
            tracing::warn!("DuckDB API returned status {}", resp.status());
            return Ok(None);
        }

        let data: DuckDbResponse = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("DuckDB parse error: {}", e)))?;

        let results = data.results.unwrap_or_default();
        if results.is_empty() {
            tracing::info!("DuckDB: No results for '{}'", lead_name);
            return Ok(None);
        }

        // Filter valid CPFs and build candidates
        let candidates: Vec<(String, String)> = results
            .into_iter()
            .filter_map(|r| {
                let cpf = normalize_cpf(r.cpf.as_deref().unwrap_or(""));
                let name = r.nome_completo.unwrap_or_default();
                if cpf.len() == 11 && is_valid_cpf(&cpf) {
                    Some((name, cpf))
                } else {
                    None
                }
            })
            .collect();

        if candidates.is_empty() {
            return Ok(None);
        }

        // Find best match by name
        if let Some((name, cpf, score, method)) =
            find_best_match(lead_name, &candidates, NAME_MATCH_THRESHOLD)
        {
            return Ok(Some(CpfDiscoveryResult {
                cpf,
                found_name: name,
                name_matches: true,
                match_score: score,
                match_method: method,
                source: "duckdb".to_string(),
            }));
        }

        // If only one result, return it even without name match
        if candidates.len() == 1 {
            let (name, cpf) = candidates.into_iter().next().unwrap();
            return Ok(Some(CpfDiscoveryResult {
                cpf,
                found_name: name,
                name_matches: false,
                match_score: 0.0,
                match_method: "single-result".to_string(),
                source: "duckdb".to_string(),
            }));
        }

        Ok(None)
    }

    /// Full 5-tier phone discovery: Work phone → Work name → DuckDB → Diretrix → DBase.
    ///
    /// Currently implements Tiers 1-2 (Work API). Tiers 3-5 delegate to the
    /// existing `find_cpf_via_diretrix` function in enrichment.rs.
    pub async fn find_cpf_by_phone(
        &self,
        phone: &str,
        lead_name: Option<&str>,
    ) -> Result<Option<CpfDiscoveryResult>, AppError> {
        // Tier 1: Work API phone
        if let Some(result) = self.find_cpf_by_phone_work_api(phone).await? {
            // Validate name match if lead_name provided
            if let Some(ln) = lead_name {
                let m = match_names_with_threshold(ln, &result.found_name, NAME_MATCH_THRESHOLD);
                return Ok(Some(CpfDiscoveryResult {
                    name_matches: m.matches,
                    match_score: m.score,
                    match_method: m.method,
                    ..result
                }));
            }
            return Ok(Some(result));
        }

        // Tier 2: Work API name (only if lead_name provided)
        if let Some(ln) = lead_name {
            if let Some(result) = self.find_cpf_by_name_work_api(ln).await? {
                return Ok(Some(result));
            }
        }

        // Tier 3: DuckDB CPF Lookup (223M records, slow ~2min)
        if let Some(ln) = lead_name {
            if let Ok(Some(result)) = self.find_cpf_by_name_duckdb(ln).await {
                return Ok(Some(result));
            }
        }

        // Tiers 4-5: Handled by enrichment.rs (DBase → Mimir)
        Ok(None)
    }

    /// Full 2-tier email discovery: Work mail → Diretrix.
    pub async fn find_cpf_by_email(
        &self,
        email: &str,
        lead_name: Option<&str>,
    ) -> Result<Option<CpfDiscoveryResult>, AppError> {
        // Tier 1: Work API mail
        if let Some(result) = self.find_cpf_by_email_work_api(email, lead_name).await? {
            return Ok(Some(result));
        }

        // Tier 2: Diretrix email — handled by enrichment.rs
        Ok(None)
    }

    /// Combined discovery: try phone first (5 tiers), then email (2 tiers).
    pub async fn find_cpf(
        &self,
        phone: Option<&str>,
        email: Option<&str>,
        lead_name: Option<&str>,
    ) -> Result<Option<CpfDiscoveryResult>, AppError> {
        // Phone discovery first (higher success rate)
        if let Some(ph) = phone {
            if let Some(result) = self.find_cpf_by_phone(ph, lead_name).await? {
                return Ok(Some(result));
            }
        }

        // Email discovery second
        if let Some(em) = email {
            if let Some(result) = self.find_cpf_by_email(em, lead_name).await? {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }
}
