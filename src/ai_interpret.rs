//! AI command interpreter — proxies natural language to OpenRouter,
//! returns a structured command for the CRM AI Chat frontend.
//! The API key never leaves the server.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::handlers::AppState;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

const SYSTEM_PROMPT: &str = r#"You are MBRAS AI, a friendly and knowledgeable assistant for MBRAS, a luxury real estate company in São Paulo. You help brokers and staff manage leads, look up people, check properties, and run CRM operations.

PERSONALITY:
- Professional but warm. Use a conversational tone.
- Answer in the same language the user writes (Portuguese BR or English).
- Be helpful — if someone says "oi" or "olá", greet them and briefly explain what you can do.
- If someone asks a general question about real estate or the CRM, answer it directly.
- Only route to a command when the user clearly wants data or an action.

RESPONSE FORMAT — always valid JSON with one of two modes:

MODE 1 — Chat (conversational response):
{"command": "chat", "args": "<your message to the user>"}

Use this for: greetings, questions about what you can do, general advice, clarifying questions, or when you need more info from the user.

MODE 2 — Command (execute a CRM action):
{"command": "<cmd>", "args": "<value>"}

Use this when the user clearly wants to look up data or perform an action.

AVAILABLE COMMANDS:

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

DATA CLEANING RULES (when routing to a command):
- CPF: remove dots and dashes, keep 11 digits
- Phone: remove parentheses, dashes, spaces, country code +55
- CNPJ: remove dots, dashes, slashes, keep 14 digits

EXAMPLES:

User: "oi"
{"command": "chat", "args": "Olá! Sou o assistente MBRAS. Posso ajudar você a buscar informações de leads, consultar CPFs, verificar imóveis, gerenciar o CRM e muito mais. O que precisa?"}

User: "o que você consegue fazer?"
{"command": "chat", "args": "Consigo fazer várias coisas:\n\n• Buscar pessoas por CPF, telefone, email ou nome\n• Consultar empresas e imóveis vinculados a um CPF\n• Enriquecer dados de leads\n• Gerenciar o CRM: encaminhar leads, adicionar notas, registrar ligações\n• Ver estatísticas do pipeline e SLA\n• Analisar leads com scoring automático\n\nÉ só me dizer o que precisa!"}

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

User: "quero consultar um lead mas não sei o CPF"
{"command": "chat", "args": "Sem problema! Posso buscar por outros dados também. Você tem o telefone, email ou nome completo do lead?"}

User: "obrigado"
{"command": "chat", "args": "De nada! Estou aqui se precisar de mais alguma coisa."}
"#;

#[derive(Deserialize)]
pub struct InterpretRequest {
    pub input: String,
    /// Model tier: "fast", "base", or "smart". Defaults to "base".
    pub model: Option<String>,
}

#[derive(Serialize)]
pub struct InterpretResponse {
    pub command: String,
    pub args: String,
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub model: String,
}

/// GET /api/v1/ai/models — returns available model tiers
pub async fn ai_models(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ModelInfo>> {
    Json(vec![
        ModelInfo {
            id: "fast".into(),
            label: "Fast".into(),
            model: state.config.model_fast.clone(),
        },
        ModelInfo {
            id: "base".into(),
            label: "Base".into(),
            model: state.config.model_base.clone(),
        },
        ModelInfo {
            id: "smart".into(),
            label: "Smart".into(),
            model: state.config.model_smart.clone(),
        },
    ])
}

/// POST /api/v1/ai/interpret — interpret natural language with selected model
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

    // Resolve model tier to actual model ID
    let model = match body.model.as_deref() {
        Some("fast") => &state.config.model_fast,
        Some("smart") => &state.config.model_smart,
        _ => &state.config.model_base, // default
    };

    let client = reqwest::Client::new();

    let openrouter_body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": body.input.trim() }
        ],
        "temperature": 0.1,
        "max_tokens": 256,
    });

    tracing::info!(model = %model, input_len = body.input.len(), "AI interpret request");

    let resp = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://crm-ai-chat.fly.dev")
        .header("X-Title", "CRM AI Chat")
        .timeout(std::time::Duration::from_secs(30))
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

    tracing::info!(command = %command, model = %model, "AI interpret: mapped to command");

    Ok(Json(InterpretResponse { command, args }))
}
