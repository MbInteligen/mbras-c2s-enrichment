use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    config::Config,
    enrichment::{is_valid_email, validate_br_phone},
    errors::AppError,
    google_ads_models::GoogleAdsWebhookPayload,
    services::{self, WorkApiService},
};

/// Query parameters for Google Ads webhook verification.
#[derive(Debug, Deserialize)]
pub struct GoogleAdsWebhookQuery {
    /// Google's webhook verification key (required for security).
    google_key: Option<String>,
}

/// Response for Google Ads webhook.
#[derive(Debug, Serialize)]
pub struct GoogleAdsWebhookResponse {
    /// Success status.
    pub success: bool,
    /// Response message.
    pub message: String,
    /// ID of the lead in Google Ads.
    pub lead_id: Option<String>,
    /// ID of the created lead in C2S.
    pub c2s_lead_id: Option<String>,
}

/// Google Ads webhook handler.
///
/// Flow:
/// 1. Validate google_key (mandatory).
/// 2. Check deduplication (google_ads_leads.google_lead_id unique constraint).
/// 3. Extract contact info (name, phone, email).
/// 4. Validate and normalize phone/email.
/// 5. Inline enrichment: Diretrix → Work API (if possible).
/// 6. Format complete description (Google Ads context + enrichment).
/// 7. Create lead in C2S via gateway (single API call).
/// 8. Store tracking record in database.
///
/// Fallback: If enrichment fails, still create lead with warning.
///
/// # Arguments
///
/// * `app_state` - The application state.
/// * `query` - Query parameters containing the verification key.
/// * `payload` - JSON body containing the Google Ads webhook payload.
///
/// # Returns
///
/// * `Result<impl IntoResponse, AppError>` - The response or an error.
pub async fn google_ads_webhook_handler(
    State(app_state): State<std::sync::Arc<crate::handlers::AppState>>,
    Query(query): Query<GoogleAdsWebhookQuery>,
    Json(payload): Json<GoogleAdsWebhookPayload>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        "📨 Received Google Ads webhook: lead_id={}, campaign={}",
        payload.lead_id,
        payload.campaign_id
    );

    // Step 1: Validate google_key (MANDATORY) - check auth BEFORE any other validation
    let google_key = query
        .google_key
        .as_deref()
        .ok_or_else(|| AppError::Unauthorized("Missing google_key parameter".to_string()))?;
    validate_google_key(&app_state.config, google_key)?;

    // Step 2: Check for duplicate (idempotency via unique constraint)
    if is_duplicate_lead(&app_state.db, &payload.lead_id).await? {
        tracing::warn!("⚠️  Duplicate Google Ads lead: {}", payload.lead_id);
        return Ok((
            StatusCode::OK,
            Json(GoogleAdsWebhookResponse {
                success: true,
                message: "Lead already processed (duplicate)".to_string(),
                lead_id: Some(payload.lead_id.clone()),
                c2s_lead_id: None,
            }),
        ));
    }

    // Step 3: Extract contact info
    let customer_name = payload
        .get_name()
        .ok_or_else(|| AppError::BadRequest("Missing customer name in form data".to_string()))?;

    let email = payload.get_email();
    let phone_raw = payload.get_phone();
    let cpf_from_form = payload.get_cpf();

    // Step 4: Validate and normalize
    let email_validated = email.as_ref().and_then(|e| {
        if is_valid_email(e) {
            Some(e.to_lowercase())
        } else {
            tracing::warn!("❌ Invalid email in Google Ads lead: {}", e);
            None
        }
    });

    let phone_validated = phone_raw.as_ref().and_then(|p| {
        let (valid, normalized) = validate_br_phone(p);
        if valid {
            Some(normalized)
        } else {
            tracing::warn!("❌ Invalid phone in Google Ads lead: {}", p);
            None
        }
    });

    // Step 5: Inline enrichment (Diretrix → Work API)
    let enrichment_result = perform_inline_enrichment(
        &app_state,
        cpf_from_form.as_deref(),
        phone_validated.as_deref(),
        email_validated.as_deref(),
    )
    .await;

    // Step 6: Format complete description
    let enrichment_text = match &enrichment_result {
        Ok(text) => Some(text.as_str()),
        Err(e) => {
            tracing::warn!("⚠️  Enrichment failed: {}", e);
            None
        }
    };

    let description = payload.format_description(enrichment_text);

    // Truncate description if needed (UTF-8 safe)
    let max_desc_len = app_state.config.c2s_description_max_length;
    let description_final = if description.chars().count() > max_desc_len {
        let truncated: String = description.chars().take(max_desc_len).collect();
        tracing::warn!(
            "⚠️  Description truncated from {} to {} chars",
            description.chars().count(),
            truncated.chars().count()
        );
        truncated
    } else {
        description.clone()
    };

    // Step 7: Create lead in C2S directly (using JSON:API format)
    let start = std::time::Instant::now();

    let c2s_service = services::C2SService::new(&app_state.config);

    let c2s_lead_id = c2s_service
        .create_lead(
            &customer_name,
            phone_validated.as_deref(),
            email_validated.as_deref(),
            &description_final,
            Some("Google Ads"),
            app_state.config.c2s_default_seller_id.as_deref(),
        )
        .await?;

    let latency_ms = start.elapsed().as_millis() as i32;

    tracing::info!("✅ Lead created in C2S: {} ({}ms)", c2s_lead_id, latency_ms);

    // Step 8: Store tracking record
    store_google_ads_lead(
        &app_state.db,
        &payload,
        &c2s_lead_id,
        enrichment_result.is_ok(),
        description_final.len() as i32,
        latency_ms,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(GoogleAdsWebhookResponse {
            success: true,
            message: "Lead created and enriched successfully".to_string(),
            lead_id: Some(payload.lead_id.clone()),
            c2s_lead_id: Some(c2s_lead_id.clone()),
        }),
    ))
}

