//! MCP Server for rust-c2s-api — 66 tools + 3 resources
//! Port of ts-c2s-api/src/mcp/ to Rust using rmcp crate.

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::*,
    service::RequestContext,
    schemars,
};
use serde_json::{json, Value};
use std::sync::Arc;
use sqlx::PgPool;

use crate::config::Config;
use crate::db_storage::EnrichmentStorage;
use crate::discovery::CpfDiscoveryService;
use crate::services::{WorkApiService, C2SService};
use crate::meilisearch::MeilisearchCompanyService;
use crate::fly_scale::FlyScaleService;
use crate::c2s_extended::C2sExtendedService;
use crate::ibvi_property::IbviPropertyService;
use crate::twenty::TwentyService;
use crate::web_search::WebSearchService;
use crate::lead_analysis::LeadAnalysisService;
use crate::report::ProfileReportService;

// ─── MCP Application State (composition root for MCP binary) ────────

/// Lean state for the MCP stdio binary. Separate from `handlers::AppState`
/// which holds Moka caches, sessions, and gateway client for the Axum HTTP server.
pub struct McpAppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub discovery: CpfDiscoveryService,
    pub storage: EnrichmentStorage,
    pub work_api: WorkApiService,
    pub c2s: C2SService,
    pub meilisearch: Arc<MeilisearchCompanyService>,
    pub fly_scale: Arc<FlyScaleService>,
    pub c2s_extended: Arc<C2sExtendedService>,
    pub ibvi_property: Arc<IbviPropertyService>,
    pub twenty: Arc<TwentyService>,
    pub web_search: Arc<WebSearchService>,
    pub lead_analysis: Arc<LeadAnalysisService>,
    pub report: ProfileReportService,
}

impl McpAppState {
    /// Single composition root — all services initialized here.
    pub fn new(config: &Config, db: PgPool) -> Self {
        Self {
            db: db.clone(),
            config: Arc::new(config.clone()),
            discovery: CpfDiscoveryService::new(config),
            storage: EnrichmentStorage::new(db.clone()),
            work_api: WorkApiService::new(config),
            c2s: C2SService::new(config),
            meilisearch: Arc::new(MeilisearchCompanyService::new(
                &config.meilisearch_url,
                &config.meilisearch_key,
            )),
            fly_scale: Arc::new(FlyScaleService::new(config)),
            c2s_extended: Arc::new(C2sExtendedService::new(
                &config.c2s_base_url,
                &config.c2s_token,
            )),
            ibvi_property: Arc::new(IbviPropertyService::new(db.clone())),
            twenty: Arc::new(TwentyService::new(
                &config.twenty_base_url,
                &config.twenty_api_key,
                config.twenty_api_key_ws_ops.as_deref(),
                config.twenty_api_key_ws_senior.as_deref(),
                config.twenty_api_key_ws_general.as_deref(),
                config.twenty_enabled,
            )),
            web_search: Arc::new(WebSearchService::new()),
            lead_analysis: Arc::new(LeadAnalysisService::new(db)),
            report: ProfileReportService::new(),
        }
    }
}

/// Helper macro: extract `&McpAppState` from `self.state`, or return stub with tool name.
macro_rules! require_state {
    ($self:expr, $tool_name:expr) => {
        match &$self.state {
            Some(s) => s,
            None => return $self.stub_tool($tool_name, &Value::Null),
        }
    };
}

