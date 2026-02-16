//! AI command interpreter — proxies natural language to OpenRouter,
//! returns a structured command for the CRM AI Chat frontend.
//! The API key never leaves the server.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::handlers::AppState;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const MODEL: &str = "google/gemini-2.5-flash-preview";

const SYSTEM_PROMPT: &str = r#"You are a CRM command router for a real estate company (MBRAS). Convert the user's natural language message into a structured command.

Available commands:

PERSON LOOKUP
- cpf <number> : Lookup person by CPF (11 digits)
- name <full name> : Search person by name
- phone <number> : Search by phone number
- email <address> : Search by email
- customer <uuid> : Get customer by ID

ENRICHMENT
- enrich <cpf|phone> : Enrich lead data (11 digits = CPF, else phone)
- work <module> <value> : Raw Work API (modules: phone, cpf, name, mail, cep)
- batch <cpf,cpf,...> : Batch enrichment
- batch-retry : Retry failed enrichments

COMPANIES
- company <cpf> : Find companies by owner CPF
- cnpj <number> : Lookup company by CNPJ (14 digits)
- search <query> : Search companies by name

PROPERTIES
- property <cpf> : Properties by owner CPF

ANALYSIS
- analyze <lead_id> : Deep lead analysis
- get-analysis <lead_id> : Get cached analysis

C2S CRM
- sellers : List all sellers
- seller <id> : Seller details
- find-phone <phone> : Find lead by phone
- find-email <email> : Find lead by email
- lead-status <lead_id> : Lead enrichment status
- forward <lead_id> <seller_id> : Forward lead to seller
- interact <lead_id> : Mark lead as interacted
- tags : List available tags
- create-tag <name> : Create new tag
- lead-tags <lead_id> : Get lead's tags
- tag <lead_id> <tag_name> : Add tag to lead

ACTIVITIES
- note <lead_id> <text> : Add note to lead
- call <lead_id> : Register phone call
- email-lead <lead_id> <text> : Register email sent
- meeting <lead_id> : Register meeting
- task <lead_id> <description> : Create task

DISTRIBUTION
- distribute : Distribute leads
- auto-assign : Auto-assign leads

TWENTY CRM
- pipeline : Pipeline statistics
- broker-stats : Broker performance
- sla : SLA violations
- lead-action <lead_id> : Recommended next action
- lead-sla <lead_id> : SLA status for lead

REPORTS
- report <cpf,cpf,...> : Generate HTML report

MONITORING
- health : API health check
- stats : Enrichment statistics
- services : Service health statuses
- dashboard : Dashboard data
- help : Show all commands

RULES:
1. Respond ONLY with valid JSON: {"command": "<cmd>", "args": "<value>"}
2. Extract numbers, names, emails from the user's message as args
3. If multiple args needed (like forward), separate with space
4. If no args needed (like health, sellers, tags), use empty string
5. Clean phone numbers: remove parentheses, dashes, spaces, country code +55
6. Clean CPF: remove dots and dashes, keep 11 digits
7. If the user asks something conversational or you can't determine a command, respond: {"command": "help", "args": ""}
8. Understand Portuguese (BR) and English

Examples:
User: "quem é o dono do telefone 11 99887-7766?"
{"command": "phone", "args": "11998877766"}

User: "busca o CPF 123.456.789-01"
{"command": "cpf", "args": "12345678901"}

User: "mostre as empresas do cpf 40749390883"
{"command": "company", "args": "40749390883"}

User: "lista os vendedores"
{"command": "sellers", "args": ""}

User: "como está o sistema?"
{"command": "health", "args": ""}

User: "adiciona uma nota no lead abc123 dizendo que liguei hoje"
{"command": "note", "args": "abc123 Liguei hoje"}

User: "encaminha o lead xyz para o vendedor 42"
{"command": "forward", "args": "xyz 42"}
"#;

#[derive(Deserialize)]
pub struct InterpretRequest {
    pub input: String,
}

#[derive(Serialize)]
pub struct InterpretResponse {
    pub command: String,
    pub args: String,
}

pub async fn ai_interpret(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InterpretRequest>,
) -> Result<Json<InterpretResponse>, (StatusCode, Json<serde_json::Value>)> {
    let api_key = state.config.openrouter_api_key.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "AI interpreter not configured"})),
        )
    })?;

    if body.input.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Input cannot be empty"})),
        ));
    }

    let client = reqwest::Client::new();

    let openrouter_body = json!({
        "model": MODEL,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": body.input.trim() }
        ],
        "temperature": 0.1,
        "max_tokens": 256,
        "response_format": { "type": "json_object" }
    });

    let resp = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://crm-ai-chat.fly.dev")
        .header("X-Title", "CRM AI Chat")
        .timeout(std::time::Duration::from_secs(15))
        .json(&openrouter_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("OpenRouter request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "AI service unavailable"})),
            )
        })?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| {
        tracing::error!("OpenRouter read error: {e}");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "AI response read error"})),
        )
    })?;

    if !status.is_success() {
        tracing::error!("OpenRouter HTTP {status}: {text}");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("AI error (HTTP {})", status.as_u16())})),
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        tracing::error!("OpenRouter parse error: {e}");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "AI response parse error"})),
        )
    })?;

    let content = json_resp
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| {
            tracing::error!("OpenRouter returned no content");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "AI returned no content"})),
            )
        })?;

    let cmd_json: serde_json::Value = serde_json::from_str(content).map_err(|e| {
        tracing::error!("AI command parse error: {e}, raw: {content}");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "AI command parse error"})),
        )
    })?;

    let command = cmd_json
        .get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("help")
        .to_string();

    let args = cmd_json
        .get("args")
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();

    tracing::info!(command = %command, "AI interpret: mapped to command");

    Ok(Json(InterpretResponse { command, args }))
}
