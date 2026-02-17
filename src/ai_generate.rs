//! AI content generator — produces brand-compliant content from customer context.
//! Uses higher temperature and longer output than the interpret endpoint.
//! Templates enforce MBRAS luxury tone, forbidden claims, and PT-BR register.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::handlers::AppState;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

// ─── Templates ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Template {
    WhatsappReply,
    EmailDraft,
    InstagramCaption,
    ListingDescription,
    ReelScript,
}

impl Template {
    fn max_chars(&self) -> usize {
        match self {
            Self::WhatsappReply => 500,
            Self::EmailDraft => 2000,
            Self::InstagramCaption => 2200,
            Self::ListingDescription => 2000,
            Self::ReelScript => 1000,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::WhatsappReply => "WhatsApp Reply",
            Self::EmailDraft => "Email Draft",
            Self::InstagramCaption => "Instagram Caption",
            Self::ListingDescription => "Listing Description",
            Self::ReelScript => "Reel Script",
        }
    }

    fn instructions(&self) -> &'static str {
        match self {
            Self::WhatsappReply => WHATSAPP_INSTRUCTIONS,
            Self::EmailDraft => EMAIL_INSTRUCTIONS,
            Self::InstagramCaption => INSTAGRAM_INSTRUCTIONS,
            Self::ListingDescription => LISTING_INSTRUCTIONS,
            Self::ReelScript => REEL_INSTRUCTIONS,
        }
    }
}

// ─── Brand guardrails (IBVI-367) ─────────────────────────────────────────────

const BRAND_SYSTEM_PROMPT: &str = r#"You are a luxury real estate content writer for MBRAS, São Paulo's premier high-end brokerage. Every piece of content you produce must embody sophistication, confidence, and exclusivity.

BRAND VOICE RULES:
- Confident and exclusive, never salesy or desperate
- PT-BR formal register — no internet slang, no diminutives, no excessive exclamation marks
- Luxury vocabulary: "exclusivo", "sofisticado", "alto padrão", "requintado", "singular"
- Short, impactful sentences. Every word must earn its place.
- Always address the client with respect ("você" formal, never "tu")

FORBIDDEN — NEVER include:
- Unverified superlatives: "o melhor de São Paulo", "o mais exclusivo", "incomparável"
- Investment/return promises: "valorização garantida", "retorno certo", "investimento seguro"
- Misleading claims about area, price, or specifications
- Competitor mentions or comparisons
- Generic real estate clichés: "oportunidade única", "não perca", "imperdível"
- Emojis in formal content (WhatsApp may use 1-2 max, never in email/listing)

REQUIRED ELEMENTS:
- Every outbound message must include a clear CTA (call to action)
- Formal content (email, listing) should reference CRECI when appropriate
- Always mention the neighborhood name when a property is referenced
- If any property data is missing or uncertain, mark it as [DADOS INDISPONÍVEIS] — never invent values

DATA INTEGRITY:
- Use ONLY the data provided in the customer context. Do not fabricate any personal, financial, or property information.
- Numerical claims (price, area, R$/m²) must come directly from the context provided.
- If critical data is missing, add: "⚠ Confirme os dados antes de enviar ao cliente."

LANGUAGE: Always respond in PT-BR (Brazilian Portuguese) unless explicitly asked for English.
"#;

const WHATSAPP_INSTRUCTIONS: &str = r#"Generate a WhatsApp message reply.

RULES:
- Maximum 500 characters
- Short, confident, high-end tone
- 1-2 message variants (separated by ---)
- Must include a CTA (suggest next step: visit, call, meeting)
- May use 1-2 tasteful emojis maximum (🏠, ✨ — never 🔥🚀💰)
- Ready to copy-paste into WhatsApp

FORMAT:
[Message variant 1]
---
[Message variant 2 (shorter alternative)]"#;

const EMAIL_INSTRUCTIONS: &str = r#"Generate a professional email draft.

RULES:
- Subject line: max 60 characters, compelling but not clickbait
- Body: greeting → context → main content → CTA → signature placeholder
- Formal luxury tone — more polished than WhatsApp
- Max 2000 characters total
- Include [CRECI XXXXX] placeholder in signature area

FORMAT:
**Assunto:** [subject line]

[email body]

Atenciosamente,
[Nome do Corretor]
MBRAS | [CRECI XXXXX]"#;

const INSTAGRAM_INSTRUCTIONS: &str = r#"Generate an Instagram caption.

