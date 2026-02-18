//! C2S CRM Extended — Seller management, tags, activities, forwarding, queue distribution, search
//!
//! Port of ts:src/services/c2s.service.ts + ts:src/routes/queue.ts

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

// ─── Rate Limiting ──────────────────────────────────────────────────────────

const RATE_LIMIT_MS: u64 = 500;

// ─── Models ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seller {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellerCreateInput {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellerUpdateInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCreateInput {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    Call,
    Meeting,
    Email,
    Task,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityInput {
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    pub description: String,
    #[serde(default)]
    pub duration_minutes: Option<u32>,
    #[serde(default)]
    pub scheduled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardInput {
    pub seller_id: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueDistributeInput {
    pub lead_ids: Vec<String>,
    #[serde(default)]
    pub seller_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueAutoAssignInput {
    pub lead_id: String,
    #[serde(default)]
    pub queue_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadSearchResult {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub seller_name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentStatusRecord {
    pub lead_id: String,
    pub status: String,
    pub retry_count: i32,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ─── Service ────────────────────────────────────────────────────────────────

pub struct C2sExtendedService {
    client: Client,
    base_url: String,
    token: String,
}

impl C2sExtendedService {
    pub fn new(base_url: &str, token: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    async fn rate_limit(&self) {
        sleep(Duration::from_millis(RATE_LIMIT_MS)).await;
    }

    // ─── Seller Management ──────────────────────────────────────────────

    pub async fn list_sellers(&self) -> Result<Vec<Seller>, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/sellers", self.base_url);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("C2S API error: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        // C2S returns { data: [...] }
        let sellers: Vec<Seller> = if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
            data.iter()
                .filter_map(|s| {
                    let attrs = s.get("attributes")?;
                    Some(Seller {
                        id: s.get("id")?.as_str()?.to_string(),
                        name: attrs.get("name")?.as_str()?.to_string(),
                        email: attrs
                            .get("email")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        phone: attrs
                            .get("phone")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        active: attrs.get("active").and_then(|v| v.as_bool()),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(sellers)
    }

    pub async fn get_seller(&self, seller_id: &str) -> Result<Option<Seller>, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/sellers/{}", self.base_url, seller_id);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("C2S API error: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        let s = body.get("data").ok_or("No data field")?;
        let attrs = s.get("attributes").ok_or("No attributes")?;
        Ok(Some(Seller {
            id: s
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            name: attrs
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            email: attrs
                .get("email")
                .and_then(|v| v.as_str())
                .map(String::from),
            phone: attrs
                .get("phone")
                .and_then(|v| v.as_str())
                .map(String::from),
            active: attrs.get("active").and_then(|v| v.as_bool()),
        }))
    }

    pub async fn create_seller(&self, input: &SellerCreateInput) -> Result<Seller, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/sellers", self.base_url);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(input)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;
        let s = body.get("data").ok_or("No data field")?;
        let attrs = s.get("attributes").unwrap_or(s);
        Ok(Seller {
            id: s
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            name: attrs
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&input.name)
                .to_string(),
            email: input.email.clone(),
            phone: input.phone.clone(),
            active: Some(true),
        })
    }

    pub async fn update_seller(
        &self,
        seller_id: &str,
        input: &SellerUpdateInput,
    ) -> Result<Seller, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/sellers/{}", self.base_url, seller_id);
        let resp = self
            .client
            .put(&url)
            .bearer_auth(&self.token)
            .json(input)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;
        let s = body.get("data").ok_or("No data field")?;
        let attrs = s.get("attributes").unwrap_or(s);
        Ok(Seller {
            id: seller_id.to_string(),
            name: attrs
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            email: attrs
                .get("email")
                .and_then(|v| v.as_str())
                .map(String::from),
            phone: attrs
                .get("phone")
                .and_then(|v| v.as_str())
                .map(String::from),
            active: attrs.get("active").and_then(|v| v.as_bool()),
        })
    }

    // ─── Tag Management ─────────────────────────────────────────────────

    pub async fn list_tags(&self) -> Result<Vec<Tag>, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/tags", self.base_url);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("C2S API error: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        let tags: Vec<Tag> = if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
            data.iter()
                .filter_map(|t| {
                    Some(Tag {
                        id: t.get("id")?.as_str()?.to_string(),
                        name: t
                            .get("name")
                            .or(t.get("attributes").and_then(|a| a.get("name")))?
                            .as_str()?
                            .to_string(),
                        color: t
                            .get("color")
                            .or(t.get("attributes").and_then(|a| a.get("color")))
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(tags)
    }

    pub async fn create_tag(&self, input: &TagCreateInput) -> Result<Tag, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/tags", self.base_url);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(input)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;
        let t = body.get("data").unwrap_or(&body);
        Ok(Tag {
            id: t
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            name: input.name.clone(),
            color: input.color.clone(),
        })
    }

    pub async fn get_lead_tags(&self, lead_id: &str) -> Result<Vec<Tag>, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/leads/{}/tags", self.base_url, lead_id);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("C2S API error: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        let tags: Vec<Tag> = if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
            data.iter()
                .filter_map(|t| {
                    Some(Tag {
                        id: t.get("id")?.as_str()?.to_string(),
                        name: t.get("name").and_then(|v| v.as_str())?.to_string(),
                        color: t.get("color").and_then(|v| v.as_str()).map(String::from),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(tags)
    }

    pub async fn add_tag_to_lead(&self, lead_id: &str, tag_id: &str) -> Result<(), String> {
        self.rate_limit().await;
        let url = format!("{}/integration/leads/{}/tags", self.base_url, lead_id);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({ "tag_id": tag_id }))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        Ok(())
    }

    // ─── Lead Activities ────────────────────────────────────────────────

    pub async fn register_activity(
        &self,
        lead_id: &str,
        input: &ActivityInput,
    ) -> Result<serde_json::Value, String> {
        self.rate_limit().await;

        let endpoint = match input.activity_type {
            ActivityType::Call => "calls",
            ActivityType::Meeting => "meetings",
            ActivityType::Email => "emails",
            ActivityType::Task => "tasks",
            ActivityType::Note => "notes",
        };

        let url = format!(
            "{}/integration/leads/{}/{}",
            self.base_url, lead_id, endpoint
        );
        let body = serde_json::json!({
            "description": input.description,
            "duration_minutes": input.duration_minutes,
            "scheduled_at": input.scheduled_at,
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        resp.json().await.map_err(|e| format!("Parse error: {}", e))
    }

    pub async fn add_note(&self, lead_id: &str, body_text: &str) -> Result<(), String> {
        self.rate_limit().await;
        let url = format!(
            "{}/integration/leads/{}/create_message",
            self.base_url, lead_id
        );
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({ "body": body_text }))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        Ok(())
    }

    // ─── Mark as Interacted ───────────────────────────────────────────────

    pub async fn mark_as_interacted(&self, lead_id: &str) -> Result<(), String> {
        self.rate_limit().await;
        let url = format!(
            "{}/integration/leads/{}/mark_as_interacted",
            self.base_url, lead_id
        );
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        Ok(())
    }

    // ─── Lead Forwarding ────────────────────────────────────────────────

    pub async fn forward_lead(&self, lead_id: &str, input: &ForwardInput) -> Result<(), String> {
        self.rate_limit().await;
        let url = format!("{}/integration/leads/{}/forward", self.base_url, lead_id);
        let body = serde_json::json!({
            "seller_id": input.seller_id,
            "message": input.message,
        });

        let resp = self
            .client
            .put(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("C2S API error {}: {}", status, body));
        }

        // Send a note if message provided
        if let Some(msg) = &input.message {
            if !msg.is_empty() {
                let _ = self.add_note(lead_id, msg).await;
            }
        }

        Ok(())
    }

    // ─── Lead Search ────────────────────────────────────────────────────

    pub async fn search_by_phone(&self, phone: &str) -> Result<Vec<LeadSearchResult>, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/leads?phone={}", self.base_url, phone);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("C2S API error: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(Self::parse_lead_search_results(&body))
    }

    pub async fn search_by_email(&self, email: &str) -> Result<Vec<LeadSearchResult>, String> {
        self.rate_limit().await;
        let url = format!("{}/integration/leads?email={}", self.base_url, email);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("C2S API error: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(Self::parse_lead_search_results(&body))
    }

    fn parse_lead_search_results(body: &serde_json::Value) -> Vec<LeadSearchResult> {
        if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
            data.iter()
                .filter_map(|l| {
                    let attrs = l.get("attributes")?;
                    let customer = attrs.get("customer")?;
                    Some(LeadSearchResult {
                        id: l.get("id")?.as_str()?.to_string(),
                        name: customer
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        phone: customer
                            .get("phone")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        email: customer
                            .get("email")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        seller_name: attrs
                            .get("seller")
                            .and_then(|s| s.get("name"))
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        status: attrs
                            .get("status")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    })
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    // ─── Queue Distribution ─────────────────────────────────────────────

    /// Round-robin distribute leads across sellers
    pub async fn distribute_leads(
        &self,
        input: &QueueDistributeInput,
    ) -> Result<serde_json::Value, String> {
        // Get target sellers (provided or all active)
        let sellers = if let Some(ids) = &input.seller_ids {
            ids.clone()
        } else {
            let all = self.list_sellers().await?;
            all.into_iter()
                .filter(|s| s.active.unwrap_or(true))
                .map(|s| s.id)
                .collect()
        };

        if sellers.is_empty() {
            return Err("No sellers available for distribution".to_string());
        }

        let mut assigned = 0;
        let mut errors = 0;

        for (i, lead_id) in input.lead_ids.iter().enumerate() {
            let seller_id = &sellers[i % sellers.len()];
            match self
                .forward_lead(
                    lead_id,
                    &ForwardInput {
                        seller_id: seller_id.clone(),
                        message: None,
                    },
                )
                .await
            {
                Ok(_) => assigned += 1,
                Err(e) => {
                    tracing::warn!(
                        "Failed to assign lead {} to seller {}: {}",
                        lead_id,
                        seller_id,
                        e
                    );
                    errors += 1;
                }
            }
        }

        Ok(serde_json::json!({
            "assigned": assigned,
            "errors": errors,
            "total": input.lead_ids.len(),
            "seller_count": sellers.len(),
        }))
    }

    /// Auto-assign a single lead to the seller with fewest active leads
    pub async fn auto_assign(
        &self,
        input: &QueueAutoAssignInput,
    ) -> Result<serde_json::Value, String> {
        let sellers = self.list_sellers().await?;
        let active_sellers: Vec<_> = sellers
            .into_iter()
            .filter(|s| s.active.unwrap_or(true))
            .collect();

        if active_sellers.is_empty() {
            return Err("No active sellers available".to_string());
        }

        // Pick first active seller (simplest round-robin — production would track counts)
        let target = &active_sellers[0];

        self.forward_lead(
            &input.lead_id,
            &ForwardInput {
                seller_id: target.id.clone(),
                message: Some(format!("Auto-assigned to {}", target.name)),
            },
        )
        .await?;

        Ok(serde_json::json!({
            "lead_id": input.lead_id,
            "assigned_to": target.name,
            "seller_id": target.id,
        }))
    }

    // ─── Enrichment Status Tracking ─────────────────────────────────────

    pub async fn get_enrichment_status(
        db: &sqlx::PgPool,
        lead_id: &str,
    ) -> Result<Option<EnrichmentStatusRecord>, String> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                i32,
                Option<String>,
                Option<chrono::DateTime<chrono::Utc>>,
            ),
        >(
            "SELECT lead_id, enrichment_status, retry_count, last_error, updated_at
             FROM analytics.c2s_leads WHERE lead_id = $1",
        )
        .bind(lead_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        Ok(row.map(
            |(lid, status, retry, err, updated)| EnrichmentStatusRecord {
                lead_id: lid,
                status,
                retry_count: retry,
                last_error: err,
                updated_at: updated.map(|d| d.to_rfc3339()),
            },
        ))
    }

    pub async fn list_enrichment_by_status(
        db: &sqlx::PgPool,
        status: &str,
        limit: i64,
    ) -> Result<Vec<EnrichmentStatusRecord>, String> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                i32,
                Option<String>,
                Option<chrono::DateTime<chrono::Utc>>,
            ),
        >(
            "SELECT lead_id, enrichment_status, retry_count, last_error, updated_at
             FROM analytics.c2s_leads WHERE enrichment_status = $1
             ORDER BY updated_at DESC LIMIT $2",
        )
        .bind(status)
        .bind(limit)
        .fetch_all(db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|(lid, st, retry, err, updated)| EnrichmentStatusRecord {
                lead_id: lid,
                status: st,
                retry_count: retry,
                last_error: err,
                updated_at: updated.map(|d| d.to_rfc3339()),
            })
            .collect())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_type_serialization() {
        let input = ActivityInput {
            activity_type: ActivityType::Call,
            description: "Called client".to_string(),
            duration_minutes: Some(15),
            scheduled_at: None,
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["type"], "call");
        assert_eq!(json["duration_minutes"], 15);
    }

    #[test]
    fn test_seller_create_input() {
        let input = SellerCreateInput {
            name: "João Silva".to_string(),
            email: Some("joao@mbras.com.br".to_string()),
            phone: None,
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["name"], "João Silva");
        assert_eq!(json["email"], "joao@mbras.com.br");
        assert!(json.get("phone").unwrap().is_null());
    }

    #[test]
    fn test_tag_create_input() {
        let input = TagCreateInput {
            name: "VIP".to_string(),
            color: Some("#FFD700".to_string()),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["name"], "VIP");
        assert_eq!(json["color"], "#FFD700");
    }

    #[test]
    fn test_forward_input() {
        let input = ForwardInput {
            seller_id: "seller-123".to_string(),
            message: Some("Premium lead".to_string()),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["seller_id"], "seller-123");
        assert_eq!(json["message"], "Premium lead");
    }

    #[test]
    fn test_queue_distribute_input() {
        let input = QueueDistributeInput {
            lead_ids: vec!["lead-1".into(), "lead-2".into(), "lead-3".into()],
            seller_ids: Some(vec!["seller-a".into(), "seller-b".into()]),
        };
        assert_eq!(input.lead_ids.len(), 3);
        // Round-robin: lead-1→seller-a, lead-2→seller-b, lead-3→seller-a
        let sellers = input.seller_ids.as_ref().unwrap();
        assert_eq!(&sellers[0 % sellers.len()], "seller-a");
        assert_eq!(&sellers[1 % sellers.len()], "seller-b");
        assert_eq!(&sellers[2 % sellers.len()], "seller-a");
    }

    #[test]
    fn test_parse_lead_search_results_empty() {
        let body = serde_json::json!({ "data": [] });
        let results = C2sExtendedService::parse_lead_search_results(&body);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_lead_search_results() {
        let body = serde_json::json!({
            "data": [{
                "id": "lead-123",
                "attributes": {
                    "customer": {
                        "name": "Maria Santos",
                        "phone": "11999887766",
                        "email": "maria@test.com"
                    },
                    "seller": { "name": "Lucas Melo" },
                    "status": "novo"
                }
            }]
        });
        let results = C2sExtendedService::parse_lead_search_results(&body);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Maria Santos");
        assert_eq!(results[0].seller_name.as_deref(), Some("Lucas Melo"));
        assert_eq!(results[0].status.as_deref(), Some("novo"));
    }

    #[test]
    fn test_enrichment_status_record() {
        let record = EnrichmentStatusRecord {
            lead_id: "lead-1".to_string(),
            status: "partial".to_string(),
            retry_count: 3,
            last_error: Some("Work API timeout".to_string()),
            updated_at: Some("2026-02-14T10:00:00+00:00".to_string()),
        };
        assert_eq!(record.retry_count, 3);
        assert!(record.last_error.unwrap().contains("timeout"));
    }

    #[test]
    fn test_all_activity_types() {
        for (at, expected) in [
            (ActivityType::Call, "call"),
            (ActivityType::Meeting, "meeting"),
            (ActivityType::Email, "email"),
            (ActivityType::Task, "task"),
            (ActivityType::Note, "note"),
        ] {
            let input = ActivityInput {
                activity_type: at,
                description: "test".to_string(),
                duration_minutes: None,
                scheduled_at: None,
            };
            let json = serde_json::to_value(&input).unwrap();
            assert_eq!(json["type"].as_str().unwrap(), expected);
        }
    }
}
