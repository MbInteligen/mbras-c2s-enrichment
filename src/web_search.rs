//! Web search service via Google Custom Search Engine.
//!
//! Ported from ts:src/services/web-search.service.ts
//!
//! Rate-limited to 90 queries/day (free tier = 100, leave margin).
//! Provides person search, LinkedIn search, news search, domain analysis.

use chrono::{Datelike, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::Mutex;

const MAX_QUERIES_PER_DAY: u32 = 90;

// Negative news keywords (Portuguese)
const NEGATIVE_KEYWORDS: &[&str] = &[
    "investigação", "CPI", "prisão", "preso", "condenado", "fraude",
    "lavagem", "crime", "processo", "indiciado", "operação",
    "Polícia Federal", "inquérito", "denúncia", "escândalo",
    "corrupção", "sonegação", "tráfico", "assassinato",
];

// Education keywords for LinkedIn extraction
const EDUCATION_KEYWORDS: &[&str] = &[
    "Harvard", "Stanford", "MIT", "Yale", "Princeton", "Columbia",
    "Wharton", "INSEAD", "USP", "FGV", "Insper", "PUC",
];

// News sites for search filtering
const NEWS_SITES: &[&str] = &[
    "infomoney", "valor", "exame", "forbes", "estadao", "folha",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonInfo {
    pub full_name: Option<String>,
    pub role: Option<String>,
    pub company: Option<String>,
    pub education: Option<String>,
    pub linkedin_url: Option<String>,
    pub instagram_url: Option<String>,
    pub bio: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
    pub is_negative: bool,
    pub keywords: Vec<String>,
}

pub struct WebSearchService {
    http: Client,
    api_key: Option<String>,
    cse_id: Option<String>,
    queries_used: AtomicU32,
    last_reset_day: Mutex<u32>,
}

impl WebSearchService {
    pub fn new() -> Self {
        let api_key = std::env::var("GOOGLE_API_KEY").ok().filter(|s| !s.is_empty());
        let cse_id = std::env::var("GOOGLE_CSE_ID").ok().filter(|s| !s.is_empty());

        if api_key.is_none() || cse_id.is_none() {
            tracing::warn!("GOOGLE_API_KEY or GOOGLE_CSE_ID not set — web search disabled");
        }

        Self {
            http: Client::new(),
            api_key,
            cse_id,
            queries_used: AtomicU32::new(0),
            last_reset_day: Mutex::new(Utc::now().day()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.api_key.is_some() && self.cse_id.is_some()
    }

    pub async fn quota_remaining(&self) -> u32 {
        self.maybe_reset_quota().await;
        MAX_QUERIES_PER_DAY.saturating_sub(self.queries_used.load(Ordering::Relaxed))
    }

    /// Search Google Custom Search Engine.
    pub async fn search(&self, query: &str, num: u32) -> Vec<SearchResult> {
        let (api_key, cse_id) = match (&self.api_key, &self.cse_id) {
            (Some(k), Some(c)) => (k, c),
            _ => return vec![],
        };

        self.maybe_reset_quota().await;
        if self.queries_used.load(Ordering::Relaxed) >= MAX_QUERIES_PER_DAY {
            tracing::warn!("Google Search daily quota exhausted");
            return vec![];
        }

        let url = format!(
            "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={}",
            api_key,
            cse_id,
            urlencoding::encode(query),
            num.min(10),
        );

        let resp = match self.http.get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Google Search request failed: {}", e);
                return vec![];
            }
        };

        self.queries_used.fetch_add(1, Ordering::Relaxed);

        let body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Google Search response parse error: {}", e);
                return vec![];
            }
        };

        let items = match body.get("items").and_then(|v| v.as_array()) {
            Some(items) => items,
            None => return vec![],
        };

        items
            .iter()
            .filter_map(|item| {
                Some(SearchResult {
                    title: item.get("title")?.as_str()?.to_string(),
                    url: item.get("link")?.as_str()?.to_string(),
                    snippet: item
                        .get("snippet")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    source: "google".to_string(),
                })
            })
            .collect()
    }

    /// Search for a person (LinkedIn, business profiles).
    pub async fn search_person(&self, name: &str) -> PersonInfo {
        let mut info = PersonInfo {
            full_name: Some(name.to_string()),
            role: None,
            company: None,
            education: None,
            linkedin_url: None,
            instagram_url: None,
            bio: None,
            source: "google_search".to_string(),
        };

        // LinkedIn search
        let query = format!("site:linkedin.com/in \"{}\"", name);
        let results = self.search(&query, 5).await;
        for r in &results {
            if r.url.contains("linkedin.com/in/") {
                info.linkedin_url = Some(r.url.clone());
                // Extract role/company from snippet
                if let Some((role, company)) = extract_role_company(&r.snippet) {
                    info.role = Some(role);
                    info.company = Some(company);
                }
                // Check education
                for kw in EDUCATION_KEYWORDS {
                    if r.snippet.contains(kw) || r.title.contains(kw) {
                        info.education = Some(kw.to_string());
                        break;
                    }
                }
                break;
            }
        }

        // General search for company/role if LinkedIn didn't find
        if info.company.is_none() {
            let query2 = format!("\"{}\" São Paulo empresário CEO fundador", name);
            let results2 = self.search(&query2, 5).await;
            for r in &results2 {
                if let Some((role, company)) = extract_role_company(&r.snippet) {
                    info.role = Some(role);
                    info.company = Some(company);
                    break;
                }
            }
        }

        info
    }

    /// Search news and flag negative results.
    pub async fn search_news(&self, name: &str) -> Vec<NewsResult> {
        let sites = NEWS_SITES.join(" OR site:");
        let query = format!("\"{}\" site:{}", name, sites);
        let results = self.search(&query, 10).await;

        results
            .into_iter()
            .map(|r| {
                let text = format!("{} {}", r.title, r.snippet).to_lowercase();
                let matched_keywords: Vec<String> = NEGATIVE_KEYWORDS
                    .iter()
                    .filter(|kw| text.contains(&kw.to_lowercase()))
                    .map(|kw| kw.to_string())
                    .collect();
                let is_negative = !matched_keywords.is_empty();
                NewsResult {
                    title: r.title,
                    url: r.url,
                    snippet: r.snippet,
                    source: r.source,
                    is_negative,
                    keywords: matched_keywords,
                }
            })
            .collect()
    }

    async fn maybe_reset_quota(&self) {
        let today = Utc::now().day();
        let mut last = self.last_reset_day.lock().await;
        if *last != today {
            self.queries_used.store(0, Ordering::Relaxed);
            *last = today;
        }
    }
}

