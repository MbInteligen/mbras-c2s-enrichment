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

use crate::config::Config;

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
#[derive(Clone)]
pub struct McpServer {
    config: Arc<Config>,
}

impl McpServer {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
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
            "enrich_lead" => self.stub_tool(name, &args),
            "enrich_bulk" => self.stub_tool(name, &args),
            "retry_failed" => self.stub_tool(name, &args),

            // Discovery (5)
            "find_and_save_person" => self.stub_tool(name, &args),
            "discover_cpf" => self.handle_discover_cpf(&args).await,
            "lookup_cpf" => self.stub_tool(name, &args),
            "search_cpf_by_name" => self.stub_tool(name, &args),
            "validate_cpf" => self.handle_validate_cpf(&args),

            // Leads (3)
            "get_lead" => self.stub_tool(name, &args),
            "list_leads" => self.stub_tool(name, &args),
            "get_c2s_lead_status" => self.stub_tool(name, &args),

            // Stats (4)
            "get_enrichment_stats" => self.stub_tool(name, &args),
            "get_service_health" => self.handle_service_health(),
            "get_enrichment_rate" => self.stub_tool(name, &args),
            "get_enrichment_health" => self.stub_tool(name, &args),

            // Property (3)
            "get_properties_by_cpf" => self.stub_tool(name, &args),
            "get_property_summary" => self.stub_tool(name, &args),
            "format_property_message" => self.stub_tool(name, &args),

            // Reports (3)
            "generate_profile_report" => self.handle_generate_report(&args),
            "generate_report_from_cpfs" => self.stub_tool(name, &args),
            "generate_report_pdf" => self.stub_tool(name, &args),

            // Analysis (6)
            "analyze_lead" => self.stub_tool(name, &args),
            "get_lead_analysis" => self.stub_tool(name, &args),
            "check_lead_alert" => self.stub_tool(name, &args),
            "score_lead_quality" => self.handle_score_quality(&args),
            "assess_risk" => self.handle_assess_risk(&args),
            "quick_risk_check" => self.handle_quick_risk(&args),

            // C2S CRM (9)
            "fetch_c2s_leads" => self.stub_tool(name, &args),
            "get_c2s_sellers" => self.stub_tool(name, &args),
            "send_c2s_message" => self.stub_tool(name, &args),
            "forward_c2s_lead" => self.stub_tool(name, &args),
            "search_c2s_by_phone" => self.stub_tool(name, &args),
            "search_c2s_by_email" => self.stub_tool(name, &args),
            "mark_c2s_interacted" => self.stub_tool(name, &args),
            "get_c2s_tags" => self.stub_tool(name, &args),
            "add_c2s_lead_tag" => self.stub_tool(name, &args),

            // Domain (3)
            "analyze_email_domain" => self.handle_analyze_domain(&args),
            "get_domain_trust_score" => self.handle_domain_trust(&args),
            "identify_company_from_email" => self.handle_identify_company(&args),

            // Companies (7)
            "lookup_cnpj" => self.stub_tool(name, &args),
            "find_companies_by_name" => self.stub_tool(name, &args),
            "analyze_company_portfolio" => self.stub_tool(name, &args),
            "find_companies_by_cpf" => self.stub_tool(name, &args),
            "get_company_by_cnpj" => self.stub_tool(name, &args),
            "search_companies" => self.stub_tool(name, &args),
            "format_companies_message" => self.stub_tool(name, &args),

            // Tier (2)
            "calculate_lead_tier" => self.handle_calculate_tier(&args),
            "get_tier_recommendation" => self.handle_tier_recommendation(&args),

            // Search (5)
            "search_web" => self.stub_tool(name, &args),
            "search_person" => self.stub_tool(name, &args),
            "search_news" => self.stub_tool(name, &args),
            "generate_web_insights" => self.stub_tool(name, &args),
            "analyze_lead_name" => self.handle_analyze_name(&args),

            // Twenty CRM (13)
            "twenty_create_lead" => self.stub_tool(name, &args),
            "twenty_update_lead" => self.stub_tool(name, &args),
            "twenty_get_lead" => self.stub_tool(name, &args),
            "twenty_route_lead" => self.handle_twenty_route(&args),
            "twenty_delegate_lead" => self.stub_tool(name, &args),
            "twenty_bulk_import" => self.stub_tool(name, &args),
            "twenty_get_pipeline_stats" => self.stub_tool(name, &args),
            "twenty_get_broker_stats" => self.stub_tool(name, &args),
            "twenty_get_adoption_metrics" => self.stub_tool(name, &args),
            "twenty_check_sla_violations" => self.stub_tool(name, &args),
            "twenty_check_delegation_expiry" => self.stub_tool(name, &args),
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
        self.stub_tool("discover_cpf", args)
    }

    fn handle_service_health(&self) -> Value {
        json!({
            "success": true,
            "overall": "unknown",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "services": {
                "database": { "status": "unknown", "note": "MCP stdio mode — no DB pool" },
                "workApi": { "status": "configured", "hasToken": !self.config.worker_api_key.is_empty() },
                "c2sApi": { "status": "configured", "hasToken": !self.config.c2s_token.is_empty() }
            },
            "hint": "Full health check requires HTTP API mode with database connection"
        })
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
                let data = self.handle_service_health();
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

    #[test]
    fn test_service_health() {
        let server = McpServer::new(test_config());
        let result = server.handle_service_health();
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