// ─── Tool Input Schemas ─────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EnrichLeadInput {
    #[schemars(description = "Phone number to enrich")]
    pub phone: Option<String>,
    #[schemars(description = "Email to enrich")]
    pub email: Option<String>,
    #[schemars(description = "Name of the person")]
    pub name: Option<String>,
    #[schemars(description = "C2S lead ID")]
    pub lead_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EnrichBulkInput {
    #[schemars(description = "Array of leads to enrich")]
    pub leads: Vec<EnrichLeadInput>,
    #[schemars(description = "Batch size (default: 10)")]
    pub batch_size: Option<u32>,
    #[schemars(description = "Delay between batches in ms (default: 2000)")]
    pub delay_ms: Option<u64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RetryFailedInput {
    #[schemars(description = "Specific lead IDs to retry")]
    pub lead_ids: Option<Vec<String>>,
    #[schemars(description = "Max leads to retry (default: 50)")]
    pub limit: Option<u32>,
    #[schemars(description = "Statuses to retry (default: failed, partial, unenriched)")]
    pub statuses: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FindAndSavePersonInput {
    #[schemars(description = "Phone number (required)")]
    pub phone: String,
    #[schemars(description = "Person name")]
    pub name: Option<String>,
    #[schemars(description = "Email address")]
    pub email: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DiscoverCpfInput {
    #[schemars(description = "Phone number")]
    pub phone: Option<String>,
    #[schemars(description = "Email address")]
    pub email: Option<String>,
    #[schemars(description = "Person name")]
    pub name: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LookupCpfInput {
    #[schemars(description = "CPF number (11 digits)")]
    pub cpf: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchCpfByNameInput {
    #[schemars(description = "Name to search (min 5 chars)")]
    pub name: String,
    #[schemars(description = "Max results (default: 10)")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ValidateCpfInput {
    #[schemars(description = "CPF to validate")]
    pub cpf: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetLeadInput {
    #[schemars(description = "Lead ID")]
    pub lead_id: Option<String>,
    #[schemars(description = "Phone number")]
    pub phone: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListLeadsInput {
    #[schemars(description = "Filter by status")]
    pub status: Option<String>,
    #[schemars(description = "Filter by seller ID")]
    pub seller_id: Option<String>,
    #[schemars(description = "Max results (default: 20, max: 100)")]
    pub limit: Option<u32>,
    #[schemars(description = "Offset for pagination")]
    pub offset: Option<u32>,
    #[schemars(description = "Filter from date (ISO 8601)")]
    pub date_from: Option<String>,
    #[schemars(description = "Filter to date (ISO 8601)")]
    pub date_to: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LeadIdInput {
    #[schemars(description = "C2S lead ID")]
    pub lead_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct StatsInput {
    #[schemars(description = "Number of days to look back (default: 7)")]
    pub days: Option<u32>,
    #[schemars(description = "Group by: day, seller, or source")]
    pub group_by: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CpfInput {
    #[schemars(description = "CPF number")]
    pub cpf: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReportPersonInput {
    #[schemars(description = "CPF")]
    pub cpf: Option<String>,
    #[schemars(description = "Name")]
    pub name: Option<String>,
    #[schemars(description = "Occupation")]
    pub occupation: Option<String>,
    #[schemars(description = "Company")]
    pub company: Option<String>,
    #[schemars(description = "Income")]
    pub income: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GenerateReportInput {
    #[schemars(description = "Person data for the report")]
    pub persons: Vec<ReportPersonInput>,
    #[schemars(description = "Report title")]
    pub title: String,
    #[schemars(description = "Report subtitle")]
    pub subtitle: Option<String>,
    #[schemars(description = "Format: md, html (default: md)")]
    pub format: Option<String>,
    #[schemars(description = "Include contact info (default: true)")]
    pub include_contacts: Option<bool>,
    #[schemars(description = "Include income data (default: true)")]
    pub include_income: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GenerateReportFromCpfsInput {
    #[schemars(description = "CPFs to look up and generate report for")]
    pub cpfs: Vec<String>,
    #[schemars(description = "Report title")]
    pub title: String,
    #[schemars(description = "Report subtitle")]
    pub subtitle: Option<String>,
    #[schemars(description = "Format: md, html, pdf (default: md)")]
    pub format: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeLeadInput {
    #[schemars(description = "Lead ID")]
    pub lead_id: String,
    #[schemars(description = "Person name")]
    pub name: String,
    #[schemars(description = "Email")]
    pub email: Option<String>,
    #[schemars(description = "Phone")]
    pub phone: Option<String>,
    #[schemars(description = "Income")]
    pub income: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ScoreLeadInput {
    #[schemars(description = "Person name")]
    pub name: Option<String>,
    #[schemars(description = "Phone")]
    pub phone: Option<String>,
    #[schemars(description = "Email")]
    pub email: Option<String>,
    #[schemars(description = "CPF")]
    pub cpf: Option<String>,
    #[schemars(description = "Enriched name from Work API")]
    pub enriched_name: Option<String>,
    #[schemars(description = "Monthly income")]
    pub income: Option<f64>,
    #[schemars(description = "Number of companies")]
    pub company_count: Option<u32>,
    #[schemars(description = "Total company capital")]
    pub total_company_capital: Option<f64>,
    #[schemars(description = "Is company administrator")]
    pub is_company_administrator: Option<bool>,
    #[schemars(description = "Has real estate sector company")]
    pub has_real_estate_sector: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RiskAssessInput {
    #[schemars(description = "Person name (min 3 chars)")]
    pub name: String,
    #[schemars(description = "Email")]
    pub email: Option<String>,
    #[schemars(description = "Phone")]
    pub phone: Option<String>,
    #[schemars(description = "Company")]
    pub company: Option<String>,
    #[schemars(description = "CPF")]
    pub cpf: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct NameInput {
    #[schemars(description = "Person name (min 3 chars)")]
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FetchC2sLeadsInput {
    #[schemars(description = "Page number (default: 1)")]
    pub page: Option<u32>,
    #[schemars(description = "Per page (max: 50, default: 20)")]
    pub perpage: Option<u32>,
    #[schemars(description = "Filter by status")]
    pub status: Option<String>,
    #[schemars(description = "Phone to search")]
    pub phone: Option<String>,
    #[schemars(description = "Email to search")]
    pub email: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SendMessageInput {
    #[schemars(description = "C2S lead ID")]
    pub lead_id: String,
    #[schemars(description = "Message body")]
    pub message: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ForwardLeadInput {
    #[schemars(description = "C2S lead ID")]
    pub lead_id: String,
    #[schemars(description = "Target seller ID")]
    pub seller_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PhoneInput {
    #[schemars(description = "Phone number to search")]
    pub phone: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EmailInput {
    #[schemars(description = "Email address")]
    pub email: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TagInput {
    #[schemars(description = "Tag name filter")]
    pub name: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddTagInput {
    #[schemars(description = "C2S lead ID")]
    pub lead_id: String,
    #[schemars(description = "Tag ID to add")]
    pub tag_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CnpjInput {
    #[schemars(description = "CNPJ number (14 digits)")]
    pub cnpj: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CompanySearchInput {
    #[schemars(description = "Company name or CNPJ to search")]
    pub query: String,
    #[schemars(description = "Max results (default: 20)")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TierCalcInput {
    #[schemars(description = "Person name")]
    pub name: String,
    #[schemars(description = "Phone")]
    pub phone: Option<String>,
    #[schemars(description = "Email")]
    pub email: Option<String>,
    #[schemars(description = "Monthly income")]
    pub income: Option<f64>,
    #[schemars(description = "Neighborhood")]
    pub neighborhood: Option<String>,
    #[schemars(description = "Number of properties owned")]
    pub property_count: Option<u32>,
    #[schemars(description = "Company name")]
    pub company: Option<String>,
    #[schemars(description = "Has risk flags")]
    pub has_risk_flags: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TierInput {
    #[schemars(description = "Tier: platinum, gold, silver, bronze, risk")]
    pub tier: String,
    #[schemars(description = "Additional context")]
    pub context: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WebSearchInput {
    #[schemars(description = "Search query")]
    pub query: String,
    #[schemars(description = "Number of results (max 10, default: 5)")]
    pub num_results: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchPersonInput {
    #[schemars(description = "Person name")]
    pub name: String,
    #[schemars(description = "Location (default: São Paulo)")]
    pub location: Option<String>,
    #[schemars(description = "Company name")]
    pub company: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WebInsightsInput {
    #[schemars(description = "Lead ID")]
    pub lead_id: Option<String>,
    #[schemars(description = "Person name")]
    pub name: String,
    #[schemars(description = "Enriched name")]
    pub enriched_name: Option<String>,
    #[schemars(description = "Phone")]
    pub phone: Option<String>,
    #[schemars(description = "Email")]
    pub email: Option<String>,
    #[schemars(description = "Income")]
    pub income: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeLeadNameInput {
    #[schemars(description = "Person name")]
    pub name: String,
    #[schemars(description = "Phone number")]
    pub phone: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ThresholdInput {
    #[schemars(description = "Rate threshold percentage (default: 80)")]
    pub threshold: Option<f64>,
}

// Twenty CRM inputs
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyCreateLeadInput {
    #[schemars(description = "Lead name")]
    pub name: String,
    #[schemars(description = "Phone number")]
    pub phone: String,
    #[schemars(description = "Lead source")]
    pub source: String,
    #[schemars(description = "Email")]
    pub email: Option<String>,
    #[schemars(description = "CPF")]
    pub cpf: Option<String>,
    #[schemars(description = "Tier: S, A, B, C, Risk")]
    pub tier: Option<String>,
    #[schemars(description = "Quality score 0-100")]
    pub score: Option<u32>,
    #[schemars(description = "Monthly income")]
    pub income: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyUpdateLeadInput {
    #[schemars(description = "Lead ID")]
    pub id: String,
    #[schemars(description = "Name")]
    pub name: Option<String>,
    #[schemars(description = "Email")]
    pub email: Option<String>,
    #[schemars(description = "Phone")]
    pub phone: Option<String>,
    #[schemars(description = "Lead status")]
    pub lead_status: Option<String>,
    #[schemars(description = "Tier")]
    pub tier: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyGetLeadInput {
    #[schemars(description = "Lead ID")]
    pub id: String,
    #[schemars(description = "Workspace: WS-OPS, WS-SENIOR, WS-GENERAL")]
    pub workspace: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyRouteInput {
    #[schemars(description = "Lead ID")]
    pub lead_id: String,
    #[schemars(description = "Lead tier (S, A, B, C, Risk)")]
    pub tier: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyDelegateInput {
    #[schemars(description = "Lead ID")]
    pub lead_id: String,
    #[schemars(description = "Who is delegating")]
    pub delegated_by: String,
    #[schemars(description = "Reason for delegation")]
    pub delegated_reason: String,
    #[schemars(description = "Target broker")]
    pub target_broker: Option<String>,
    #[schemars(description = "Days until expiration")]
    pub expiration_days: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyBulkInput {
    #[schemars(description = "Leads to import")]
    pub leads: Vec<TwentyCreateLeadInput>,
    #[schemars(description = "Skip duplicates (default: true)")]
    pub skip_duplicates: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyStatsInput {
    #[schemars(description = "Workspace filter")]
    pub workspace: Option<String>,
    #[schemars(description = "Date from (ISO 8601)")]
    pub date_from: Option<String>,
    #[schemars(description = "Date to (ISO 8601)")]
    pub date_to: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentySlaInput {
    #[schemars(description = "Workspace filter")]
    pub workspace: Option<String>,
    #[schemars(description = "Tier filter")]
    pub tier_filter: Option<String>,
    #[schemars(description = "Max results")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyDelegationExpiryInput {
    #[schemars(description = "Days ahead to check (default: 7)")]
    pub days_ahead: Option<u32>,
    #[schemars(description = "Workspace filter")]
    pub workspace: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyIntentInput {
    #[schemars(description = "Lead ID")]
    pub lead_id: Option<String>,
    #[schemars(description = "Lead source")]
    pub source: Option<String>,
    #[schemars(description = "Last contact date (ISO 8601)")]
    pub last_contact_date: Option<String>,
    #[schemars(description = "Next contact date (ISO 8601)")]
    pub next_contact_date: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyNextActionInput {
    #[schemars(description = "Lead ID")]
    pub lead_id: Option<String>,
    #[schemars(description = "Current lead status")]
    pub lead_status: String,
    #[schemars(description = "Lead tier")]
    pub tier: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TwentyBrokerStatsInput {
    #[schemars(description = "Broker ID")]
    pub broker_id: Option<String>,
    #[schemars(description = "Workspace filter")]
    pub workspace: Option<String>,
    #[schemars(description = "Period filter")]
    pub period: Option<String>,
}

// ─── MCP Server ─────────────────────────────────────────────────────

/// MCP Server for C2S Lead Enrichment API
///
/// Exposes 66 tools and 3 resources for AI assistant integration.
/// `state: None` = stub mode (pure-logic tools only, for tests).
/// `state: Some(...)` = fully wired (DB + services).
#[derive(Clone)]
pub struct McpServer {
    config: Arc<Config>,
    state: Option<Arc<McpAppState>>,
}

impl McpServer {
    /// Stub mode — no DB, no services. Pure-logic tools only.
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            state: None,
        }
    }

    /// Fully wired mode — DB + all services available.
    pub fn with_state(config: Config, state: McpAppState) -> Self {
        Self {
            config: Arc::new(config),
            state: Some(Arc::new(state)),
        }
    }

    // ─── Tool definitions ──────────────────────────────────────────

    fn tool_definitions() -> Vec<Tool> {
        vec![
            // Enrichment (3)
            Self::define_tool::<EnrichLeadInput>("enrich_lead", "Enrich a single lead by phone, email, or name with full CPF discovery"),
            Self::define_tool::<EnrichBulkInput>("enrich_bulk", "Batch enrichment of multiple leads with rate limiting"),
            Self::define_tool::<RetryFailedInput>("retry_failed", "Retry failed or partial enrichments"),

            // Discovery (5)
            Self::define_tool::<FindAndSavePersonInput>("find_and_save_person", "Find person by phone, fetch full data, save to PostgreSQL"),
            Self::define_tool::<DiscoverCpfInput>("discover_cpf", "Find CPF using multi-tier discovery (Work API, DuckDB, Diretrix, DBase)"),
            Self::define_tool::<LookupCpfInput>("lookup_cpf", "Get full data for a known CPF from Work API"),
            Self::define_tool::<SearchCpfByNameInput>("search_cpf_by_name", "Search 223M CPF database by name"),
            Self::define_tool::<ValidateCpfInput>("validate_cpf", "Validate CPF format with mod-11 check"),

            // Leads (3)
            Self::define_tool::<GetLeadInput>("get_lead", "Get lead details by ID or phone"),
            Self::define_tool::<ListLeadsInput>("list_leads", "List leads with filters (status, seller, date range)"),
            Self::define_tool::<LeadIdInput>("get_c2s_lead_status", "Get full C2S lead record including messages"),

            // Stats (4)
            Self::define_tool::<StatsInput>("get_enrichment_stats", "Enrichment statistics with grouping options"),
            Self::define_tool::<serde_json::Value>("get_service_health", "Health status of all services"),
            Self::define_tool::<serde_json::Value>("get_enrichment_rate", "Current enrichment rate"),
            Self::define_tool::<ThresholdInput>("get_enrichment_health", "Health status vs threshold"),

            // Property (3)
            Self::define_tool::<CpfInput>("get_properties_by_cpf", "Find all properties owned by CPF in IBVI database"),
            Self::define_tool::<CpfInput>("get_property_summary", "Aggregated property portfolio summary"),
            Self::define_tool::<CpfInput>("format_property_message", "Format properties for C2S message"),

            // Reports (3)
            Self::define_tool::<GenerateReportInput>("generate_profile_report", "Generate profile report (MD/HTML)"),
            Self::define_tool::<GenerateReportFromCpfsInput>("generate_report_from_cpfs", "Lookup CPFs, enrich, and generate report"),
            Self::define_tool::<GenerateReportInput>("generate_report_pdf", "Generate PDF report"),

            // Analysis (6)
            Self::define_tool::<AnalyzeLeadInput>("analyze_lead", "Deep analysis with web search, risk detection, tier calculation"),
            Self::define_tool::<LeadIdInput>("get_lead_analysis", "Retrieve cached analysis from database"),
            Self::define_tool::<LeadIdInput>("check_lead_alert", "Check if lead should trigger premium/risk alert"),
            Self::define_tool::<ScoreLeadInput>("score_lead_quality", "Calculate 0-100 quality score with breakdown"),
            Self::define_tool::<RiskAssessInput>("assess_risk", "Full risk assessment with negative news search"),
            Self::define_tool::<NameInput>("quick_risk_check", "Fast check against known risks database"),

            // C2S CRM (9)
            Self::define_tool::<FetchC2sLeadsInput>("fetch_c2s_leads", "Fetch leads directly from C2S with filters"),
            Self::define_tool::<serde_json::Value>("get_c2s_sellers", "List all sellers in C2S"),
            Self::define_tool::<SendMessageInput>("send_c2s_message", "Add a message/note to a lead"),
            Self::define_tool::<ForwardLeadInput>("forward_c2s_lead", "Forward a lead to another seller"),
            Self::define_tool::<PhoneInput>("search_c2s_by_phone", "Find lead by phone in C2S"),
            Self::define_tool::<EmailInput>("search_c2s_by_email", "Find lead by email in C2S"),
            Self::define_tool::<LeadIdInput>("mark_c2s_interacted", "Mark a lead as interacted"),
            Self::define_tool::<TagInput>("get_c2s_tags", "List available tags"),
            Self::define_tool::<AddTagInput>("add_c2s_lead_tag", "Add a tag to a lead"),

            // Domain (3)
            Self::define_tool::<EmailInput>("analyze_email_domain", "Full domain analysis from email"),
            Self::define_tool::<EmailInput>("get_domain_trust_score", "Quick trust score for domain"),
            Self::define_tool::<EmailInput>("identify_company_from_email", "Identify company from email domain"),

            // Companies (7)
            Self::define_tool::<CnpjInput>("lookup_cnpj", "Lookup company by CNPJ"),
            Self::define_tool::<NameInput>("find_companies_by_name", "Find companies by owner name"),
            Self::define_tool::<NameInput>("analyze_company_portfolio", "Aggregate company portfolio analysis"),
            Self::define_tool::<CpfInput>("find_companies_by_cpf", "Find all companies where CPF is partner (65M CNPJs)"),
            Self::define_tool::<CnpjInput>("get_company_by_cnpj", "Get detailed company info by CNPJ"),
            Self::define_tool::<CompanySearchInput>("search_companies", "Search companies by name or CNPJ"),
            Self::define_tool::<CpfInput>("format_companies_message", "Format companies for C2S message"),

            // Tier (2)
            Self::define_tool::<TierCalcInput>("calculate_lead_tier", "Calculate tier (platinum/gold/silver/bronze/risk)"),
            Self::define_tool::<TierInput>("get_tier_recommendation", "Get recommendation for a tier"),

            // Search (5)
            Self::define_tool::<WebSearchInput>("search_web", "General web search"),
            Self::define_tool::<SearchPersonInput>("search_person", "Person-focused search (LinkedIn, business)"),
            Self::define_tool::<WebSearchInput>("search_news", "Search news and flag negative results"),
            Self::define_tool::<WebInsightsInput>("generate_web_insights", "Generate insights from web/search/surnames"),
            Self::define_tool::<AnalyzeLeadNameInput>("analyze_lead_name", "Comprehensive name analysis"),

            // Twenty CRM (13)
            Self::define_tool::<TwentyCreateLeadInput>("twenty_create_lead", "Create lead in Twenty CRM (auto-routes by tier)"),
            Self::define_tool::<TwentyUpdateLeadInput>("twenty_update_lead", "Update existing Twenty lead"),
            Self::define_tool::<TwentyGetLeadInput>("twenty_get_lead", "Fetch lead by ID (supports multi-workspace)"),
            Self::define_tool::<TwentyRouteInput>("twenty_route_lead", "Route lead to workspace by tier"),
            Self::define_tool::<TwentyDelegateInput>("twenty_delegate_lead", "Delegate lead with expiration tracking"),
            Self::define_tool::<TwentyBulkInput>("twenty_bulk_import", "Import multiple leads with deduplication"),
            Self::define_tool::<TwentyStatsInput>("twenty_get_pipeline_stats", "Pipeline stats (leads by tier/status)"),
            Self::define_tool::<TwentyBrokerStatsInput>("twenty_get_broker_stats", "Broker performance stats"),
            Self::define_tool::<TwentyBrokerStatsInput>("twenty_get_adoption_metrics", "Team adoption metrics"),
            Self::define_tool::<TwentySlaInput>("twenty_check_sla_violations", "Find SLA violations"),
            Self::define_tool::<TwentyDelegationExpiryInput>("twenty_check_delegation_expiry", "Find expiring delegations"),
            Self::define_tool::<TwentyIntentInput>("twenty_calculate_intent_signal", "Calculate intent signal from activity"),
            Self::define_tool::<TwentyNextActionInput>("twenty_get_next_action", "Recommended next action for lead"),
        ]
    }

    fn define_tool<T: schemars::JsonSchema + 'static>(name: &str, description: &str) -> Tool {
        // Build schema using schemars draft2020_12 (same as rmcp internals)
        let mut settings = schemars::generate::SchemaSettings::draft2020_12();
        settings.transforms = vec![Box::new(schemars::transform::AddNullable::default())];
        let generator = settings.into_generator();
        let schema = generator.into_root_schema_for::<T>();
        let schema_value = serde_json::to_value(schema).unwrap_or_default();
        let schema_obj = match schema_value {
            Value::Object(obj) => obj,
            _ => serde_json::Map::new(),
        };
        Tool::new(name.to_string(), description.to_string(), Arc::new(schema_obj))
    }

    // ─── Tool dispatcher ──────────────────────────────────────────

    async fn dispatch_tool(&self, name: &str, args: Value) -> Value {
        match name {
            // Enrichment (3)
            "enrich_lead" => self.handle_enrich_lead(&args).await,
            "enrich_bulk" => self.handle_enrich_bulk(&args).await,
            "retry_failed" => self.handle_retry_failed(&args).await,

            // Discovery (5)
            "find_and_save_person" => self.handle_find_and_save_person(&args).await,
            "discover_cpf" => self.handle_discover_cpf(&args).await,
            "lookup_cpf" => self.handle_lookup_cpf(&args).await,
            "search_cpf_by_name" => self.handle_search_cpf_by_name(&args).await,
            "validate_cpf" => self.handle_validate_cpf(&args),

            // Leads (3)
            "get_lead" => self.handle_get_lead(&args).await,
            "list_leads" => self.handle_list_leads(&args).await,
            "get_c2s_lead_status" => self.handle_get_c2s_lead_status(&args).await,

            // Stats (4)
            "get_enrichment_stats" => self.handle_enrichment_stats(&args).await,
            "get_service_health" => self.handle_service_health().await,
            "get_enrichment_rate" => self.handle_enrichment_rate(&args).await,
            "get_enrichment_health" => self.handle_enrichment_health(&args).await,

            // Property (3)
            "get_properties_by_cpf" => self.handle_get_properties_by_cpf(&args).await,
            "get_property_summary" => self.handle_get_properties_by_cpf(&args).await,
            "format_property_message" => self.handle_format_property_message(&args).await,

            // Reports (3)
            "generate_profile_report" => self.handle_generate_report(&args),
            "generate_report_from_cpfs" => self.handle_generate_report_extended(&args).await,
            "generate_report_pdf" => self.handle_generate_report_extended(&args).await,

            // Analysis (6)
            "analyze_lead" => self.handle_analyze_lead(&args).await,
            "get_lead_analysis" => self.handle_get_lead_analysis(&args).await,
            "check_lead_alert" => self.handle_get_lead_analysis(&args).await,
            "score_lead_quality" => self.handle_score_quality(&args),
            "assess_risk" => self.handle_assess_risk(&args),
            "quick_risk_check" => self.handle_quick_risk(&args),

            // C2S CRM (9)
            "fetch_c2s_leads" => self.handle_fetch_c2s_leads(&args).await,
            "get_c2s_sellers" => self.handle_get_c2s_sellers().await,
            "send_c2s_message" => self.handle_send_c2s_message(&args).await,
            "forward_c2s_lead" => self.handle_forward_c2s_lead(&args).await,
            "search_c2s_by_phone" => self.handle_search_c2s_by_phone(&args).await,
            "search_c2s_by_email" => self.handle_search_c2s_by_email(&args).await,
            "mark_c2s_interacted" => self.stub_tool(name, &args),
            "get_c2s_tags" => self.handle_get_c2s_tags().await,
            "add_c2s_lead_tag" => self.handle_add_c2s_lead_tag(&args).await,

            // Domain (3)
            "analyze_email_domain" => self.handle_analyze_domain(&args),
            "get_domain_trust_score" => self.handle_domain_trust(&args),
            "identify_company_from_email" => self.handle_identify_company(&args),

            // Companies (7)
            "lookup_cnpj" => self.handle_get_company_by_cnpj(&args).await,
            "find_companies_by_name" => self.handle_search_companies(&args).await,
            "analyze_company_portfolio" => self.handle_find_companies_by_cpf(&args).await,
            "find_companies_by_cpf" => self.handle_find_companies_by_cpf(&args).await,
            "get_company_by_cnpj" => self.handle_get_company_by_cnpj(&args).await,
            "search_companies" => self.handle_search_companies(&args).await,
            "format_companies_message" => self.handle_format_companies_message(&args).await,

            // Tier (2)
            "calculate_lead_tier" => self.handle_calculate_tier(&args),
            "get_tier_recommendation" => self.handle_tier_recommendation(&args),

            // Search (5)
            "search_web" => self.handle_search_web(&args).await,
            "search_person" => self.handle_search_person(&args).await,
            "search_news" => self.handle_search_news(&args).await,
            "generate_web_insights" => self.handle_search_person(&args).await,
            "analyze_lead_name" => self.handle_analyze_name(&args),

            // Twenty CRM (13)
            "twenty_create_lead" => self.handle_twenty_create_lead(&args).await,
            "twenty_update_lead" => self.handle_twenty_update_lead(&args).await,
            "twenty_get_lead" => self.handle_twenty_get_lead(&args).await,
            "twenty_route_lead" => self.handle_twenty_route(&args),
            "twenty_delegate_lead" => self.handle_twenty_delegate_lead(&args).await,
            "twenty_bulk_import" => self.handle_twenty_bulk_import(&args).await,
            "twenty_get_pipeline_stats" => self.handle_twenty_get_pipeline_stats(&args).await,
            "twenty_get_broker_stats" => self.handle_twenty_get_broker_stats(&args).await,
            "twenty_get_adoption_metrics" => self.handle_twenty_get_pipeline_stats(&args).await,
            "twenty_check_sla_violations" => self.handle_twenty_check_sla_violations(&args).await,
            "twenty_check_delegation_expiry" => self.handle_twenty_check_delegation_expiry(&args).await,
            "twenty_calculate_intent_signal" => self.handle_twenty_intent(&args),
            "twenty_get_next_action" => self.handle_twenty_next_action(&args),

            _ => json!({ "success": false, "error": format!("Unknown tool: {}", name) }),
        }
    }

    // ─── Stub for tools that need DB/HTTP (wired in Phase 12b) ──────

    fn stub_tool(&self, name: &str, _args: &Value) -> Value {
        json!({
            "success": false,
            "error": format!("Tool '{}' requires database/HTTP connection — not yet wired for MCP stdio transport. Use the HTTP API instead.", name),
            "hint": "This tool's schema is registered and validated. Full implementation requires AppState with DB pool and HTTP clients."
        })
    }

    // ─── DB read tool handlers (RML-1106) ──────────────────────────

    async fn handle_get_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "get_lead");
        let lead_id = args.get("lead_id").and_then(|v| v.as_str());
        let phone = args.get("phone").and_then(|v| v.as_str());

        if lead_id.is_none() && phone.is_none() {
            return json!({ "success": false, "error": "lead_id or phone required" });
        }

        let row = sqlx::query_as::<_, (
            String,                          // lead_id
            Option<String>,                  // customer_name
            Option<String>,                  // customer_phone
            Option<String>,                  // customer_email
            Option<String>,                  // enrichment_status
            Option<String>,                  // cpf
            Option<uuid::Uuid>,              // party_id
            Option<chrono::NaiveDateTime>,   // received_at
        )>(
            r#"SELECT lead_id, customer_name, customer_phone, customer_email,
                      enrichment_status, cpf, party_id, received_at
               FROM analytics.c2s_leads
               WHERE ($1::text IS NOT NULL AND lead_id = $1)
                  OR ($2::text IS NOT NULL AND customer_phone_normalized = $2)
               LIMIT 1"#,
        )
        .bind(lead_id)
        .bind(phone)
        .fetch_optional(&state.db)
        .await;

        match row {
            Ok(Some((lid, name, ph, email, status, cpf, party_id, received))) => json!({
                "success": true,
                "lead": {
                    "lead_id": lid,
                    "customer_name": name,
                    "customer_phone": ph,
                    "customer_email": email,
                    "enrichment_status": status,
                    "cpf": cpf,
                    "party_id": party_id.map(|u| u.to_string()),
                    "received_at": received.map(|d| d.to_string()),
                }
            }),
            Ok(None) => json!({ "success": true, "lead": null, "message": "Lead not found" }),
            Err(e) => json!({ "success": false, "error": format!("DB query failed: {}", e) }),
        }
    }

    async fn handle_list_leads(&self, args: &Value) -> Value {
        let state = require_state!(self, "list_leads");
        let status = args.get("status").and_then(|v| v.as_str());
        let seller_id = args.get("seller_id").and_then(|v| v.as_str());
        let date_from = args.get("date_from").and_then(|v| v.as_str());
        let date_to = args.get("date_to").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20).min(100) as i64;
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, (
            String, Option<String>, Option<String>, Option<String>,
            Option<String>, Option<String>, Option<chrono::NaiveDateTime>,
        )>(
            r#"SELECT lead_id, customer_name, customer_phone, customer_email,
                      enrichment_status, cpf, received_at
               FROM analytics.c2s_leads
               WHERE ($1::text IS NULL OR enrichment_status = $1)
                 AND ($2::timestamptz IS NULL OR received_at >= $2::timestamptz)
                 AND ($3::timestamptz IS NULL OR received_at <= $3::timestamptz)
               ORDER BY received_at DESC
               LIMIT $4 OFFSET $5"#,
        )
        .bind(status)
        .bind(date_from)
        .bind(date_to)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await;

        match rows {
            Ok(rows) => {
                let leads: Vec<Value> = rows.iter().map(|(lid, name, ph, email, st, cpf, recv)| {
                    json!({
                        "lead_id": lid,
                        "customer_name": name,
                        "customer_phone": ph,
                        "customer_email": email,
                        "enrichment_status": st,
                        "cpf": cpf,
                        "received_at": recv.map(|d| d.to_string()),
                    })
                }).collect();
                let mut result = json!({ "success": true, "count": leads.len(), "leads": leads });
                if seller_id.is_some() {
                    result["note"] = json!("seller_id filter requires C2S API — use fetch_c2s_leads tool instead");
                }
                result
            }
            Err(e) => json!({ "success": false, "error": format!("DB query failed: {}", e) }),
        }
    }

    async fn handle_enrichment_stats(&self, args: &Value) -> Value {
        let state = require_state!(self, "get_enrichment_stats");
        let date_from = args.get("date_from").and_then(|v| v.as_str());
        let date_to = args.get("date_to").and_then(|v| v.as_str());

        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64)>(
            r#"SELECT
                 COUNT(*) as total,
                 COUNT(*) FILTER (WHERE enrichment_status = 'completed') as completed,
                 COUNT(*) FILTER (WHERE enrichment_status IN ('partial', 'basic')) as partial,
                 COUNT(*) FILTER (WHERE enrichment_status IN ('unenriched', 'failed')) as failed,
                 COUNT(*) FILTER (WHERE enrichment_status = 'pending') as pending,
                 COUNT(*) FILTER (WHERE enrichment_status = 'processing') as processing
               FROM analytics.c2s_leads
               WHERE ($1::timestamptz IS NULL OR received_at >= $1::timestamptz)
                 AND ($2::timestamptz IS NULL OR received_at <= $2::timestamptz)"#,
        )
        .bind(date_from)
        .bind(date_to)
        .fetch_one(&state.db)
        .await;

        match row {
            Ok((total, completed, partial, failed, pending, processing)) => {
                let rate = if total > 0 { (completed as f64 / total as f64) * 100.0 } else { 0.0 };
                json!({
                    "success": true,
                    "total": total,
                    "completed": completed,
                    "partial": partial,
                    "failed": failed,
                    "pending": pending,
                    "processing": processing,
                    "enrichment_rate": format!("{:.1}%", rate),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })
            }
            Err(e) => json!({ "success": false, "error": format!("DB query failed: {}", e) }),
        }
    }

    async fn handle_enrichment_rate(&self, _args: &Value) -> Value {
        let state = require_state!(self, "get_enrichment_rate");

        let row = sqlx::query_as::<_, (i64, i64)>(
            r#"SELECT
                 COUNT(*) as total,
                 COUNT(*) FILTER (WHERE enrichment_status = 'completed') as completed
               FROM analytics.c2s_leads"#,
        )
        .fetch_one(&state.db)
        .await;

        match row {
            Ok((total, completed)) => {
                let rate = if total > 0 { (completed as f64 / total as f64) * 100.0 } else { 0.0 };
                json!({
                    "success": true,
                    "rate": format!("{:.1}", rate),
                    "total": total,
                    "completed": completed,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })
            }
            Err(e) => json!({ "success": false, "error": format!("DB query failed: {}", e) }),
        }
    }

    async fn handle_enrichment_health(&self, args: &Value) -> Value {
        let state = require_state!(self, "get_enrichment_health");
        let threshold = args.get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(80.0);

        let row = sqlx::query_as::<_, (i64, i64)>(
            r#"SELECT
                 COUNT(*) as total,
                 COUNT(*) FILTER (WHERE enrichment_status = 'completed') as completed
               FROM analytics.c2s_leads"#,
        )
        .fetch_one(&state.db)
        .await;

        match row {
            Ok((total, completed)) => {
                let rate = if total > 0 { (completed as f64 / total as f64) * 100.0 } else { 0.0 };
                let healthy = rate >= threshold;
                json!({
                    "success": true,
                    "healthy": healthy,
                    "rate": format!("{:.1}", rate),
                    "threshold": threshold,
                    "total": total,
                    "completed": completed,
                    "status": if healthy { "ok" } else { "degraded" },
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })
            }
            Err(e) => json!({ "success": false, "error": format!("DB query failed: {}", e) }),
        }
    }

    // ─── Pure-logic tool handlers (no DB/HTTP needed) ───────────────

    fn handle_validate_cpf(&self, args: &Value) -> Value {
        let cpf = args.get("cpf").and_then(|v| v.as_str()).unwrap_or("");
        let digits: Vec<u8> = cpf.chars().filter(|c| c.is_ascii_digit()).map(|c| c as u8 - b'0').collect();
        if digits.len() != 11 {
            return json!({ "success": true, "cpf": cpf, "isValid": false, "reason": "CPF must have 11 digits" });
        }
        // mod-11 check
        let all_same = digits.iter().all(|&d| d == digits[0]);
        if all_same {
            return json!({ "success": true, "cpf": cpf, "isValid": false, "reason": "All digits are the same" });
        }
        let check1: u32 = (0..9).map(|i| digits[i] as u32 * (10 - i as u32)).sum::<u32>();
        let rem1 = (check1 * 10) % 11;
        let d1 = if rem1 == 10 { 0 } else { rem1 as u8 };
        let check2: u32 = (0..10).map(|i| digits[i] as u32 * (11 - i as u32)).sum::<u32>();
        let rem2 = (check2 * 10) % 11;
        let d2 = if rem2 == 10 { 0 } else { rem2 as u8 };
        let valid = d1 == digits[9] && d2 == digits[10];
        json!({ "success": true, "cpf": cpf, "isValid": valid, "reason": if valid { "Valid CPF" } else { "Failed mod-11 check" } })
    }

    async fn handle_discover_cpf(&self, args: &Value) -> Value {
        let phone = args.get("phone").and_then(|v| v.as_str());
        let email = args.get("email").and_then(|v| v.as_str());
        let name = args.get("name").and_then(|v| v.as_str());
        if phone.is_none() && email.is_none() && name.is_none() {
            return json!({ "success": false, "error": "At least one of phone, email, or name is required" });
        }
        let state = require_state!(self, "discover_cpf");

        // Try phone discovery first (5-tier cascade)
        if let Some(phone) = phone {
            match state.discovery.find_cpf_by_phone(phone, name).await {
                Ok(Some(result)) => return json!({
                    "success": true,
                    "cpf": result.cpf,
                    "source": result.source,
                    "foundName": result.found_name,
                    "nameMatches": result.name_matches,
                    "matchScore": result.match_score,
                    "matchMethod": result.match_method
                }),
                Ok(None) => {} // fall through to email/name
                Err(e) => return json!({ "success": false, "error": e.to_string() }),
            }
        }

        // Try email discovery (2-tier cascade)
        if let Some(email) = email {
            match state.discovery.find_cpf_by_email(email, name).await {
                Ok(Some(result)) => return json!({
                    "success": true,
                    "cpf": result.cpf,
                    "source": result.source,
                    "foundName": result.found_name,
                    "nameMatches": result.name_matches,
                    "matchScore": result.match_score,
                    "matchMethod": result.match_method
                }),
                Ok(None) => {}
                Err(e) => return json!({ "success": false, "error": e.to_string() }),
            }
        }

        // Try name-only DuckDB search as last resort
        if let Some(name) = name {
            match state.discovery.find_cpf_by_name_duckdb(name).await {
                Ok(Some(result)) => return json!({
                    "success": true,
                    "cpf": result.cpf,
                    "source": result.source,
                    "foundName": result.found_name,
                    "matchScore": result.match_score
                }),
                Ok(None) => {}
                Err(e) => return json!({ "success": false, "error": e.to_string() }),
            }
        }

        json!({ "success": false, "error": "CPF not found via any discovery tier" })
    }

    async fn handle_lookup_cpf(&self, args: &Value) -> Value {
        let state = require_state!(self, "lookup_cpf");
        let cpf = match args.get("cpf").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => return json!({ "success": false, "error": "cpf is required" }),
        };
        match state.work_api.fetch_all_modules(cpf).await {
            Ok(data) => json!({ "success": true, "cpf": cpf, "data": data }),
            Err(e) => json!({ "success": false, "cpf": cpf, "error": e.to_string() }),
        }
    }

    async fn handle_search_cpf_by_name(&self, args: &Value) -> Value {
        let state = require_state!(self, "search_cpf_by_name");
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n,
            _ => return json!({ "success": false, "error": "name is required" }),
        };
        match state.discovery.find_cpf_by_name_duckdb(name).await {
            Ok(Some(result)) => json!({
                "success": true,
                "found": true,
                "cpf": result.cpf,
                "source": result.source,
                "foundName": result.found_name,
                "matchScore": result.match_score
            }),
            Ok(None) => json!({ "success": true, "found": false, "message": "No CPF found for this name" }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    }

    async fn handle_find_and_save_person(&self, args: &Value) -> Value {
        let state = require_state!(self, "find_and_save_person");
        let phone = match args.get("phone").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p,
            _ => return json!({ "success": false, "error": "phone is required" }),
        };
        let name = args.get("name").and_then(|v| v.as_str());

        // Step 1: Discover CPF via phone
        let discovery_result = match state.discovery.find_cpf_by_phone(phone, name).await {
            Ok(Some(r)) => r,
            Ok(None) => return json!({ "success": false, "error": "CPF not found for this phone" }),
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };
        let cpf = &discovery_result.cpf;

        // Step 2: Fetch full Work API data
        let work_data = match state.work_api.fetch_all_modules(cpf).await {
            Ok(d) => d,
            Err(e) => return json!({ "success": false, "cpf": cpf, "error": format!("Work API fetch failed: {}", e) }),
        };

        // Step 3: Save to core.parties via EnrichmentStorage
        match state.storage.store_enriched_person(cpf, &work_data).await {
            Ok(party_id) => json!({
                "success": true,
                "cpf": cpf,
                "partyId": party_id.to_string(),
                "source": discovery_result.source,
                "foundName": discovery_result.found_name
            }),
            Err(e) => json!({ "success": false, "cpf": cpf, "error": format!("Storage failed: {}", e) }),
        }
    }




    // ─── Enrichment Tool Handlers (RML-1108) ────────────────────

    async fn handle_enrich_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "enrich_lead");
        let phone = args.get("phone").and_then(|v| v.as_str());
        let email = args.get("email").and_then(|v| v.as_str());
        let name = args.get("name").and_then(|v| v.as_str());
        let lead_id = args.get("lead_id").and_then(|v| v.as_str());

        if phone.is_none() && email.is_none() {
            return json!({ "success": false, "error": "At least one of phone or email is required" });
        }

        // Step 1: Discover CPF(s)
        let cpf_result = match crate::enrichment::find_cpf_via_diretrix(
            phone, email, &state.config, name,
        ).await {
            Ok(r) => r,
            Err(e) => return json!({ "success": false, "error": format!("CPF discovery failed: {}", e) }),
        };

        if cpf_result.cpfs.is_empty() {
            return json!({ "success": false, "error": "No CPF found via any discovery tier" });
        }

        // Step 2: Enrich with Work API
        let enriched_data = match crate::enrichment::enrich_cpfs_with_work_api(
            &cpf_result.cpfs, &state.config,
        ).await {
            Ok(d) => d,
            Err(e) => return json!({ "success": false, "cpfs": cpf_result.cpfs, "error": format!("Work API enrichment failed: {}", e) }),
        };

        // Step 3: Format message
        let mut message_body = crate::enrichment::format_enriched_message_body(
            name.unwrap_or(""),
            phone.unwrap_or(""),
            email.unwrap_or(""),
            &enriched_data,
            cpf_result.same_person,
        );

        // Step 3b: Append company data from Meilisearch
        if state.meilisearch.is_enabled() && !cpf_result.cpfs.is_empty() {
            let summary = state.meilisearch.find_companies_by_cpf(&cpf_result.cpfs[0]).await;
            if summary.total_companies > 0 {
                let company_msg = MeilisearchCompanyService::format_companies_for_message(&summary);
                if !company_msg.is_empty() {
                    message_body.push_str(&company_msg);
                }
            }
        }

        // Step 4: Send to C2S (if lead_id provided)
        let message_sent = if let Some(lid) = lead_id {
            match state.c2s.send_message(lid, &message_body).await {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!("Failed to send C2S message: {}", e);
                    false
                }
            }
        } else {
            false
        };

        // Step 5: Store in database
        let stored_ids = match crate::enrichment::store_enriched_data(
            &state.db, &cpf_result.cpfs, &enriched_data, lead_id,
        ).await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!("Failed to store enriched data: {}", e);
                vec![]
            }
        };

        // Step 5b: Update c2s_leads enrichment status
        if let Some(lid) = lead_id {
            let status = if !cpf_result.cpfs.is_empty() && !enriched_data.is_empty() { "completed" } else { "partial" };
            let _ = sqlx::query(
                "UPDATE analytics.c2s_leads SET enrichment_status = $1, cpf = $2, party_id = $3, enriched_at = now(), updated_at = now() WHERE lead_id = $4"
            )
            .bind(status)
            .bind(cpf_result.cpfs.first().map(|s| s.as_str()))
            .bind(stored_ids.first())
            .bind(lid)
            .execute(&state.db)
            .await;
        }

        json!({
            "success": true,
            "cpfs": cpf_result.cpfs,
            "samePerson": cpf_result.same_person,
            "enrichedCount": enriched_data.len(),
            "messageSent": message_sent,
            "storedCount": stored_ids.len(),
            "entityIds": stored_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()
        })
    }

    async fn handle_enrich_bulk(&self, args: &Value) -> Value {
        let state = require_state!(self, "enrich_bulk");
        let leads = match args.get("leads").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return json!({ "success": false, "error": "leads array is required" }),
        };

        let mut results = Vec::new();
        let mut success_count = 0u32;
        let mut fail_count = 0u32;

        for lead in leads.iter().take(50) { // Cap at 50 to prevent abuse
            let phone = lead.get("phone").and_then(|v| v.as_str());
            let email = lead.get("email").and_then(|v| v.as_str());
            let name = lead.get("name").and_then(|v| v.as_str());
            let lead_id = lead.get("lead_id").and_then(|v| v.as_str());

            if phone.is_none() && email.is_none() {
                fail_count += 1;
                results.push(json!({ "lead_id": lead_id, "success": false, "error": "no phone or email" }));
                continue;
            }

            // Discovery
            match state.discovery.find_cpf_by_phone(
                phone.unwrap_or(""), name,
            ).await {
                Ok(Some(cpf_result)) => {
                    // Enrich
                    match state.work_api.fetch_all_modules(&cpf_result.cpf).await {
                        Ok(work_data) => {
                            // Store
                            let party_id = state.storage.store_enriched_person(&cpf_result.cpf, &work_data).await.ok();
                            success_count += 1;
                            results.push(json!({
                                "lead_id": lead_id,
                                "success": true,
                                "cpf": cpf_result.cpf,
                                "partyId": party_id.map(|id| id.to_string())
                            }));
                        }
                        Err(e) => {
                            fail_count += 1;
                            results.push(json!({ "lead_id": lead_id, "success": false, "cpf": cpf_result.cpf, "error": e.to_string() }));
                        }
                    }
                }
                Ok(None) => {
                    fail_count += 1;
                    results.push(json!({ "lead_id": lead_id, "success": false, "error": "CPF not found" }));
                }
                Err(e) => {
                    fail_count += 1;
                    results.push(json!({ "lead_id": lead_id, "success": false, "error": e.to_string() }));
                }
            }

            // Rate limit between leads
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        json!({
            "success": true,
            "total": leads.len(),
            "processed": results.len(),
            "succeeded": success_count,
            "failed": fail_count,
            "results": results
        })
    }

    async fn handle_retry_failed(&self, args: &Value) -> Value {
        let state = require_state!(self, "retry_failed");
        let statuses: Vec<&str> = args.get("statuses")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_else(|| vec!["failed", "partial", "unenriched"]);
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20) as i64;

        // Find leads by status
        let rows = match sqlx::query(
            "SELECT lead_id, customer_name, customer_phone, customer_email, enrichment_status, retry_count              FROM analytics.c2s_leads              WHERE enrichment_status = ANY($1) AND (retry_count < 3 OR retry_count IS NULL)              ORDER BY received_at DESC LIMIT $2"
        )
        .bind(&statuses.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .bind(limit)
        .fetch_all(&state.db)
        .await {
            Ok(rows) => rows,
            Err(e) => return json!({ "success": false, "error": format!("DB query failed: {}", e) }),
        };

        let total = rows.len();
        let mut retried = 0u32;
        let mut succeeded = 0u32;

        for row in &rows {
            use sqlx::Row;
            let lead_id: String = row.try_get("lead_id").unwrap_or_default();
            let phone: Option<String> = row.try_get("customer_phone").ok();
            let name: Option<String> = row.try_get("customer_name").ok();

            if let Some(ref p) = phone {
                match state.discovery.find_cpf_by_phone(p, name.as_deref()).await {
                    Ok(Some(cpf_result)) => {
                        if let Ok(work_data) = state.work_api.fetch_all_modules(&cpf_result.cpf).await {
                            let party_id = state.storage.store_enriched_person(&cpf_result.cpf, &work_data).await.ok();
                            let _ = sqlx::query(
                                "UPDATE analytics.c2s_leads SET enrichment_status = 'completed', cpf = $1, party_id = $2, enriched_at = now(), retry_count = COALESCE(retry_count, 0) + 1, updated_at = now() WHERE lead_id = $3"
                            )
                            .bind(&cpf_result.cpf)
                            .bind(party_id)
                            .bind(&lead_id)
                            .execute(&state.db)
                            .await;
                            succeeded += 1;
                        }
                    }
                    _ => {
                        let _ = sqlx::query(
                            "UPDATE analytics.c2s_leads SET retry_count = COALESCE(retry_count, 0) + 1, last_retry_at = now(), updated_at = now() WHERE lead_id = $1"
                        )
                        .bind(&lead_id)
                        .execute(&state.db)
                        .await;
                    }
                }
            }
            retried += 1;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        json!({
            "success": true,
            "total": total,
            "retried": retried,
            "succeeded": succeeded,
            "statuses": statuses
        })
    }


    // ─── Web Search Tool Handlers (RML-1112) ────────────────────

    async fn handle_search_web(&self, args: &Value) -> Value {
        let state = require_state!(self, "search_web");
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            _ => return json!({ "success": false, "error": "query is required" }),
        };
        let num = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
        let results = state.web_search.search(query, num).await;
        json!({ "success": true, "query": query, "count": results.len(), "results": serde_json::to_value(&results).unwrap_or(Value::Null) })
    }

    async fn handle_search_person(&self, args: &Value) -> Value {
        let state = require_state!(self, "search_person");
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n,
            _ => return json!({ "success": false, "error": "name is required" }),
        };
        let info = state.web_search.search_person(name).await;
        json!({ "success": true, "name": name, "person": serde_json::to_value(&info).unwrap_or(Value::Null) })
    }

    async fn handle_search_news(&self, args: &Value) -> Value {
        let state = require_state!(self, "search_news");
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n,
            _ => return json!({ "success": false, "error": "name is required" }),
        };
        let results = state.web_search.search_news(name).await;
        let has_negative = results.iter().any(|r| r.is_negative);
        json!({ "success": true, "name": name, "hasNegative": has_negative, "count": results.len(), "results": serde_json::to_value(&results).unwrap_or(Value::Null) })
    }

    // ─── Lead Analysis Tool Handlers (RML-1113) ─────────────────

    async fn handle_analyze_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "analyze_lead");
        let input = crate::lead_analysis::LeadAnalysisInput {
            lead_id: args.get("lead_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            name: args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            email: args.get("email").and_then(|v| v.as_str()).map(String::from),
            phone: args.get("phone").and_then(|v| v.as_str()).map(String::from),
            cpf: args.get("cpf").and_then(|v| v.as_str()).map(String::from),
            income: args.get("income").and_then(|v| v.as_f64()),
        };
        let result = state.lead_analysis.analyze(&input).await;
        json!({
            "success": true,
            "tier": result.tier,
            "score": result.score,
            "discovered": serde_json::to_value(&result.discovered).unwrap_or(Value::Null),
            "alerts": result.alerts,
            "highlights": result.highlights,
            "recommendation": serde_json::to_value(&result.recommendation).unwrap_or(Value::Null),
            "riskAssessment": serde_json::to_value(&result.risk_assessment).unwrap_or(Value::Null),
            "durationMs": result.duration_ms
        })
    }

    async fn handle_get_lead_analysis(&self, args: &Value) -> Value {
        let state = require_state!(self, "get_lead_analysis");
        let lead_id = match args.get("lead_id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return json!({ "success": false, "error": "lead_id is required" }),
        };
        match state.lead_analysis.get_cached(lead_id).await {
            Some(result) => json!({
                "success": true,
                "cached": true,
                "tier": result.tier,
                "score": result.score,
                "discovered": serde_json::to_value(&result.discovered).unwrap_or(Value::Null),
                "recommendation": serde_json::to_value(&result.recommendation).unwrap_or(Value::Null)
            }),
            None => json!({ "success": true, "cached": false, "message": "No cached analysis found — use analyze_lead to generate" }),
        }
    }

    // ─── Report Tool Handlers ───────────────────────────────────

    async fn handle_generate_report_extended(&self, args: &Value) -> Value {
        let state = require_state!(self, "generate_report");
        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("html");
        let persons: Vec<crate::report::ReportPerson> = args.get("persons")
            .or_else(|| args.get("cpfs"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        if persons.is_empty() {
            return json!({ "success": false, "error": "persons array is required" });
        }
        let options = crate::report::ReportOptions {
            title: args.get("title").and_then(|v| v.as_str()).unwrap_or("Lead Report").to_string(),
            subtitle: args.get("subtitle").and_then(|v| v.as_str()).map(String::from),
            classification: "Confidencial - Uso Interno".to_string(),
            include_contacts: true,
            include_income: true,
            output_dir: args.get("output_dir").and_then(|v| v.as_str()).map(String::from),
        };
        let result = match format {
            "pdf" => state.report.generate_pdf(&persons, &options).await,
            "markdown" | "md" => state.report.generate_markdown(&persons, &options),
            _ => state.report.generate_html(&persons, &options),
        };
        json!({
            "success": result.success,
            "format": result.format,
            "filePath": result.file_path,
            "contentLength": result.content.as_ref().map(|c| c.len()),
            "error": result.error
        })
    }

    // ─── Twenty CRM Tool Handlers (RML-1113) ────────────────────

    async fn handle_twenty_create_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_create_lead");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let input = crate::twenty::TwentyLeadInput {
            name: args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            phone: args.get("phone").and_then(|v| v.as_str()).map(String::from),
            email: args.get("email").and_then(|v| v.as_str()).map(String::from),
            source: args.get("source").and_then(|v| v.as_str()).map(String::from),
            tier: args.get("tier").and_then(|v| v.as_str()).map(String::from),
            score: args.get("score").and_then(|v| v.as_i64()).map(|v| v as i32),
            cpf: args.get("cpf").and_then(|v| v.as_str()).map(String::from),
            metadata: args.get("metadata").cloned(),
        };
        match state.twenty.create_lead(&input).await {
            Ok(lead) => json!({ "success": true, "lead": serde_json::to_value(&lead).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_twenty_update_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_update_lead");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let lead_id = match args.get("lead_id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return json!({ "success": false, "error": "lead_id is required" }),
        };
        let workspace = parse_workspace(args.get("workspace").and_then(|v| v.as_str()));
        let updates = args.get("updates").cloned().unwrap_or(json!({}));
        match state.twenty.update_lead(lead_id, workspace, updates).await {
            Ok(result) => json!({ "success": true, "result": result }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_twenty_get_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_get_lead");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let lead_id = match args.get("lead_id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return json!({ "success": false, "error": "lead_id is required" }),
        };
        match state.twenty.find_lead(lead_id).await {
            Ok(Some(lead)) => json!({ "success": true, "lead": lead }),
            Ok(None) => json!({ "success": true, "found": false }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_twenty_delegate_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_delegate_lead");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let input: crate::twenty::DelegateInput = match serde_json::from_value(args.clone()) {
            Ok(i) => i,
            Err(e) => return json!({ "success": false, "error": format!("Invalid input: {}", e) }),
        };
        let tier = args.get("tier").and_then(|v| v.as_str());
        let info = match tier {
            Some(t) => state.twenty.create_delegation_with_tier(&input, t),
            None => state.twenty.create_delegation(&input),
        };
        json!({ "success": true, "delegation": serde_json::to_value(&info).unwrap_or(Value::Null) })
    }

    async fn handle_twenty_bulk_import(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_bulk_import");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let input: crate::twenty::BulkImportInput = match serde_json::from_value(args.clone()) {
            Ok(i) => i,
            Err(e) => return json!({ "success": false, "error": format!("Invalid input: {}", e) }),
        };
        match state.twenty.bulk_import(&input).await {
            Ok(result) => json!({ "success": true, "result": serde_json::to_value(&result).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_twenty_get_pipeline_stats(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_get_pipeline_stats");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let workspace = parse_workspace(args.get("workspace").and_then(|v| v.as_str()));
        match state.twenty.get_pipeline_stats(workspace).await {
            Ok(stats) => json!({ "success": true, "stats": serde_json::to_value(&stats).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_twenty_get_broker_stats(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_get_broker_stats");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let workspace = parse_workspace(args.get("workspace").and_then(|v| v.as_str()));
        match state.twenty.get_broker_stats(workspace).await {
            Ok(stats) => json!({ "success": true, "brokers": serde_json::to_value(&stats).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_twenty_check_sla_violations(&self, args: &Value) -> Value {
        let state = require_state!(self, "twenty_check_sla_violations");
        if !state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        let workspace = parse_workspace(args.get("workspace").and_then(|v| v.as_str()));
        match state.twenty.check_sla_violations(workspace).await {
            Ok(violations) => json!({ "success": true, "violations": serde_json::to_value(&violations).unwrap_or(Value::Null), "count": violations.len() }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_twenty_check_delegation_expiry(&self, _args: &Value) -> Value {
        let _state = require_state!(self, "twenty_check_delegation_expiry");
        if !_state.twenty.is_enabled() {
            return json!({ "success": false, "error": "Twenty CRM is not enabled" });
        }
        // Delegation expiry checking is per-lead — return guidance
        json!({
            "success": true,
            "note": "Use twenty_get_lead to fetch a lead, then check delegation.expires_at field",
            "hint": "TwentyService::is_delegation_expired(delegation) checks expiry"
        })
    }

    // ─── C2S CRM Tool Handlers (RML-1109) ───────────────────────

    async fn handle_fetch_c2s_leads(&self, args: &Value) -> Value {
        let state = require_state!(self, "fetch_c2s_leads");
        let lead_id = args.get("lead_id").and_then(|v| v.as_str());
        match lead_id {
            Some(id) => {
                match state.c2s.fetch_lead(id).await {
                    Ok(resp) => json!({ "success": true, "lead": serde_json::to_value(&resp.data).unwrap_or(Value::Null) }),
                    Err(e) => json!({ "success": false, "error": e.to_string() }),
                }
            }
            None => json!({ "success": false, "error": "lead_id is required (C2S API does not support listing — use list_leads for DB query)" }),
        }
    }

    async fn handle_get_c2s_sellers(&self) -> Value {
        let state = require_state!(self, "get_c2s_sellers");
        match state.c2s_extended.list_sellers().await {
            Ok(sellers) => json!({ "success": true, "sellers": serde_json::to_value(&sellers).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_send_c2s_message(&self, args: &Value) -> Value {
        let state = require_state!(self, "send_c2s_message");
        let lead_id = match args.get("lead_id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return json!({ "success": false, "error": "lead_id is required" }),
        };
        let message = match args.get("message").and_then(|v| v.as_str()) {
            Some(m) if !m.is_empty() => m,
            _ => return json!({ "success": false, "error": "message is required" }),
        };
        match state.c2s.send_message(lead_id, message).await {
            Ok(()) => json!({ "success": true, "lead_id": lead_id }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    }

    async fn handle_forward_c2s_lead(&self, args: &Value) -> Value {
        let state = require_state!(self, "forward_c2s_lead");
        let lead_id = match args.get("lead_id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return json!({ "success": false, "error": "lead_id is required" }),
        };
        let seller_id = match args.get("seller_id").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => return json!({ "success": false, "error": "seller_id is required" }),
        };
        let message = args.get("message").and_then(|v| v.as_str()).map(String::from);
        let input = crate::c2s_extended::ForwardInput {
            seller_id: seller_id.to_string(),
            message,
        };
        match state.c2s_extended.forward_lead(lead_id, &input).await {
            Ok(()) => json!({ "success": true, "lead_id": lead_id, "forwarded_to": seller_id }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_search_c2s_by_phone(&self, args: &Value) -> Value {
        let state = require_state!(self, "search_c2s_by_phone");
        let phone = match args.get("phone").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p,
            _ => return json!({ "success": false, "error": "phone is required" }),
        };
        match state.c2s_extended.search_by_phone(phone).await {
            Ok(results) => json!({ "success": true, "results": serde_json::to_value(&results).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_search_c2s_by_email(&self, args: &Value) -> Value {
        let state = require_state!(self, "search_c2s_by_email");
        let email = match args.get("email").and_then(|v| v.as_str()) {
            Some(e) if !e.is_empty() => e,
            _ => return json!({ "success": false, "error": "email is required" }),
        };
        match state.c2s_extended.search_by_email(email).await {
            Ok(results) => json!({ "success": true, "results": serde_json::to_value(&results).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_get_c2s_tags(&self) -> Value {
        let state = require_state!(self, "get_c2s_tags");
        match state.c2s_extended.list_tags().await {
            Ok(tags) => json!({ "success": true, "tags": serde_json::to_value(&tags).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_add_c2s_lead_tag(&self, args: &Value) -> Value {
        let state = require_state!(self, "add_c2s_lead_tag");
        let lead_id = match args.get("lead_id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return json!({ "success": false, "error": "lead_id is required" }),
        };
        let tag_id = match args.get("tag_id").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => t,
            _ => return json!({ "success": false, "error": "tag_id is required" }),
        };
        match state.c2s_extended.add_tag_to_lead(lead_id, tag_id).await {
            Ok(()) => json!({ "success": true, "lead_id": lead_id, "tag_id": tag_id }),
            Err(e) => json!({ "success": false, "error": e }),
        }
    }

    async fn handle_get_c2s_lead_status(&self, args: &Value) -> Value {
        let state = require_state!(self, "get_c2s_lead_status");
        let lead_id = match args.get("lead_id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return json!({ "success": false, "error": "lead_id is required" }),
        };
        match state.c2s.fetch_lead(lead_id).await {
            Ok(resp) => json!({ "success": true, "lead": serde_json::to_value(&resp.data).unwrap_or(Value::Null) }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    }

    // ─── Company/Meilisearch Tool Handlers (RML-1110) ───────────

    async fn handle_find_companies_by_cpf(&self, args: &Value) -> Value {
        let state = require_state!(self, "find_companies_by_cpf");
        let cpf = match args.get("cpf").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => return json!({ "success": false, "error": "cpf is required" }),
        };
        let summary = state.meilisearch.find_companies_by_cpf(cpf).await;
        json!({
            "success": true,
            "cpf": cpf,
            "totalCompanies": summary.total_companies,
            "totalCapitalSocial": summary.total_capital_social,
            "companies": serde_json::to_value(&summary.companies).unwrap_or(Value::Null)
        })
    }

    async fn handle_get_company_by_cnpj(&self, args: &Value) -> Value {
        let state = require_state!(self, "get_company_by_cnpj");
        let cnpj = args.get("cnpj").and_then(|v| v.as_str())
            .unwrap_or_else(|| args.get("query").and_then(|v| v.as_str()).unwrap_or(""));
        if cnpj.is_empty() {
            return json!({ "success": false, "error": "cnpj is required" });
        }
        match state.meilisearch.get_company_by_cnpj(cnpj).await {
            Some(company) => json!({ "success": true, "company": serde_json::to_value(&company).unwrap_or(Value::Null) }),
            None => json!({ "success": true, "found": false, "message": "Company not found" }),
        }
    }

    async fn handle_search_companies(&self, args: &Value) -> Value {
        let state = require_state!(self, "search_companies");
        let query = args.get("query").and_then(|v| v.as_str())
            .or_else(|| args.get("name").and_then(|v| v.as_str()))
            .unwrap_or("");
        if query.is_empty() {
            return json!({ "success": false, "error": "query (or name) is required" });
        }
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        let results = state.meilisearch.search_companies(query, limit).await;
        json!({
            "success": true,
            "query": query,
            "count": results.len(),
            "companies": serde_json::to_value(&results).unwrap_or(Value::Null)
        })
    }

    async fn handle_format_companies_message(&self, args: &Value) -> Value {
        let state = require_state!(self, "format_companies_message");
        let cpf = match args.get("cpf").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => return json!({ "success": false, "error": "cpf is required" }),
        };
        let summary = state.meilisearch.find_companies_by_cpf(cpf).await;
        let message = MeilisearchCompanyService::format_companies_for_message(&summary);
        json!({ "success": true, "cpf": cpf, "message": message, "totalCompanies": summary.total_companies })
    }

    // ─── Property Tool Handlers (RML-1111) ──────────────────────

    async fn handle_get_properties_by_cpf(&self, args: &Value) -> Value {
        let state = require_state!(self, "get_properties_by_cpf");
        let cpf = match args.get("cpf").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => return json!({ "success": false, "error": "cpf is required" }),
        };
        match state.ibvi_property.find_properties_by_cpf(cpf).await {
            Some(summary) => json!({
                "success": true,
                "cpf": cpf,
                "totalProperties": summary.total_properties,
                "totalCurrentProperties": summary.total_current_properties,
                "totalMarketValue": summary.total_market_value,
                "totalMarketValueFormatted": summary.total_market_value_formatted,
                "totalBuiltArea": summary.total_built_area,
                "properties": serde_json::to_value(&summary.properties).unwrap_or(Value::Null)
            }),
            None => json!({ "success": true, "cpf": cpf, "found": false, "message": "No properties found for this CPF" }),
        }
    }

    async fn handle_format_property_message(&self, args: &Value) -> Value {
        let state = require_state!(self, "format_property_message");
        let cpf = match args.get("cpf").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => return json!({ "success": false, "error": "cpf is required" }),
        };
        match state.ibvi_property.find_properties_by_cpf(cpf).await {
            Some(summary) => {
                let message = IbviPropertyService::format_for_message(&summary);
                json!({ "success": true, "cpf": cpf, "message": message, "totalProperties": summary.total_properties })
            }
            None => json!({ "success": true, "cpf": cpf, "found": false, "message": "No properties found" }),
        }
    }

    async fn handle_service_health(&self) -> Value {
        if let Some(state) = &self.state {
            let db_ok = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
            json!({
                "success": true,
                "overall": if db_ok { "healthy" } else { "degraded" },
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "mode": "wired",
                "services": {
                    "database": { "status": if db_ok { "healthy" } else { "unhealthy" } },
                    "work_api": { "status": "configured", "hasToken": !self.config.worker_api_key.is_empty() },
                    "c2s_api": { "status": "configured", "hasToken": !self.config.c2s_token.is_empty() },
                    "meilisearch": { "status": "configured" },
                    "cpf_discovery": { "status": "configured" }
                }
            })
        } else {
            json!({
                "success": true,
                "overall": "unknown",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "mode": "stub",
                "services": {
                    "database": { "status": "unknown", "note": "MCP stdio mode — no DB pool" },
                    "work_api": { "status": "configured", "hasToken": !self.config.worker_api_key.is_empty() },
                    "c2s_api": { "status": "configured", "hasToken": !self.config.c2s_token.is_empty() }
                },
                "hint": "Full health check requires McpServer::with_state()"
            })
        }
    }

    fn handle_score_quality(&self, args: &Value) -> Value {
        use crate::scoring::quality::{LeadQualityInput, Address, calculate_lead_quality_score};
        let input = LeadQualityInput {
            name: args.get("name").and_then(|v| v.as_str()).map(String::from),
            phone: args.get("phone").and_then(|v| v.as_str()).map(String::from),
            email: args.get("email").and_then(|v| v.as_str()).map(String::from),
            cpf: args.get("cpf").and_then(|v| v.as_str()).map(String::from),
            enriched_name: args.get("enriched_name").and_then(|v| v.as_str()).map(String::from),
            income: args.get("income").and_then(|v| v.as_f64()),
            presumed_income: args.get("presumed_income").and_then(|v| v.as_f64()),
            addresses: args.get("addresses").and_then(|v| v.as_array()).map(|arr| {
                arr.iter().map(|a| Address {
                    neighborhood: a.get("neighborhood").and_then(|v| v.as_str()).map(String::from),
                    city: a.get("city").and_then(|v| v.as_str()).map(String::from),
                    state: a.get("state").and_then(|v| v.as_str()).map(String::from),
                }).collect()
            }).unwrap_or_default(),
            company_count: args.get("company_count").and_then(|v| v.as_u64()).map(|v| v as u32),
            total_company_capital: args.get("total_company_capital").and_then(|v| v.as_f64()),
            is_company_administrator: args.get("is_company_administrator").and_then(|v| v.as_bool()).unwrap_or(false),
            has_real_estate_sector: args.get("has_real_estate_sector").and_then(|v| v.as_bool()).unwrap_or(false),
        };
        let result = calculate_lead_quality_score(&input);
        json!({
            "success": true,
            "score": result.score,
            "grade": format!("{:?}", result.grade),
            "scoreMethod": format!("{:?}", result.score_method),
            "breakdown": {
                "dataCompleteness": result.breakdown.data_completeness,
                "incomeScore": result.breakdown.income_score,
                "locationScore": result.breakdown.location_score,
                "contactValidity": result.breakdown.contact_validity,
                "enrichmentBonus": result.breakdown.enrichment_bonus,
            },
            "flags": result.flags,
        })
    }

    fn handle_assess_risk(&self, args: &Value) -> Value {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.len() < 3 {
            return json!({ "success": false, "error": "Name must be at least 3 characters" });
        }
        use crate::risk_detector::RiskDetectorService;
        let alert = RiskDetectorService::quick_check(name);
        let has_risk = alert.is_some();
        json!({
            "success": true,
            "name": name,
            "hasKnownRisk": has_risk,
            "alert": alert.as_ref().map(|a| json!({
                "type": format!("{:?}", a.category),
                "severity": format!("{:?}", a.severity),
                "title": a.title,
                "description": a.description,
            })),
            "recommendation": if has_risk { "Review before proceeding" } else { "No known risks" },
        })
    }

    fn handle_quick_risk(&self, args: &Value) -> Value {
        // quick_risk_check is a simpler version - delegates to same logic
        self.handle_assess_risk(args)
    }

    fn handle_analyze_domain(&self, args: &Value) -> Value {
        let email = args.get("email").and_then(|v| v.as_str()).unwrap_or("");
        if !email.contains('@') {
            return json!({ "success": false, "error": "Invalid email format" });
        }
        use crate::domain_analyzer::DomainAnalyzerService;
        let result = DomainAnalyzerService::analyze(email);
        json!({
            "success": true,
            "email": email,
            "domain": result.domain,
            "analysis": {
                "isPersonalEmail": result.is_personal_email,
                "isBusinessEmail": result.is_business_email,
                "trustScore": result.trust_score,
            },
            "company": result.company_name.as_ref().map(|name| json!({
                "name": name,
                "sector": result.sector,
            })),
        })
    }

    fn handle_domain_trust(&self, args: &Value) -> Value {
        let email = args.get("email").and_then(|v| v.as_str()).unwrap_or("");
        if !email.contains('@') {
            return json!({ "success": false, "error": "Invalid email format" });
        }
        use crate::domain_analyzer::DomainAnalyzerService;
        let result = DomainAnalyzerService::analyze(email);
        let level = if result.trust_score >= 70 { "high" } else if result.trust_score >= 40 { "medium" } else { "low" };
        json!({
            "success": true,
            "email": email,
            "domain": result.domain,
            "trustScore": result.trust_score,
            "level": level,
            "isPersonal": result.is_personal_email,
            "isBusiness": result.is_business_email,
        })
    }

    fn handle_identify_company(&self, args: &Value) -> Value {
        let email = args.get("email").and_then(|v| v.as_str()).unwrap_or("");
        if !email.contains('@') {
            return json!({ "success": false, "error": "Invalid email format" });
        }
        use crate::domain_analyzer::DomainAnalyzerService;
        let result = DomainAnalyzerService::analyze(email);
        match &result.company_name {
            Some(name) => json!({
                "success": true,
                "found": true,
                "domain": result.domain,
                "company": { "name": name, "sector": result.sector },
            }),
            None => json!({
                "success": true,
                "found": false,
                "domain": result.domain,
                "message": "No company identified for this domain",
            }),
        }
    }

    fn handle_calculate_tier(&self, args: &Value) -> Value {
        use crate::scoring::tier::{calculate_tier, TierEnrichmentData};
        use crate::scoring::quality::Address;
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let phone = args.get("phone").and_then(|v| v.as_str());
        let email = args.get("email").and_then(|v| v.as_str());

        let enrichment = TierEnrichmentData {
            income: args.get("income").and_then(|v| v.as_f64()),
            addresses: args.get("neighborhood").and_then(|v| v.as_str()).map(|n| {
                vec![Address {
                    neighborhood: Some(n.to_string()),
                    city: args.get("city").and_then(|v| v.as_str()).map(String::from),
                    state: args.get("state").and_then(|v| v.as_str()).map(String::from),
                }]
            }).unwrap_or_default(),
            property_count: args.get("property_count").and_then(|v| v.as_u64()).map(|v| v as u32),
            total_company_capital: args.get("total_company_capital").and_then(|v| v.as_f64()),
            is_company_administrator: args.get("is_company_administrator").and_then(|v| v.as_bool()),
        };

        let result = calculate_tier(name, phone, email, Some(&enrichment), None);
        json!({
            "success": true,
            "name": name,
            "tier": format!("{:?}", result.tier),
            "tierLabel": result.tier_label,
            "score": result.score,
            "highlights": result.highlights,
            "recommendation": {
                "action": result.recommendation_action,
                "title": result.recommendation_title,
                "description": result.recommendation_description,
            },
        })
    }

    fn handle_tier_recommendation(&self, args: &Value) -> Value {
        let tier = args.get("tier").and_then(|v| v.as_str()).unwrap_or("bronze");
        let (action, title, description) = match tier.to_lowercase().as_str() {
            "platinum" | "s" => ("priority", "Lead Premium", "Contato imediato — perfil de altíssimo valor. SLA: 2 horas."),
            "gold" | "a" => ("priority", "Lead Alto Valor", "Contato prioritário. SLA: 24 horas."),
            "silver" | "b" => ("qualify", "Lead Qualificado", "Qualificar interesse e agendar contato. SLA: 48 horas."),
            "bronze" | "c" => ("contact", "Lead Standard", "Contato padrão. SLA: 72 horas."),
            "risk" => ("avoid", "Lead com Risco", "Verificar alertas antes de prosseguir."),
            _ => ("contact", "Lead", "Contato padrão."),
        };
        json!({
            "success": true,
            "tier": tier,
            "recommendation": { "action": action, "title": title, "description": description },
        })
    }

    fn handle_generate_report(&self, args: &Value) -> Value {
        use crate::report::{ProfileReportService, ReportPerson, ReportOptions};
        let persons: Vec<ReportPerson> = args.get("persons")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(|p| ReportPerson {
                name: p.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                cpf: p.get("cpf").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                occupation: p.get("occupation").and_then(|v| v.as_str()).map(String::from),
                company: p.get("company").and_then(|v| v.as_str()).map(String::from),
                income: p.get("income").and_then(|v| v.as_f64()),
                birth_date: None,
                gender: None,
                phones: vec![],
                emails: vec![],
                address: None,
            }).collect())
            .unwrap_or_default();
        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("Report");
        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("md");
        let options = ReportOptions {
            title: title.to_string(),
            subtitle: args.get("subtitle").and_then(|v| v.as_str()).map(String::from),
            classification: "Confidencial - Uso Interno".to_string(),
            include_contacts: args.get("include_contacts").and_then(|v| v.as_bool()).unwrap_or(true),
            include_income: args.get("include_income").and_then(|v| v.as_bool()).unwrap_or(true),
            output_dir: None,
        };
        let service = ProfileReportService;
        let result = if format == "html" {
            service.generate_html(&persons, &options)
        } else {
            service.generate_markdown(&persons, &options)
        };
        let content_str = result.content.unwrap_or_default();
        json!({
            "success": result.success,
            "format": result.format,
            "personCount": persons.len(),
            "title": title,
            "contentLength": content_str.len(),
            "content": content_str,
        })
    }

    fn handle_analyze_name(&self, args: &Value) -> Value {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let phone = args.get("phone").and_then(|v| v.as_str());
        let parts: Vec<&str> = name.split_whitespace().collect();
        let surnames: Vec<&str> = if parts.len() > 1 { parts[1..].to_vec() } else { vec![] };

        // Check notable families
        use crate::scoring::families::analyze_full_name;
        let analyses = analyze_full_name(name);
        let notable: Vec<String> = analyses.iter()
            .filter(|a| a.is_notable_family)
            .map(|a| a.surname.clone())
            .collect();
        let rare: Vec<String> = analyses.iter()
            .filter(|a| a.is_rare)
            .map(|a| a.surname.clone())
            .collect();

        let is_international = phone.map(|p| {
            let digits: String = p.chars().filter(|c| c.is_ascii_digit()).collect();
            digits.len() > 11 && !digits.starts_with("55")
        }).unwrap_or(false);

        json!({
            "success": true,
            "name": name,
            "phone": phone,
            "analysis": {
                "surnames": surnames,
                "notableFamilies": notable,
                "rareSurnames": rare,
                "international": { "detected": is_international },
            },
            "hasHighValueIndicators": !notable.is_empty() || !rare.is_empty(),
        })
    }

    fn handle_twenty_route(&self, args: &Value) -> Value {
        let tier = args.get("tier").and_then(|v| v.as_str()).unwrap_or("C");
        let workspace = match tier.to_uppercase().as_str() {
            "S" | "A" => "WS-SENIOR",
            _ => "WS-GENERAL",
        };
        json!({
            "success": true,
            "leadId": args.get("lead_id"),
            "tier": tier,
            "workspace": workspace,
            "message": format!("Lead routed to {} based on tier {}", workspace, tier),
        })
    }

    fn handle_twenty_intent(&self, args: &Value) -> Value {
        use crate::twenty::IntentSignalInput;
        let input = IntentSignalInput {
            source: args.get("source").and_then(|v| v.as_str()).map(String::from),
            last_contact_date: args.get("last_contact_date").and_then(|v| v.as_str()).map(String::from),
            next_contact_date: args.get("next_contact_date").and_then(|v| v.as_str()).map(String::from),
        };
        // Inline intent signal calculation (same logic as TwentyService::calculate_intent_signal)
        let is_paid = input.source.as_deref().map(|s| {
            matches!(s, "google_ads" | "facebook_ads" | "instagram_ads" | "paid" | "ads")
        }).unwrap_or(false);
        let has_recent_contact = input.last_contact_date.as_deref().map(|d| {
            chrono::NaiveDate::parse_from_str(&d[..10], "%Y-%m-%d")
                .map(|date| (chrono::Utc::now().date_naive() - date).num_days() <= 14)
                .unwrap_or(false)
        }).unwrap_or(false);
        let has_followup = input.next_contact_date.is_some();
        let signal = if is_paid && has_recent_contact && has_followup { "high" }
            else if has_recent_contact || has_followup { "medium" }
            else { "low" };
        json!({
            "success": true,
            "intentSignal": signal,
            "leadId": args.get("lead_id"),
            "factors": { "isPaidSource": is_paid, "hasRecentContact": has_recent_contact, "hasFollowUp": has_followup },
        })
    }

    fn handle_twenty_next_action(&self, args: &Value) -> Value {
        let status = args.get("lead_status").and_then(|v| v.as_str()).unwrap_or("novo");
        let tier = args.get("tier").and_then(|v| v.as_str()).unwrap_or("C");
        let is_premium = matches!(tier.to_uppercase().as_str(), "S" | "A");
        let (action, priority, reason) = match status {
            "novo" => ("Fazer primeiro contato", if is_premium { "high" } else { "medium" }, "Lead novo aguardando primeiro contato"),
            "contato_inicial" => ("Qualificar interesse", "medium", "Lead contatado, precisa qualificar"),
            "qualificado" => ("Agendar visita", "medium", "Lead qualificado, agendar visita"),
            "visita_agendada" => ("Confirmar visita", "high", "Visita agendada, confirmar presença"),
            "visita_realizada" => ("Enviar proposta", "high", "Visita realizada, enviar proposta"),
            "proposta_enviada" => ("Follow-up proposta", "high", "Proposta enviada, acompanhar"),
            "negociacao" => ("Negociar termos", "high", "Em negociação ativa"),
            _ => ("Verificar status", "low", "Status requer revisão"),
        };
        json!({
            "success": true,
            "leadId": args.get("lead_id"),
            "currentStatus": status,
            "tier": tier,
            "primaryAction": { "action": action, "priority": priority, "reason": reason },
        })
    }
}

// ─── ServerHandler impl ────────────────────────────────────────────

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "rust-c2s-api-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: Some("C2S Lead Enrichment MCP Server".into()),
                description: Some("66 tools for lead discovery, enrichment, scoring, and CRM integration".into()),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "C2S Lead Enrichment API — 66 tools for lead discovery, enrichment, \
                 scoring, risk assessment, CRM integration, and reporting. \
                 Tools that require database/HTTP are stubbed in stdio mode.".into()
            ),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource::new("enrichment://stats", "Enrichment Statistics".to_string()).no_annotation(),
                RawResource::new("enrichment://health", "Service Health".to_string()).no_annotation(),
                RawResource::new("enrichment://recent", "Recent Leads".to_string()).no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri.as_str();
        match uri {
            "enrichment://stats" => {
                let data = json!({
                    "note": "Stats require database connection — use HTTP API /stats endpoint",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(
                        serde_json::to_string_pretty(&data).unwrap_or_default(),
                        request.uri,
                    )],
                })
            }
            "enrichment://health" => {
                let data = self.handle_service_health().await;
                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(
                        serde_json::to_string_pretty(&data).unwrap_or_default(),
                        request.uri,
                    )],
                })
            }
            "enrichment://recent" => {
                let data = json!({
                    "note": "Recent leads require database connection — use HTTP API /stats endpoint",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(
                        serde_json::to_string_pretty(&data).unwrap_or_default(),
                        request.uri,
                    )],
                })
            }
            _ => Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": uri })),
            )),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: Self::tool_definitions(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name: &str = &request.name;
        let args = request.arguments
            .map(|obj| Value::Object(obj))
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let result = self.dispatch_tool(name, args).await;
        let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

// ─── Tests ─────────────────────────────────────────────────────────


// ─── Helpers ────────────────────────────────────────────────────

fn parse_workspace(s: Option<&str>) -> crate::twenty::Workspace {
    match s {
        Some("WS-OPS" | "ws-ops" | "ops") => crate::twenty::Workspace::WsOps,
        Some("WS-SENIOR" | "ws-senior" | "senior") => crate::twenty::Workspace::WsSenior,
        _ => crate::twenty::Workspace::WsGeneral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            database_url: "postgresql://test@localhost/test".to_string(),
            port: 3000,
            c2s_token: "test_token".to_string(),
            c2s_base_url: "https://test.c2s.com".to_string(),
            webhook_secret: None,
            worker_api_key: "test_work_api".to_string(),
            diretrix_base_url: "https://diretrix.example.com".to_string(),
            diretrix_user: "test".to_string(),
            diretrix_pass: "test".to_string(),
            dbase_key: "test".to_string(),
            mimir_token: None,
            google_ads_webhook_key: None,
            c2s_default_seller_id: None,
            c2s_description_max_length: 5000,
            cpf_lookup_api_url: "https://cpf-lookup.test".to_string(),
            cpf_lookup_timeout_ms: 30000,
            income_multiplier: 1.9,
            cron_interval_business_secs: 300,
            cron_interval_evening_secs: 600,
            cron_interval_night_secs: 1800,
            cron_enabled: false,
            meilisearch_url: "https://test.meili.dev".to_string(),
            meilisearch_key: "test_key".to_string(),
            meilisearch_auto_scale: false,
            meilisearch_app_name: "test-meili".to_string(),
            meilisearch_machine_id: None,
            meilisearch_fly_api_token: None,
            twenty_base_url: "https://twenty.example.com".to_string(),
            twenty_api_key: "test_twenty".to_string(),
            twenty_api_key_ws_ops: None,
            twenty_api_key_ws_senior: None,
            twenty_api_key_ws_general: None,
            twenty_enabled: false,
        }
    }

    #[test]
    fn test_tool_count() {
        let tools = McpServer::tool_definitions();
        assert_eq!(tools.len(), 66, "Must have exactly 66 tools");
    }

    #[test]
    fn test_tool_names_unique() {
        let tools = McpServer::tool_definitions();
        let mut names: Vec<&str> = tools.iter().map(|t| &*t.name).collect();
        let total = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), total, "All tool names must be unique");
    }

    #[test]
    fn test_all_tools_have_descriptions() {
        let tools = McpServer::tool_definitions();
        for tool in &tools {
            assert!(tool.description.is_some(), "Tool '{}' missing description", tool.name);
            assert!(!tool.description.as_ref().unwrap().is_empty(), "Tool '{}' has empty description", tool.name);
        }
    }

    #[test]
    fn test_validate_cpf_valid() {
        let server = McpServer::new(test_config());
        let result = server.handle_validate_cpf(&json!({ "cpf": "52998224725" }));
        assert_eq!(result["success"], true);
        assert_eq!(result["isValid"], true);
    }

    #[test]
    fn test_validate_cpf_invalid() {
        let server = McpServer::new(test_config());
        let result = server.handle_validate_cpf(&json!({ "cpf": "11111111111" }));
        assert_eq!(result["success"], true);
        assert_eq!(result["isValid"], false);
    }

    #[test]
    fn test_score_quality() {
        let server = McpServer::new(test_config());
        let result = server.handle_score_quality(&json!({
            "name": "João Silva",
            "phone": "11999887766",
            "email": "joao@empresa.com.br",
            "cpf": "52998224725",
            "income": 25000.0,
        }));
        assert_eq!(result["success"], true);
        assert!(result["score"].as_u64().unwrap() > 0);
        assert!(result["grade"].as_str().is_some());
    }

    #[test]
    fn test_tier_recommendation() {
        let server = McpServer::new(test_config());
        let result = server.handle_tier_recommendation(&json!({ "tier": "platinum" }));
        assert_eq!(result["success"], true);
        assert_eq!(result["recommendation"]["action"], "priority");
    }

    #[test]
    fn test_analyze_domain_personal() {
        let server = McpServer::new(test_config());
        let result = server.handle_analyze_domain(&json!({ "email": "joao@gmail.com" }));
        assert_eq!(result["success"], true);
        // Gmail is a personal email domain
        assert_eq!(result["analysis"]["isPersonalEmail"], true);
        assert_eq!(result["analysis"]["isBusinessEmail"], false);
    }

    #[test]
    fn test_domain_trust() {
        let server = McpServer::new(test_config());
        let result = server.handle_domain_trust(&json!({ "email": "ceo@empresa.com.br" }));
        assert_eq!(result["success"], true);
        assert!(result["trustScore"].as_u64().is_some());
    }

    #[test]
    fn test_twenty_route() {
        let server = McpServer::new(test_config());
        let result = server.handle_twenty_route(&json!({ "lead_id": "123", "tier": "S" }));
        assert_eq!(result["workspace"], "WS-SENIOR");
        let result2 = server.handle_twenty_route(&json!({ "lead_id": "456", "tier": "C" }));
        assert_eq!(result2["workspace"], "WS-GENERAL");
    }

    #[test]
    fn test_twenty_next_action() {
        let server = McpServer::new(test_config());
        let result = server.handle_twenty_next_action(&json!({ "lead_status": "novo", "tier": "S" }));
        assert_eq!(result["success"], true);
        assert!(result["primaryAction"].is_object());
    }

    #[test]
    fn test_analyze_name() {
        let server = McpServer::new(test_config());
        let result = server.handle_analyze_name(&json!({ "name": "João Safra Silva" }));
        assert_eq!(result["success"], true);
        assert_eq!(result["hasHighValueIndicators"], true);
    }

    #[test]
    fn test_generate_report() {
        let server = McpServer::new(test_config());
        let result = server.handle_generate_report(&json!({
            "title": "Test Report",
            "persons": [{ "name": "João Silva", "cpf": "12345678901", "income": 25000.0 }],
            "format": "md",
        }));
        assert!(result["format"].as_str().is_some());
    }

    #[test]
    fn test_quick_risk() {
        let server = McpServer::new(test_config());
        let result = server.handle_quick_risk(&json!({ "name": "João Silva" }));
        assert_eq!(result["success"], true);
    }

    #[tokio::test]
    async fn test_service_health() {
        let server = McpServer::new(test_config());
        let result = server.handle_service_health().await;
        assert_eq!(result["success"], true);
        assert!(result["services"].is_object());
    }

    #[tokio::test]
    async fn test_dispatch_unknown_tool() {
        let server = McpServer::new(test_config());
        let result = server.dispatch_tool("nonexistent_tool", json!({})).await;
        assert_eq!(result["success"], false);
        assert!(result["error"].as_str().unwrap().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_dispatch_stub_tool() {
        let server = McpServer::new(test_config());
        let result = server.dispatch_tool("enrich_lead", json!({ "phone": "11999887766" })).await;
        assert_eq!(result["success"], false);
        assert!(result["error"].as_str().unwrap().contains("requires database"));
    }
}