RULES:
- Max 2200 characters
- Structure: HOOK (first line, attention-grabbing) → BODY (2-3 short paragraphs) → CTA → HASHTAGS
- Hook styles: luxury lifestyle / educational / storytelling (pick the best for context)
- 15-20 hashtags: mix of branded (#MBRAS #AltoParadrao), location (#JardinsVNCVilaNova), and niche (#ImoveisdeLuxoSP)
- Use line breaks for readability
- May include emojis between sections (tasteful, 3-5 total)

FORMAT:
[Hook line — attention grabber]

[Body paragraph 1]

[Body paragraph 2]

[CTA — "Link na bio", "Agende sua visita", etc.]

.
.
.
[hashtags]"#;

const LISTING_INSTRUCTIONS: &str = r#"Generate a luxury property listing description.

RULES:
- Max 2000 characters
- Structure: HEADLINE → NARRATIVE → SPECS → DIFFERENTIATORS → NEIGHBORHOOD
- Headline: compelling one-liner that captures the essence
- Narrative: 2-3 sentences painting the lifestyle, not just the specs
- Specs: structured list (area, rooms, parking, etc.) — only include what's in the data
- Differentiators: what makes this property unique
- Neighborhood: brief context about the area
- Mark any missing spec as [DADOS INDISPONÍVEIS]

FORMAT:
## [Headline]

[Narrative paragraph]

**Características:**
- [spec 1]
- [spec 2]
- ...

**Diferenciais:**
[What makes it special]

**Localização:**
[Neighborhood context]"#;

const REEL_INSTRUCTIONS: &str = r#"Generate a short video script for Instagram Reels.

RULES:
- 15-60 seconds duration (mark timestamps)
- Structure: HOOK (0-3s) → BODY (4-50s) → CTA (last 5-10s)
- Hook must grab attention immediately — question, bold statement, or visual cue
- Body: 3-5 short talking points or scene descriptions
- CTA: clear action (follow, save, comment, link in bio)
- Include camera/visual directions in [brackets]
- Max 1000 characters

FORMAT:
**[HOOK 0-3s]**
[Visual direction] "Opening line"

**[BODY 4-50s]**
[Scene 1] "..."
[Scene 2] "..."
[Scene 3] "..."

**[CTA últimos 5-10s]**
[Visual direction] "Closing line + CTA""#;

// ─── Request/Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub template: Template,
    pub context: serde_json::Value,
    pub instructions: Option<String>,
    pub model: Option<String>,
}

#[derive(Serialize)]
pub struct GenerateResponse {
    pub content: String,
    pub template: String,
    pub model: String,
    pub tokens_used: Option<u64>,
}

// ─── Handler ─────────────────────────────────────────────────────────────────

/// POST /api/v1/ai/generate — generate brand-compliant content from customer context
pub async fn ai_generate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, Json<serde_json::Value>)> {
    let api_key = state.config.openrouter_api_key.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "AI generator not configured"})),
        )
    })?;

    // For content generation, default to smart model (best writing quality)
    let model = match body.model.as_deref() {
        Some("fast") => &state.config.model_fast,
        Some("base") => &state.config.model_base,
        Some("smart") => &state.config.model_smart,
        _ => &state.config.model_smart, // default to smart for writing quality
    };

    // Build context summary from customer data
    let context_summary = build_context_summary(&body.context);

    // Build user prompt
    let user_prompt = build_user_prompt(&body.template, &context_summary, body.instructions.as_deref());

    let client = reqwest::Client::new();

    let openrouter_body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": format!("{}\n\n{}", BRAND_SYSTEM_PROMPT, body.template.instructions()) },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.7,
        "max_tokens": 2048,
    });

    tracing::info!(
        template = %body.template.label(),
        model = %model,
        "AI generate request"
    );

    let resp = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://mbras-c2s.fly.dev")
        .header("X-Title", "CRM AI Chat")
        .timeout(std::time::Duration::from_secs(60))
        .json(&openrouter_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("OpenRouter generate request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "AI service unavailable"})),
            )
        })?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| {
        tracing::error!("OpenRouter generate read error: {e}");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "AI response read error"})),
        )
    })?;

    if !status.is_success() {
        tracing::error!("OpenRouter generate HTTP {status}: {text}");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("AI error (HTTP {})", status.as_u16())})),
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        tracing::error!("OpenRouter generate parse error: {e}");
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
            tracing::error!("OpenRouter generate returned no content");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "AI returned no content"})),
            )
        })?;

    let tokens_used = json_resp
        .get("usage")
        .and_then(|u| u.get("total_tokens"))
        .and_then(|t| t.as_u64());

    tracing::info!(
        template = %body.template.label(),
        model = %model,
        tokens = ?tokens_used,
        content_len = content.len(),
        "AI generate complete"
    );

    Ok(Json(GenerateResponse {
        content: content.to_string(),
        template: body.template.label().to_string(),
        model: model.clone(),
        tokens_used,
    }))
}