/// Validates the Google webhook verification key.
fn validate_google_key(config: &Config, provided_key: &str) -> Result<(), AppError> {
    let expected_key = config.google_ads_webhook_key.as_ref().ok_or_else(|| {
        AppError::InternalError("GOOGLE_ADS_WEBHOOK_KEY not configured (required)".to_string())
    })?;

    if provided_key != expected_key {
        tracing::error!("❌ Invalid Google Ads webhook key");
        return Err(AppError::Unauthorized(
            "Invalid google_key parameter".to_string(),
        ));
    }

    tracing::debug!("✓ Google webhook key validated");
    Ok(())
}

/// Checks if the lead has already been processed (deduplication).
async fn is_duplicate_lead(db: &PgPool, google_lead_id: &str) -> Result<bool, AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM google_ads_leads WHERE google_lead_id = $1)",
    )
    .bind(google_lead_id)
    .fetch_one(db)
    .await?;

    Ok(exists)
}

/// Performs inline enrichment: Diretrix → Work API.
async fn perform_inline_enrichment(
    state: &std::sync::Arc<crate::handlers::AppState>,
    cpf_from_form: Option<&str>,
    phone: Option<&str>,
    email: Option<&str>,
) -> Result<String, AppError> {
    let mut enrichment = String::new();

    // Try to get CPF (priority: form > Diretrix lookup)
    let cpf = if let Some(cpf) = cpf_from_form {
        enrichment.push_str(&format!("📄 CPF do Formulário: {}\n", cpf));
        Some(cpf.to_string())
    } else {
        // Try Diretrix lookup by phone/email (using optimized lookup)
        // First check cache/DB
        if let Ok(Some(existing)) =
            crate::enrichment::find_existing_enrichment(state, phone, email).await
        {
            tracing::info!("✅ CACHE/DB HIT: Found CPF {} for contact", existing.cpf);
            Some(existing.cpf)
        } else {
            // Fallback to Diretrix
            let lookup_result =
                crate::enrichment::find_cpf_via_diretrix(phone, email, &state.config).await;

            match lookup_result {
                Ok(result) if !result.cpfs.is_empty() => {
                    let cpf_found = result.cpfs[0].clone();
                    enrichment.push_str(&format!("🔍 CPF Encontrado: {}\n", cpf_found));
                    Some(cpf_found)
                }
                Ok(_) => {
                    tracing::info!("📞 Diretrix: CPF not found for phone/email");
                    None
                }
                Err(e) => {
                    tracing::warn!("⚠️  Diretrix lookup failed: {}", e);
                    None
                }
            }
        }
    };

    // If we have CPF, enrich with Work API
    if let Some(cpf_val) = cpf {
        enrichment.push_str("\n💰 Dados Econômicos:\n");

        let work_api = WorkApiService::new(&state.config);
        match work_api.fetch_all_modules(&cpf_val).await {
            Ok(work_data) => {
                // Extract key enrichment data from JSON
                if let Some(basic) = work_data.get("DadosBasicos") {
                    if let Some(nome) = basic.get("nome").and_then(|v| v.as_str()) {
                        enrichment.push_str(&format!("   • Nome Completo: {}\n", nome));
                    }
                    if let Some(idade) = basic.get("idade").and_then(|v| v.as_i64()) {
                        enrichment.push_str(&format!("   • Idade: {} anos\n", idade));
                    }
                }

                if let Some(econ) = work_data.get("DadosEconomicos") {
                    if let Some(renda) = econ.get("renda").and_then(|v| v.as_str()) {
                        enrichment.push_str(&format!("   • Renda Estimada: {}\n", renda));
                    }
                    if let Some(score) = econ.get("score") {
                        if let Some(nota) = score.get("nota").and_then(|v| v.as_str()) {
                            enrichment.push_str(&format!("   • Score: {}\n", nota));
                        }
                    }
                }

                // Addresses
                if let Some(enderecos) = work_data.get("enderecos").and_then(|v| v.as_array()) {
                    if !enderecos.is_empty() {
                        enrichment.push_str("\n🏠 Endereços:\n");
                        for (i, addr) in enderecos.iter().take(2).enumerate() {
                            enrichment.push_str(&format!("   {}. ", i + 1));
                            if let Some(log) = addr.get("logradouro").and_then(|v| v.as_str()) {
                                enrichment.push_str(log);
                            }
                            if let Some(num) = addr.get("numero").and_then(|v| v.as_str()) {
                                enrichment.push_str(&format!(", {}", num));
                            }
                            if let Some(bairro) = addr.get("bairro").and_then(|v| v.as_str()) {
                                enrichment.push_str(&format!(" - {}", bairro));
                            }
                            if let Some(cidade) = addr.get("cidade").and_then(|v| v.as_str()) {
                                enrichment.push_str(&format!(", {}", cidade));
                            }
                            if let Some(uf) = addr.get("uf").and_then(|v| v.as_str()) {
                                enrichment.push_str(&format!("/{}", uf));
                            }
                            if let Some(cep) = addr.get("cep").and_then(|v| v.as_str()) {
                                enrichment.push_str(&format!(" (CEP: {})", cep));
                            }
                            enrichment.push_str("\n");
                        }
                    }
                }

                // Additional phones
                if let Some(telefones) = work_data.get("telefones").and_then(|v| v.as_array()) {
                    if !telefones.is_empty() {
                        enrichment
                            .push_str(&format!("\n📱 Telefones Adicionais: {}\n", telefones.len()));
                    }
                }

                // Additional emails
                if let Some(emails) = work_data.get("emails").and_then(|v| v.as_array()) {
                    if !emails.is_empty() {
                        enrichment.push_str(&format!("📧 E-mails Adicionais: {}\n", emails.len()));
                    }
                }

                tracing::info!("✅ Work API enrichment successful");
            }
            Err(e) => {
                tracing::warn!("⚠️  Work API enrichment failed: {}", e);
                enrichment.push_str(&format!("\n⚠️  Enriquecimento parcial (erro: {})\n", e));
            }
        }
    } else {
        enrichment.push_str("\n⚠️  CPF não disponível - Enriquecimento limitado\n");
    }

    if enrichment.is_empty() {
        Err(AppError::InternalError(
            "No enrichment data available".to_string(),
        ))
    } else {
        Ok(enrichment)
    }
}

/// Stores the Google Ads lead tracking record in the database.
async fn store_google_ads_lead(
    db: &PgPool,
    payload: &GoogleAdsWebhookPayload,
    c2s_lead_id: &str,
    enrichment_success: bool,
    description_length: i32,
    c2s_latency_ms: i32,
) -> Result<(), AppError> {
    let cpf = payload.get_cpf();
    let enrichment_status = if enrichment_success {
        "completed"
    } else {
        "partial"
    };

    sqlx::query(
        r#"
        INSERT INTO google_ads_leads (
            google_lead_id,
            c2s_lead_id,
            form_id,
            campaign_id,
            gcl_id,
            payload_raw,
            enrichment_status,
            cpf,
            description_length,
            c2s_latency_ms,
            c2s_created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(&payload.lead_id)
    .bind(c2s_lead_id)
    .bind(payload.form_id)
    .bind(payload.campaign_id)
    .bind(&payload.gcl_id)
    .bind(serde_json::to_value(payload).unwrap())
    .bind(enrichment_status)
    .bind(cpf)
    .bind(description_length)
    .bind(c2s_latency_ms)
    .bind(Utc::now())
    .execute(db)
    .await?;

    tracing::info!("✓ Google Ads lead tracking record stored");
    Ok(())
}