/// Extract role and company from text like "Director at Company XYZ".
fn extract_role_company(text: &str) -> Option<(String, String)> {
    let patterns = [" at ", " na ", " em ", " @ ", " - ", " | "];
    for pat in &patterns {
        if let Some(pos) = text.find(pat) {
            let role = text[..pos].trim();
            let company = text[pos + pat.len()..].trim();
            // Clean up: take first sentence/segment
            let company = company.split(&['.', ',', '|', '-'][..]).next().unwrap_or(company).trim();
            if !role.is_empty() && !company.is_empty() && role.len() < 100 && company.len() < 100 {
                return Some((role.to_string(), company.to_string()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_role_company() {
        assert_eq!(
            extract_role_company("CEO at Acme Corp"),
            Some(("CEO".to_string(), "Acme Corp".to_string()))
        );
        assert_eq!(
            extract_role_company("Diretor na Empresa XYZ"),
            Some(("Diretor".to_string(), "Empresa XYZ".to_string()))
        );
        assert_eq!(extract_role_company("no pattern here"), None);
    }

    #[test]
    fn test_negative_keywords_detection() {
        let text = "investigação sobre fraude na empresa";
        let matched: Vec<&str> = NEGATIVE_KEYWORDS
            .iter()
            .filter(|kw| text.contains(&kw.to_lowercase()))
            .copied()
            .collect();
        assert!(matched.contains(&"investigação"));
        assert!(matched.contains(&"fraude"));
    }

    #[tokio::test]
    async fn test_quota_management() {
        let svc = WebSearchService::new();
        let remaining = svc.quota_remaining().await;
        assert_eq!(remaining, MAX_QUERIES_PER_DAY);
    }

    #[test]
    fn test_is_enabled_without_keys() {
        let svc = WebSearchService::new();
        // Without env vars set, should be disabled
        assert!(!svc.is_enabled());
    }
}