// ─── Context builder ─────────────────────────────────────────────────────────

/// Extract key fields from UnifiedCustomerResponse into a readable summary for the AI
fn build_context_summary(ctx: &serde_json::Value) -> String {
    let mut parts = Vec::new();

    // Personal info
    if let Some(pi) = ctx.get("personal_info") {
        let name = pi.get("name").and_then(|v| v.as_str()).unwrap_or("[DADOS INDISPONÍVEIS]");
        parts.push(format!("Cliente: {name}"));

        if let Some(gender) = pi.get("gender").and_then(|v| v.as_str()) {
            parts.push(format!("Gênero: {gender}"));
        }
        if let Some(marital) = pi.get("marital_status").and_then(|v| v.as_str()) {
            parts.push(format!("Estado civil: {marital}"));
        }
    }

    // Financial info
    if let Some(fi) = ctx.get("financial_info") {
        if let Some(income) = fi.get("estimated_income").or(fi.get("income")).and_then(|v| v.as_f64()) {
            parts.push(format!("Renda estimada: R$ {:.2}", income));
        }
        if let Some(score) = fi.get("credit_score").and_then(|v| v.as_f64()) {
            if score > 0.0 {
                parts.push(format!("Score de crédito: {:.0}", score));
            }
        }
        if let Some(range) = fi.get("income_range").and_then(|v| v.as_str()) {
            parts.push(format!("Faixa de renda: {range}"));
        }
    }

    // Wealth assessment
    if let Some(wa) = ctx.get("wealth_assessment") {
        if let Some(tier) = wa.get("tier").and_then(|v| v.as_str()) {
            parts.push(format!("Tier: {tier}"));
        }
        if let Some(label) = wa.get("tier_label").and_then(|v| v.as_str()) {
            parts.push(format!("Perfil: {label}"));
        }
        if let Some(nw) = wa.get("estimated_net_worth").and_then(|v| v.as_f64()) {
            if nw > 0.0 {
                parts.push(format!("Patrimônio estimado: R$ {:.2}", nw));
            }
        }
    }

    // Contact info
    if let Some(ci) = ctx.get("contact_info") {
        if let Some(phones) = ci.get("phones").and_then(|v| v.as_array()) {
            let phone_list: Vec<&str> = phones.iter()
                .filter_map(|p| p.get("phone").and_then(|v| v.as_str()))
                .take(3)
                .collect();
            if !phone_list.is_empty() {
                parts.push(format!("Telefones: {}", phone_list.join(", ")));
            }
        }
        if let Some(emails) = ci.get("emails").and_then(|v| v.as_array()) {
            let email_list: Vec<&str> = emails.iter()
                .filter_map(|e| e.get("email").and_then(|v| v.as_str()))
                .take(3)
                .collect();
            if !email_list.is_empty() {
                parts.push(format!("Emails: {}", email_list.join(", ")));
            }
        }
    }

    // Addresses (neighborhood context)
    if let Some(addrs) = ctx.get("addresses").and_then(|v| v.as_array()) {
        if let Some(first) = addrs.first() {
            let neighborhood = first.get("neighborhood").and_then(|v| v.as_str());
            let city = first.get("city").and_then(|v| v.as_str());
            if let Some(n) = neighborhood {
                let loc = city.map(|c| format!("{n}, {c}")).unwrap_or_else(|| n.to_string());
                parts.push(format!("Localização: {loc}"));
            }
        }
    }

    // Metadata
    if let Some(meta) = ctx.get("metadata") {
        let sources: Vec<&str> = meta.get("sources")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.as_str()).collect())
            .unwrap_or_default();
        if !sources.is_empty() {
            parts.push(format!("Fontes: {}", sources.join(", ")));
        }
    }

    if parts.is_empty() {
        "Nenhum dado de contexto disponível.".to_string()
    } else {
        parts.join("\n")
    }
}

fn build_user_prompt(template: &Template, context: &str, instructions: Option<&str>) -> String {
    let mut prompt = format!(
        "Gere um {} com base no seguinte contexto do cliente:\n\n{}\n\nLimite: {} caracteres.",
        template.label(),
        context,
        template.max_chars()
    );

    if let Some(instr) = instructions {
        if !instr.trim().is_empty() {
            prompt.push_str(&format!("\n\nInstruções adicionais: {instr}"));
        }
    }

    prompt
}
