//! Twenty CRM Integration — GraphQL client, workspace routing, SLA, delegation, intent signal
//!
//! Port of ts:src/services/twenty.service.ts

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// ─── Constants ──────────────────────────────────────────────────────────────

/// SLA hours by tier for first contact
fn sla_hours(tier: &str) -> f64 {
    match tier {
        "S" => 2.0,
        "A" => 24.0,
        "B" => 48.0,
        "C" | "Risk" => 72.0,
        _ => 72.0,
    }
}

/// Delegation expiry days by tier
fn delegation_expiry_days(tier: &str) -> i64 {
    match tier {
        "S" | "A" => 7,
        _ => 14,
    }
}

/// Route tier to workspace
fn tier_to_workspace(tier: &str) -> Workspace {
    match tier {
        "S" | "A" => Workspace::WsSenior,
        _ => Workspace::WsGeneral,
    }
}

// ─── Enums ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Workspace {
    WsOps,
    WsSenior,
    WsGeneral,
}

impl Workspace {
    pub fn label(&self) -> &str {
        match self {
            Self::WsOps => "Operations",
            Self::WsSenior => "Senior",
            Self::WsGeneral => "General",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadStatus {
    Novo,
    ContatoInicial,
    Qualificado,
    VisitaAgendada,
    VisitaRealizada,
    PropostaEnviada,
    Negociacao,
    FechadoGanho,
    FechadoPerdido,
    Nurturing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentSignal {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationReason {
    Training,
    Workload,
    Profile,
    Coverage,
}

// ─── Models ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwentyLeadInput {
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub source: Option<String>,
    pub tier: Option<String>,
    pub score: Option<i32>,
    pub cpf: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwentyLead {
    pub id: String,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub source: Option<String>,
    pub tier: Option<String>,
    pub score: Option<i32>,
    pub status: LeadStatus,
    pub workspace: Workspace,
    pub created_at: String,
    #[serde(default)]
    pub assigned_to: Option<String>,
    #[serde(default)]
    pub delegation: Option<DelegationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationInfo {
    pub from_workspace: Workspace,
    pub to_workspace: Workspace,
    pub reason: DelegationReason,
    pub delegated_at: String,
    pub expires_at: String,
    pub delegated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateInput {
    pub lead_id: String,
    pub to_workspace: Workspace,
    pub reason: DelegationReason,
    #[serde(default)]
    pub delegated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaCheck {
    pub within_sla: bool,
    pub hours_elapsed: f64,
    pub sla_hours: f64,
    pub tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSignalInput {
    pub source: Option<String>,
    pub last_contact_date: Option<String>,
    pub next_contact_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStats {
    pub total_leads: i64,
    pub by_tier: HashMap<String, i64>,
    pub by_status: HashMap<String, i64>,
    pub total_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerStats {
    pub broker_id: String,
    pub broker_name: String,
    pub total_leads: i64,
    pub sla_compliance: f64,
    pub conversion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaViolation {
    pub lead_id: String,
    pub lead_name: String,
    pub tier: String,
    pub hours_elapsed: f64,
    pub sla_hours: f64,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportInput {
    pub leads: Vec<TwentyLeadInput>,
    #[serde(default)]
    pub deduplicate_by: Option<String>, // "phone", "email", "cpf"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportResult {
    pub created: i64,
    pub skipped: i64,
    pub errors: i64,
    pub total: i64,
}

// ─── Service ────────────────────────────────────────────────────────────────

pub struct TwentyService {
    client: Client,
    base_url: String,
    api_keys: HashMap<Workspace, String>,
    enabled: bool,
}

impl TwentyService {
    pub fn new(
        base_url: &str,
        primary_key: &str,
        ws_ops_key: Option<&str>,
        ws_senior_key: Option<&str>,
        ws_general_key: Option<&str>,
        enabled: bool,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        let mut api_keys = HashMap::new();
        api_keys.insert(Workspace::WsOps, ws_ops_key.unwrap_or(primary_key).to_string());
        api_keys.insert(Workspace::WsSenior, ws_senior_key.unwrap_or(primary_key).to_string());
        api_keys.insert(Workspace::WsGeneral, ws_general_key.unwrap_or(primary_key).to_string());

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_keys,
            enabled,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn api_key_for(&self, ws: Workspace) -> &str {
        self.api_keys.get(&ws).map(|s| s.as_str()).unwrap_or_default()
    }

    /// Execute a GraphQL query against the given workspace
    async fn graphql(
        &self,
        workspace: Workspace,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        if !self.enabled {
            return Err("Twenty CRM is not enabled".to_string());
        }

        let url = format!("{}/graphql", self.base_url);
        let body = serde_json::json!({
            "query": query,
            "variables": variables.unwrap_or(serde_json::json!({})),
        });

        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key_for(workspace)))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("GraphQL request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Twenty API error {}: {}", status, body));
        }

        let result: serde_json::Value = resp.json().await
            .map_err(|e| format!("GraphQL parse error: {}", e))?;

        if let Some(errors) = result.get("errors").and_then(|e| e.as_array()) {
            if !errors.is_empty() {
                let msg = errors.iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(format!("GraphQL errors: {}", msg));
            }
        }

        Ok(result.get("data").cloned().unwrap_or(serde_json::json!(null)))
    }

    // ─── Lead CRUD ──────────────────────────────────────────────────────

    pub async fn create_lead(&self, input: &TwentyLeadInput) -> Result<TwentyLead, String> {
        let tier = input.tier.as_deref().unwrap_or("C");
        let workspace = tier_to_workspace(tier);

        let query = r#"
            mutation CreateLead($input: LeadCreateInput!) {
                createLead(data: $input) {
                    id name phone email source tier score status workspace createdAt assignedTo
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "name": input.name,
                "phone": input.phone,
                "email": input.email,
                "source": input.source,
                "tier": tier,
                "score": input.score,
                "cpf": input.cpf,
                "metadata": input.metadata,
            }
        });

        let data = self.graphql(workspace, query, Some(variables)).await?;
        let lead = data.get("createLead").ok_or("No createLead in response")?;

        Ok(TwentyLead {
            id: lead.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            name: input.name.clone(),
            phone: input.phone.clone(),
            email: input.email.clone(),
            source: input.source.clone(),
            tier: Some(tier.to_string()),
            score: input.score,
            status: LeadStatus::Novo,
            workspace,
            created_at: Utc::now().to_rfc3339(),
            assigned_to: None,
            delegation: None,
        })
    }

    pub async fn update_lead(
        &self,
        lead_id: &str,
        workspace: Workspace,
        updates: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let query = r#"
            mutation UpdateLead($id: ID!, $input: LeadUpdateInput!) {
                updateLead(id: $id, data: $input) {
                    id name tier score status
                }
            }
        "#;

        let variables = serde_json::json!({
            "id": lead_id,
            "input": updates,
        });

        self.graphql(workspace, query, Some(variables)).await
    }

    pub async fn get_lead(&self, lead_id: &str, workspace: Workspace) -> Result<Option<serde_json::Value>, String> {
        let query = r#"
            query GetLead($id: ID!) {
                lead(id: $id) {
                    id name phone email source tier score status workspace
                    createdAt assignedTo
                }
            }
        "#;

        let variables = serde_json::json!({ "id": lead_id });
        let data = self.graphql(workspace, query, Some(variables)).await?;
        Ok(data.get("lead").cloned())
    }

    /// Try all workspaces to find a lead
    pub async fn find_lead(&self, lead_id: &str) -> Result<Option<serde_json::Value>, String> {
        for ws in [Workspace::WsOps, Workspace::WsSenior, Workspace::WsGeneral] {
            if let Ok(Some(lead)) = self.get_lead(lead_id, ws).await {
                return Ok(Some(lead));
            }
        }
        Ok(None)
    }

    // ─── Workspace Routing ──────────────────────────────────────────────

    pub fn route_lead(&self, tier: &str) -> Workspace {
        tier_to_workspace(tier)
    }

    // ─── SLA Tracking ───────────────────────────────────────────────────

    pub fn check_sla(&self, tier: &str, created_at: &str) -> Result<SlaCheck, String> {
        let created: DateTime<Utc> = created_at.parse()
            .map_err(|e| format!("Invalid date: {}", e))?;
        let now = Utc::now();
        let elapsed = now.signed_duration_since(created);
        let elapsed_hours = elapsed.num_minutes() as f64 / 60.0;
        let sla = sla_hours(tier);

        Ok(SlaCheck {
            within_sla: elapsed_hours <= sla,
            hours_elapsed: (elapsed_hours * 10.0).round() / 10.0,
            sla_hours: sla,
            tier: tier.to_string(),
        })
    }

    // ─── Delegation ─────────────────────────────────────────────────────

    pub fn create_delegation(&self, input: &DelegateInput) -> DelegationInfo {
        let tier = "C"; // default — caller should provide actual tier
        let expiry_days = delegation_expiry_days(tier);
        let now = Utc::now();
        let expires = now + ChronoDuration::days(expiry_days);

        DelegationInfo {
            from_workspace: self.route_lead(tier),
            to_workspace: input.to_workspace,
            reason: input.reason,
            delegated_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            delegated_by: input.delegated_by.clone(),
        }
    }

    pub fn create_delegation_with_tier(&self, input: &DelegateInput, tier: &str) -> DelegationInfo {
        let expiry_days = delegation_expiry_days(tier);
        let now = Utc::now();
        let expires = now + ChronoDuration::days(expiry_days);

        DelegationInfo {
            from_workspace: self.route_lead(tier),
            to_workspace: input.to_workspace,
            reason: input.reason,
            delegated_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            delegated_by: input.delegated_by.clone(),
        }
    }

    pub fn is_delegation_expired(delegation: &DelegationInfo) -> bool {
        if let Ok(expires) = delegation.expires_at.parse::<DateTime<Utc>>() {
            Utc::now() > expires
        } else {
            true // treat parse errors as expired
        }
    }

    // ─── Intent Signal ──────────────────────────────────────────────────

    pub fn calculate_intent_signal(&self, input: &IntentSignalInput) -> IntentSignal {
        let is_paid = input.source.as_deref()
            .map(|s| matches!(s, "google_ads" | "facebook_ads" | "instagram_ads" | "paid"))
            .unwrap_or(false);

        let recent_contact = input.last_contact_date.as_ref()
            .and_then(|d| d.parse::<DateTime<Utc>>().ok())
            .map(|d| {
                let days = Utc::now().signed_duration_since(d).num_days();
                days <= 14
            })
            .unwrap_or(false);

        let has_follow_up = input.next_contact_date.is_some();

        if is_paid && recent_contact && has_follow_up {
            IntentSignal::High
        } else if recent_contact || has_follow_up {
            IntentSignal::Medium
        } else {
            IntentSignal::Low
        }
    }

    // ─── Pipeline & Broker Stats ────────────────────────────────────────

    pub async fn get_pipeline_stats(&self, workspace: Workspace) -> Result<PipelineStats, String> {
        let query = r#"
            query PipelineStats {
                leads(orderBy: { createdAt: DESC }, first: 1000) {
                    edges {
                        node { id tier status score }
                    }
                }
            }
        "#;

        let data = self.graphql(workspace, query, None).await?;
        let edges = data.get("leads")
            .and_then(|l| l.get("edges"))
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();

        let mut by_tier: HashMap<String, i64> = HashMap::new();
        let mut by_status: HashMap<String, i64> = HashMap::new();
        let mut total_value = 0.0;

        for edge in &edges {
            if let Some(node) = edge.get("node") {
                let tier = node.get("tier").and_then(|v| v.as_str()).unwrap_or("unknown");
                let status = node.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                let score = node.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);

                *by_tier.entry(tier.to_string()).or_default() += 1;
                *by_status.entry(status.to_string()).or_default() += 1;
                total_value += score;
            }
        }

        Ok(PipelineStats {
            total_leads: edges.len() as i64,
            by_tier,
            by_status,
            total_value,
        })
    }

    pub async fn get_broker_stats(&self, workspace: Workspace) -> Result<Vec<BrokerStats>, String> {
        let query = r#"
            query BrokerStats {
                leads(orderBy: { createdAt: DESC }, first: 1000) {
                    edges {
                        node { id assignedTo status tier createdAt }
                    }
                }
            }
        "#;

        let data = self.graphql(workspace, query, None).await?;
        let edges = data.get("leads")
            .and_then(|l| l.get("edges"))
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();

        let mut broker_map: HashMap<String, (i64, i64, i64)> = HashMap::new(); // total, sla_ok, converted

        for edge in &edges {
            if let Some(node) = edge.get("node") {
                let broker = node.get("assignedTo").and_then(|v| v.as_str()).unwrap_or("unassigned");
                let status = node.get("status").and_then(|v| v.as_str()).unwrap_or("");
                let tier = node.get("tier").and_then(|v| v.as_str()).unwrap_or("C");
                let created = node.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");

                let entry = broker_map.entry(broker.to_string()).or_default();
                entry.0 += 1; // total

                // Check SLA
                if let Ok(check) = self.check_sla(tier, created) {
                    if check.within_sla { entry.1 += 1; }
                }

                // Check conversion
                if status == "fechado_ganho" { entry.2 += 1; }
            }
        }

        Ok(broker_map.into_iter().map(|(name, (total, sla_ok, converted))| {
            BrokerStats {
                broker_id: name.clone(),
                broker_name: name,
                total_leads: total,
                sla_compliance: if total > 0 { (sla_ok as f64 / total as f64 * 100.0).round() } else { 0.0 },
                conversion_rate: if total > 0 { (converted as f64 / total as f64 * 100.0).round() } else { 0.0 },
            }
        }).collect())
    }

    // ─── SLA Violation Detection ────────────────────────────────────────

    pub async fn check_sla_violations(&self, workspace: Workspace) -> Result<Vec<SlaViolation>, String> {
        let query = r#"
            query SlaCheck {
                leads(filter: { status: { eq: "novo" } }, orderBy: { createdAt: ASC }, first: 500) {
                    edges {
                        node { id name tier createdAt assignedTo }
                    }
                }
            }
        "#;

        let data = self.graphql(workspace, query, None).await?;
        let edges = data.get("leads")
            .and_then(|l| l.get("edges"))
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();

        let mut violations = Vec::new();

        for edge in &edges {
            if let Some(node) = edge.get("node") {
                let tier = node.get("tier").and_then(|v| v.as_str()).unwrap_or("C");
                let created = node.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");

                if let Ok(check) = self.check_sla(tier, created) {
                    if !check.within_sla {
                        violations.push(SlaViolation {
                            lead_id: node.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            lead_name: node.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            tier: tier.to_string(),
                            hours_elapsed: check.hours_elapsed,
                            sla_hours: check.sla_hours,
                            assigned_to: node.get("assignedTo").and_then(|v| v.as_str()).map(String::from),
                        });
                    }
                }
            }
        }

        Ok(violations)
    }

    // ─── Bulk Import ────────────────────────────────────────────────────

    pub async fn bulk_import(&self, input: &BulkImportInput) -> Result<BulkImportResult, String> {
        let dedup_field = input.deduplicate_by.as_deref().unwrap_or("phone");
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut created: i64 = 0;
        let mut skipped: i64 = 0;
        let mut errors: i64 = 0;

        for lead in &input.leads {
            // Deduplication
            let key = match dedup_field {
                "email" => lead.email.clone().unwrap_or_default(),
                "cpf" => lead.cpf.clone().unwrap_or_default(),
                _ => lead.phone.clone().unwrap_or_default(),
            };

            if key.is_empty() || seen.contains(&key) {
                skipped += 1;
                continue;
            }
            seen.insert(key);

            match self.create_lead(lead).await {
                Ok(_) => created += 1,
                Err(e) => {
                    tracing::warn!("Bulk import error for {}: {}", lead.name, e);
                    errors += 1;
                }
            }
        }

        Ok(BulkImportResult {
            created,
            skipped,
            errors,
            total: input.leads.len() as i64,
        })
    }

    // ─── Next Action ────────────────────────────────────────────────────

    pub fn get_next_action(&self, status: &str, tier: &str) -> serde_json::Value {
        let (action, priority, reason) = match status {
            "novo" => match tier {
                "S" => ("Contato imediato", "critical", "Lead Platinum — SLA 2h"),
                "A" => ("Contato prioritário", "high", "Lead Gold — SLA 24h"),
                _ => ("Contato inicial", "medium", "Novo lead aguardando primeiro contato"),
            },
            "contato_inicial" => ("Qualificar lead", "medium", "Lead já contactado, qualificar interesse"),
            "qualificado" => ("Agendar visita", "medium", "Lead qualificado sem visita"),
            "visita_agendada" => ("Confirmar visita", "high", "Visita agendada, confirmar presença"),
            "visita_realizada" => ("Enviar proposta", "high", "Visita realizada, enviar proposta"),
            "proposta_enviada" => ("Follow-up proposta", "medium", "Proposta enviada, acompanhar"),
            "negociacao" => ("Negociar termos", "high", "Em negociação ativa"),
            "nurturing" => ("Reengajar", "low", "Lead em nurturing, reengajar periodicamente"),
            _ => ("Verificar status", "low", "Status desconhecido"),
        };

        serde_json::json!({
            "action": action,
            "priority": priority,
            "reason": reason,
            "current_status": status,
            "tier": tier,
        })
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> TwentyService {
        TwentyService::new(
            "https://twenty.example.com",
            "test-key",
            None, None, None,
            false, // disabled for unit tests
        )
    }

    #[test]
    fn test_sla_hours() {
        assert_eq!(sla_hours("S"), 2.0);
        assert_eq!(sla_hours("A"), 24.0);
        assert_eq!(sla_hours("B"), 48.0);
        assert_eq!(sla_hours("C"), 72.0);
        assert_eq!(sla_hours("Risk"), 72.0);
        assert_eq!(sla_hours("X"), 72.0); // unknown defaults to 72
    }

    #[test]
    fn test_delegation_expiry_days() {
        assert_eq!(delegation_expiry_days("S"), 7);
        assert_eq!(delegation_expiry_days("A"), 7);
        assert_eq!(delegation_expiry_days("B"), 14);
        assert_eq!(delegation_expiry_days("C"), 14);
    }

    #[test]
    fn test_tier_to_workspace() {
        assert_eq!(tier_to_workspace("S"), Workspace::WsSenior);
        assert_eq!(tier_to_workspace("A"), Workspace::WsSenior);
        assert_eq!(tier_to_workspace("B"), Workspace::WsGeneral);
        assert_eq!(tier_to_workspace("C"), Workspace::WsGeneral);
        assert_eq!(tier_to_workspace("Risk"), Workspace::WsGeneral);
    }

    #[test]
    fn test_workspace_label() {
        assert_eq!(Workspace::WsOps.label(), "Operations");
        assert_eq!(Workspace::WsSenior.label(), "Senior");
        assert_eq!(Workspace::WsGeneral.label(), "General");
    }

    #[test]
    fn test_check_sla_within() {
        let svc = test_service();
        // Created 1 hour ago — S tier SLA is 2h
        let created = (Utc::now() - ChronoDuration::hours(1)).to_rfc3339();
        let check = svc.check_sla("S", &created).unwrap();
        assert!(check.within_sla);
        assert_eq!(check.sla_hours, 2.0);
        assert!(check.hours_elapsed < 2.0);
    }

    #[test]
    fn test_check_sla_violated() {
        let svc = test_service();
        // Created 3 hours ago — S tier SLA is 2h
        let created = (Utc::now() - ChronoDuration::hours(3)).to_rfc3339();
        let check = svc.check_sla("S", &created).unwrap();
        assert!(!check.within_sla);
        assert!(check.hours_elapsed > 2.0);
    }

    #[test]
    fn test_check_sla_a_tier() {
        let svc = test_service();
        let created = (Utc::now() - ChronoDuration::hours(12)).to_rfc3339();
        let check = svc.check_sla("A", &created).unwrap();
        assert!(check.within_sla); // 12h < 24h
        assert_eq!(check.sla_hours, 24.0);
    }

    #[test]
    fn test_intent_signal_high() {
        let svc = test_service();
        let signal = svc.calculate_intent_signal(&IntentSignalInput {
            source: Some("google_ads".to_string()),
            last_contact_date: Some((Utc::now() - ChronoDuration::days(3)).to_rfc3339()),
            next_contact_date: Some((Utc::now() + ChronoDuration::days(2)).to_rfc3339()),
        });
        assert_eq!(signal, IntentSignal::High);
    }

    #[test]
    fn test_intent_signal_medium_recent() {
        let svc = test_service();
        let signal = svc.calculate_intent_signal(&IntentSignalInput {
            source: Some("organic".to_string()),
            last_contact_date: Some((Utc::now() - ChronoDuration::days(5)).to_rfc3339()),
            next_contact_date: None,
        });
        assert_eq!(signal, IntentSignal::Medium);
    }

    #[test]
    fn test_intent_signal_medium_followup() {
        let svc = test_service();
        let signal = svc.calculate_intent_signal(&IntentSignalInput {
            source: None,
            last_contact_date: None,
            next_contact_date: Some((Utc::now() + ChronoDuration::days(5)).to_rfc3339()),
        });
        assert_eq!(signal, IntentSignal::Medium);
    }

    #[test]
    fn test_intent_signal_low() {
        let svc = test_service();
        let signal = svc.calculate_intent_signal(&IntentSignalInput {
            source: Some("organic".to_string()),
            last_contact_date: Some((Utc::now() - ChronoDuration::days(30)).to_rfc3339()),
            next_contact_date: None,
        });
        assert_eq!(signal, IntentSignal::Low);
    }

    #[test]
    fn test_delegation_creation_sa_tier() {
        let svc = test_service();
        let delegation = svc.create_delegation_with_tier(&DelegateInput {
            lead_id: "lead-1".to_string(),
            to_workspace: Workspace::WsGeneral,
            reason: DelegationReason::Training,
            delegated_by: Some("manager-1".to_string()),
        }, "S");

        assert_eq!(delegation.from_workspace, Workspace::WsSenior);
        assert_eq!(delegation.to_workspace, Workspace::WsGeneral);
        assert_eq!(delegation.reason, DelegationReason::Training);
        // S tier: 7 days expiry
        let delegated: DateTime<Utc> = delegation.delegated_at.parse().unwrap();
        let expires: DateTime<Utc> = delegation.expires_at.parse().unwrap();
        let days = expires.signed_duration_since(delegated).num_days();
        assert_eq!(days, 7);
    }

    #[test]
    fn test_delegation_creation_b_tier() {
        let svc = test_service();
        let delegation = svc.create_delegation_with_tier(&DelegateInput {
            lead_id: "lead-2".to_string(),
            to_workspace: Workspace::WsGeneral,
            reason: DelegationReason::Workload,
            delegated_by: None,
        }, "B");

        // B tier: 14 days expiry
        let delegated: DateTime<Utc> = delegation.delegated_at.parse().unwrap();
        let expires: DateTime<Utc> = delegation.expires_at.parse().unwrap();
        let days = expires.signed_duration_since(delegated).num_days();
        assert_eq!(days, 14);
    }

    #[test]
    fn test_delegation_expired() {
        let expired = DelegationInfo {
            from_workspace: Workspace::WsSenior,
            to_workspace: Workspace::WsGeneral,
            reason: DelegationReason::Coverage,
            delegated_at: (Utc::now() - ChronoDuration::days(10)).to_rfc3339(),
            expires_at: (Utc::now() - ChronoDuration::days(1)).to_rfc3339(), // yesterday
            delegated_by: None,
        };
        assert!(TwentyService::is_delegation_expired(&expired));
    }

    #[test]
    fn test_delegation_not_expired() {
        let active = DelegationInfo {
            from_workspace: Workspace::WsSenior,
            to_workspace: Workspace::WsGeneral,
            reason: DelegationReason::Profile,
            delegated_at: Utc::now().to_rfc3339(),
            expires_at: (Utc::now() + ChronoDuration::days(5)).to_rfc3339(), // 5 days from now
            delegated_by: None,
        };
        assert!(!TwentyService::is_delegation_expired(&active));
    }

    #[test]
    fn test_next_action_novo_s_tier() {
        let svc = test_service();
        let action = svc.get_next_action("novo", "S");
        assert_eq!(action["action"], "Contato imediato");
        assert_eq!(action["priority"], "critical");
    }

    #[test]
    fn test_next_action_novo_c_tier() {
        let svc = test_service();
        let action = svc.get_next_action("novo", "C");
        assert_eq!(action["action"], "Contato inicial");
        assert_eq!(action["priority"], "medium");
    }

    #[test]
    fn test_next_action_qualificado() {
        let svc = test_service();
        let action = svc.get_next_action("qualificado", "A");
        assert_eq!(action["action"], "Agendar visita");
    }

    #[test]
    fn test_next_action_visita_realizada() {
        let svc = test_service();
        let action = svc.get_next_action("visita_realizada", "B");
        assert_eq!(action["action"], "Enviar proposta");
        assert_eq!(action["priority"], "high");
    }

    #[test]
    fn test_next_action_negociacao() {
        let svc = test_service();
        let action = svc.get_next_action("negociacao", "A");
        assert_eq!(action["action"], "Negociar termos");
    }

    #[test]
    fn test_next_action_nurturing() {
        let svc = test_service();
        let action = svc.get_next_action("nurturing", "C");
        assert_eq!(action["action"], "Reengajar");
        assert_eq!(action["priority"], "low");
    }

    #[test]
    fn test_route_lead() {
        let svc = test_service();
        assert_eq!(svc.route_lead("S"), Workspace::WsSenior);
        assert_eq!(svc.route_lead("A"), Workspace::WsSenior);
        assert_eq!(svc.route_lead("B"), Workspace::WsGeneral);
        assert_eq!(svc.route_lead("Risk"), Workspace::WsGeneral);
    }

    #[test]
    fn test_lead_input_serialization() {
        let input = TwentyLeadInput {
            name: "João Silva".to_string(),
            phone: Some("11999887766".to_string()),
            email: Some("joao@test.com".to_string()),
            source: Some("google_ads".to_string()),
            tier: Some("A".to_string()),
            score: Some(85),
            cpf: Some("12345678901".to_string()),
            metadata: None,
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["name"], "João Silva");
        assert_eq!(json["tier"], "A");
        assert_eq!(json["score"], 85);
    }

    #[test]
    fn test_bulk_import_dedup_logic() {
        // Test the dedup key extraction logic
        let leads = vec![
            TwentyLeadInput {
                name: "Lead 1".to_string(),
                phone: Some("11111".to_string()),
                email: None, source: None, tier: None, score: None, cpf: None, metadata: None,
            },
            TwentyLeadInput {
                name: "Lead 2 (dup)".to_string(),
                phone: Some("11111".to_string()), // same phone
                email: None, source: None, tier: None, score: None, cpf: None, metadata: None,
            },
            TwentyLeadInput {
                name: "Lead 3".to_string(),
                phone: Some("22222".to_string()),
                email: None, source: None, tier: None, score: None, cpf: None, metadata: None,
            },
        ];

        // Simulate dedup
        let dedup_field = "phone";
        let mut seen = std::collections::HashSet::new();
        let mut unique = 0;
        let mut skipped = 0;
        for lead in &leads {
            let key = match dedup_field {
                "phone" => lead.phone.clone().unwrap_or_default(),
                _ => String::new(),
            };
            if key.is_empty() || seen.contains(&key) {
                skipped += 1;
            } else {
                seen.insert(key);
                unique += 1;
            }
        }
        assert_eq!(unique, 2);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn test_delegation_reasons() {
        for (reason, expected) in [
            (DelegationReason::Training, "training"),
            (DelegationReason::Workload, "workload"),
            (DelegationReason::Profile, "profile"),
            (DelegationReason::Coverage, "coverage"),
        ] {
            let json = serde_json::to_value(reason).unwrap();
            assert_eq!(json.as_str().unwrap(), expected);
        }
    }
}
