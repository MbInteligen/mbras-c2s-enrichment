//! Alert service for Slack webhook and Email (Resend API) notifications.
//!
//! Ported from ts:src/services/alert.service.ts
//!
//! Features:
//! - Slack webhook alerts (POST to ALERT_WEBHOOK_URL)
//! - Email alerts via Resend API (POST to https://api.resend.com/emails)
//! - Rate limiting per alert type:service composite key
//! - Service health tracking (6 services)
//! - High-value lead alerts (async, non-blocking)
//! - Service-down alerts (5+ consecutive errors)
//! - Low enrichment rate alerts (<80% threshold)

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    HighValueLead,
    HighErrorRate,
    ServiceDown,
    LowEnrichmentRate,
    LeadMaxRetries,
    SystemInfo,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HighValueLead => "high_value_lead",
            Self::HighErrorRate => "high_error_rate",
            Self::ServiceDown => "service_down",
            Self::LowEnrichmentRate => "low_enrichment_rate",
            Self::LeadMaxRetries => "lead_max_retries",
            Self::SystemInfo => "system_info",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::HighValueLead => "💎",
            Self::HighErrorRate => "🔴",
            Self::ServiceDown => "🚨",
            Self::LowEnrichmentRate => "⚠️",
            Self::LeadMaxRetries => "🔄",
            Self::SystemInfo => "ℹ️",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertPayload {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub service: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackedService {
    WorkApi,
    Diretrix,
    DBase,
    Meilisearch,
    C2S,
    CpfLookup,
}

impl TrackedService {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkApi => "work_api",
            Self::Diretrix => "diretrix",
            Self::DBase => "dbase",
            Self::Meilisearch => "meilisearch",
            Self::C2S => "c2s",
            Self::CpfLookup => "cpf_lookup",
        }
    }

    pub fn all() -> &'static [TrackedService] {
        &[
            Self::WorkApi,
            Self::Diretrix,
            Self::DBase,
            Self::Meilisearch,
            Self::C2S,
            Self::CpfLookup,
        ]
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub service: TrackedService,
    pub healthy: bool,
    pub consecutive_errors: u32,
    pub last_success: Option<DateTime<Utc>>,
    pub last_error: Option<DateTime<Utc>>,
    pub last_error_message: Option<String>,
    pub tracked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceHealthReport {
    pub services: Vec<ServiceStatus>,
    pub all_healthy: bool,
    pub checked_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct ServiceState {
    consecutive_errors: u32,
    last_success: Option<DateTime<Utc>>,
    last_error: Option<DateTime<Utc>>,
    last_error_message: Option<String>,
}

struct AlertInternalState {
    /// Rate-limit: composite key "type:service" -> last sent timestamp
    rate_limit_map: HashMap<String, DateTime<Utc>>,
    /// Per-service health tracking
    service_states: HashMap<TrackedService, ServiceState>,
}

// ---------------------------------------------------------------------------
// AlertService
// ---------------------------------------------------------------------------

/// Minimum seconds between alerts of the same type:service composite key.
const RATE_LIMIT_SECS: i64 = 300; // 5 minutes
/// Consecutive errors before firing service_down alert.
const SERVICE_DOWN_THRESHOLD: u32 = 5;

pub struct AlertService {
    http: Client,
    slack_webhook_url: Option<String>,
    resend_api_key: Option<String>,
    alert_email_to: String,
    alert_email_from: String,
    state: Arc<RwLock<AlertInternalState>>,
}

impl AlertService {
    pub fn new() -> Self {
        let slack_url = std::env::var("ALERT_WEBHOOK_URL").ok().filter(|s| !s.is_empty());
        let resend_key = std::env::var("RESEND_API_KEY").ok().filter(|s| !s.is_empty());
        let email_to = std::env::var("ALERT_EMAIL_TO")
            .unwrap_or_else(|_| "ronaldo@incorp.com.br".to_string());
        let email_from = std::env::var("ALERT_EMAIL_FROM")
            .unwrap_or_else(|_| "alerts@ts-c2s-api.fly.dev".to_string());

        if slack_url.is_none() {
            tracing::warn!("ALERT_WEBHOOK_URL not set — Slack alerts disabled");
        }
        if resend_key.is_none() {
            tracing::warn!("RESEND_API_KEY not set — email alerts disabled");
        }

        let mut service_states = HashMap::new();
        for svc in TrackedService::all() {
            service_states.insert(
                *svc,
                ServiceState {
                    consecutive_errors: 0,
                    last_success: None,
                    last_error: None,
                    last_error_message: None,
                },
            );
        }

        Self {
            http: Client::new(),
            slack_webhook_url: slack_url,
            resend_api_key: resend_key,
            alert_email_to: email_to,
            alert_email_from: email_from,
            state: Arc::new(RwLock::new(AlertInternalState {
                rate_limit_map: HashMap::new(),
                service_states,
            })),
        }
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Send an alert (Slack + Email). Rate-limited by composite key.
    pub async fn send_alert(&self, payload: &AlertPayload) {
        let composite_key = match &payload.service {
            Some(svc) => format!("{}:{}", payload.alert_type.as_str(), svc),
            None => payload.alert_type.as_str().to_string(),
        };

        if !self.check_rate_limit(&composite_key).await {
            tracing::debug!("Alert rate-limited: {}", composite_key);
            return;
        }

        // Fire Slack and Email concurrently
        let (slack_result, email_result) = tokio::join!(
            self.send_slack(payload),
            self.send_email(payload),
        );

        if let Err(e) = slack_result {
            tracing::error!("Slack alert failed: {}", e);
        }
        if let Err(e) = email_result {
            tracing::error!("Email alert failed: {}", e);
        }
    }

    /// Record a service call result (success or failure).
    /// Fires service_down alert if consecutive errors >= threshold.
    pub async fn record_service_status(
        &self,
        service: TrackedService,
        success: bool,
        error_message: Option<&str>,
    ) {
        let should_alert = {
            let mut state = self.state.write().await;
            let svc_state = state
                .service_states
                .entry(service)
                .or_insert_with(|| ServiceState {
                    consecutive_errors: 0,
                    last_success: None,
                    last_error: None,
                    last_error_message: None,
                });

            if success {
                svc_state.consecutive_errors = 0;
                svc_state.last_success = Some(Utc::now());
                false
            } else {
                svc_state.consecutive_errors += 1;
                svc_state.last_error = Some(Utc::now());
                svc_state.last_error_message = error_message.map(|s| s.to_string());
                svc_state.consecutive_errors == SERVICE_DOWN_THRESHOLD
            }
        };

        if should_alert {
            let msg = error_message.unwrap_or("Unknown error");
            self.send_alert(&AlertPayload {
                alert_type: AlertType::ServiceDown,
                severity: AlertSeverity::Critical,
                title: format!("{} service down", service.as_str()),
                message: format!(
                    "Service {} has {} consecutive errors. Last error: {}",
                    service.as_str(),
                    SERVICE_DOWN_THRESHOLD,
                    msg
                ),
                service: Some(service.as_str().to_string()),
                metadata: None,
            })
            .await;
        }
    }

    /// Get health report for all tracked services.
    pub async fn get_service_health(&self) -> ServiceHealthReport {
        let state = self.state.read().await;
        let mut services = Vec::new();
        let mut all_healthy = true;

        for svc in TrackedService::all() {
            let (healthy, consecutive_errors, last_success, last_error, last_error_message) =
                match state.service_states.get(svc) {
                    Some(s) => (
                        s.consecutive_errors < SERVICE_DOWN_THRESHOLD,
                        s.consecutive_errors,
                        s.last_success,
                        s.last_error,
                        s.last_error_message.clone(),
                    ),
                    None => (true, 0, None, None, None),
                };

            if !healthy {
                all_healthy = false;
            }

            services.push(ServiceStatus {
                service: *svc,
                healthy,
                consecutive_errors,
                last_success,
                last_error,
                last_error_message,
                tracked: true,
            });
        }

        ServiceHealthReport {
            services,
            all_healthy,
            checked_at: Utc::now(),
        }
    }

    /// Send a high-value lead alert (async, non-blocking).
    pub fn send_high_value_alert_async(self: &Arc<Self>, name: &str, income: f64, details: &str) {
        let this = Arc::clone(self);
        let name = name.to_string();
        let details = details.to_string();
        tokio::spawn(async move {
            this.send_alert(&AlertPayload {
                alert_type: AlertType::HighValueLead,
                severity: AlertSeverity::Critical,
                title: format!("Lead de alto valor: {}", name),
                message: format!(
                    "Renda: R$ {:.2}/mês\n{}",
                    income, details
                ),
                service: None,
                metadata: Some(serde_json::json!({
                    "name": name,
                    "income": income,
                })),
            })
            .await;
        });
    }

    /// Send a low enrichment rate alert.
    pub async fn send_low_rate_alert(&self, rate: f64, threshold: f64, total: u64, enriched: u64) {
        self.send_alert(&AlertPayload {
            alert_type: AlertType::LowEnrichmentRate,
            severity: AlertSeverity::Warning,
            title: format!("Enrichment rate below threshold: {:.1}%", rate * 100.0),
            message: format!(
                "Current rate: {:.1}% (threshold: {:.1}%)\nEnriched: {} / {} total",
                rate * 100.0,
                threshold * 100.0,
                enriched,
                total
            ),
            service: None,
            metadata: Some(serde_json::json!({
                "rate": rate,
                "threshold": threshold,
                "total": total,
                "enriched": enriched,
            })),
        })
        .await;
    }

    // ------------------------------------------------------------------
    // Private
    // ------------------------------------------------------------------

    /// Returns true if this alert should be sent (not rate-limited).
    async fn check_rate_limit(&self, composite_key: &str) -> bool {
        let mut state = self.state.write().await;
        let now = Utc::now();

        if let Some(last_sent) = state.rate_limit_map.get(composite_key) {
            let elapsed = (now - *last_sent).num_seconds();
            if elapsed < RATE_LIMIT_SECS {
                return false;
            }
        }

        state
            .rate_limit_map
            .insert(composite_key.to_string(), now);
        true
    }

    /// Send alert to Slack via webhook.
    async fn send_slack(&self, payload: &AlertPayload) -> Result<(), String> {
        let url = match &self.slack_webhook_url {
            Some(u) => u,
            None => return Ok(()), // Slack not configured
        };

        let severity_color = match payload.severity {
            AlertSeverity::Critical => "#dc3545",
            AlertSeverity::Warning => "#ffc107",
            AlertSeverity::Info => "#17a2b8",
        };

        let body = serde_json::json!({
            "attachments": [{
                "color": severity_color,
                "title": format!("{} {}", payload.alert_type.emoji(), payload.title),
                "text": payload.message,
                "footer": "rust-c2s-api",
                "ts": Utc::now().timestamp(),
            }]
        });

        self.http
            .post(url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Slack request failed: {}", e))?;

        tracing::info!("Slack alert sent: {}", payload.title);
        Ok(())
    }

    /// Send alert email via Resend API.
    async fn send_email(&self, payload: &AlertPayload) -> Result<(), String> {
        let api_key = match &self.resend_api_key {
            Some(k) => k,
            None => return Ok(()), // Email not configured
        };

        let severity_label = match payload.severity {
            AlertSeverity::Critical => "[CRITICAL]",
            AlertSeverity::Warning => "[WARNING]",
            AlertSeverity::Info => "[INFO]",
        };

        let subject = format!(
            "{} {} C2S Alert: {}",
            payload.alert_type.emoji(),
            severity_label,
            payload.title
        );

        let body = serde_json::json!({
            "from": self.alert_email_from,
            "to": [self.alert_email_to],
            "subject": subject,
            "text": payload.message,
        });

        self.http
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Resend email failed: {}", e))?;

        tracing::info!("Email alert sent: {}", subject);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_type_str() {
        assert_eq!(AlertType::HighValueLead.as_str(), "high_value_lead");
        assert_eq!(AlertType::ServiceDown.as_str(), "service_down");
        assert_eq!(AlertType::LowEnrichmentRate.as_str(), "low_enrichment_rate");
    }

    #[test]
    fn test_tracked_service_all() {
        assert_eq!(TrackedService::all().len(), 6);
    }

    #[test]
    fn test_alert_type_emoji() {
        assert_eq!(AlertType::HighValueLead.emoji(), "💎");
        assert_eq!(AlertType::ServiceDown.emoji(), "🚨");
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let svc = AlertService::new();
        // First call should pass
        assert!(svc.check_rate_limit("test_key").await);
        // Second call within 5 minutes should be blocked
        assert!(!svc.check_rate_limit("test_key").await);
        // Different key should pass
        assert!(svc.check_rate_limit("other_key").await);
    }

    #[tokio::test]
    async fn test_composite_rate_limit_key() {
        let svc = AlertService::new();
        // service_down:work_api and service_down:meilisearch are independent
        assert!(svc.check_rate_limit("service_down:work_api").await);
        assert!(svc.check_rate_limit("service_down:meilisearch").await);
        // But same key is blocked
        assert!(!svc.check_rate_limit("service_down:work_api").await);
    }

    #[tokio::test]
    async fn test_service_health_initial() {
        let svc = AlertService::new();
        let health = svc.get_service_health().await;
        assert!(health.all_healthy);
        assert_eq!(health.services.len(), 6);
        for s in &health.services {
            assert!(s.healthy);
            assert!(s.tracked);
            assert_eq!(s.consecutive_errors, 0);
        }
    }

    #[tokio::test]
    async fn test_service_health_after_errors() {
        let svc = AlertService::new();
        // Record 4 errors (below threshold)
        for _ in 0..4 {
            svc.record_service_status(TrackedService::WorkApi, false, Some("timeout"))
                .await;
        }
        let health = svc.get_service_health().await;
        assert!(health.all_healthy); // Still healthy (threshold is 5)

        // 5th error crosses threshold
        svc.record_service_status(TrackedService::WorkApi, false, Some("timeout"))
            .await;
        let health = svc.get_service_health().await;
        assert!(!health.all_healthy);

        let work_api = health
            .services
            .iter()
            .find(|s| s.service == TrackedService::WorkApi)
            .unwrap();
        assert!(!work_api.healthy);
        assert_eq!(work_api.consecutive_errors, 5);
    }

    #[tokio::test]
    async fn test_service_health_recovery() {
        let svc = AlertService::new();
        // 5 errors → unhealthy
        for _ in 0..5 {
            svc.record_service_status(TrackedService::C2S, false, Some("500"))
                .await;
        }
        assert!(!svc.get_service_health().await.all_healthy);

        // 1 success → healthy again
        svc.record_service_status(TrackedService::C2S, true, None)
            .await;
        assert!(svc.get_service_health().await.all_healthy);
    }
}
