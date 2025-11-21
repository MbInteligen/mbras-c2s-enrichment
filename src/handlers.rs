use crate::config::Config;
use crate::errors::AppError;
use crate::gateway_client::C2sGatewayClient;
use crate::models::*;
use crate::services::{DiretrixService, EnrichmentService, WorkApiService};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use moka::future::Cache;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    pub gateway_client: Option<C2sGatewayClient>, // Optional gateway client
    /// Global deduplication cache to prevent processing same CPF within short time window
    pub recent_cpf_cache: Cache<String, i64>,
    /// Lead-level deduplication cache to prevent concurrent processing of same lead_id
    pub processing_leads_cache: Cache<String, i64>,
}

/// Health check endpoint
pub async fn health() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "service": "rust-c2s-api",
            "version": "0.1.0"
        })),
    )
}

/// GET /api/v1/contributor/customer
/// Main endpoint that mimics ibvi-api's /contributor/customer
/// This is what mbras-c2s will call
pub async fn get_customer(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CustomerQueryParams>,
) -> Result<Json<UnifiedCustomerResponse>, AppError> {
    tracing::info!("GET /contributor/customer - params: {:?}", params);

    // Validate at least one identifier is provided
    if params.cpf.is_none()
        && params.email.is_none()
        && params.phone.is_none()
        && params.name.is_none()
    {
        return Err(AppError::BadRequest(
            "At least one identifier required (cpf, email, phone, or name)".to_string(),
        ));
    }

    let enrichment_service = EnrichmentService::new(&state.config, state.db.clone());
    let customer_data = enrichment_service.get_customer_unified(&params).await?;

    tracing::info!(
        "Successfully retrieved customer data. Enriched: {}, Sources: {:?}",
        customer_data.metadata.enriched,
        customer_data.metadata.sources
    );

    Ok(Json(customer_data))
}

/// GET /api/v1/customers/:id
/// Get customer by UUID
pub async fn get_customer_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<EnrichedCustomerData>, AppError> {
    tracing::info!("GET /customers/{}", id);

    let customer = sqlx::query_as::<_, Customer>(
        "SELECT * FROM core.parties WHERE id = $1 AND party_type = 'customer'",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Customer with id {} not found", id)))?;

    let emails = sqlx::query_as::<_, Email>(
        "SELECT e.* FROM app.emails e
         INNER JOIN core.party_emails pe ON e.id = pe.email_id
         WHERE pe.party_id = $1",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let phones = sqlx::query_as::<_, Phone>(
        "SELECT ph.* FROM app.phones ph
         INNER JOIN core.party_phones pp ON ph.id = pp.phone_id
         WHERE pp.party_id = $1",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(EnrichedCustomerData {
        customer,
        emails,
        phones,
        enrichment_data: None,
    }))
}

/// POST /api/v1/enrich
/// Enrich customer data via Work API
pub async fn enrich_customer(
    State(state): State<Arc<AppState>>,
    Json(params): Json<CustomerQueryParams>,
) -> Result<Json<UnifiedCustomerResponse>, AppError> {
    tracing::info!("POST /enrich - params: {:?}", params);

    let enrichment_service = EnrichmentService::new(&state.config, state.db.clone());
    let customer_data = enrichment_service.get_customer_unified(&params).await?;

    Ok(Json(customer_data))
}

/// GET /api/v1/work/modules/all
/// Fetch all Work API modules for a given document
pub async fn fetch_all_modules(
    State(state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> Result<Json<crate::models::WorkApiCompleteResponse>, AppError> {
    let documento = params
        .get("documento")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing 'documento' parameter".to_string()))?;

    tracing::info!("Fetching all Work API modules for: {}", documento);

    let work_api = crate::services::WorkApiService::new(&state.config);
    let result = work_api.fetch_all_modules(documento).await?;

    Ok(Json(result))
}

/// GET /api/v1/work/modules/{module}
/// Fetch specific Work API module
pub async fn fetch_module(
    State(state): State<Arc<AppState>>,
    Path(module): Path<String>,
    Query(params): Query<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let documento = params
        .get("documento")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing 'documento' parameter".to_string()))?;

    tracing::info!("Fetching Work API module '{}' for: {}", module, documento);

    let work_api = crate::services::WorkApiService::new(&state.config);
    let result = work_api.fetch_module(&module, documento).await?;

    Ok(Json(
        result.unwrap_or(serde_json::json!({"error": "No data"})),
    ))
}

/// POST /api/v1/leads
/// Process lead (similar to mbras-c2s ProcessLead flow)
pub async fn process_lead(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LeadRequest>,
) -> Result<Json<LeadResponse>, AppError> {
    tracing::info!("POST /leads - lead_id: {}", payload.lead_id);

    // Build query params from lead data
    let params = CustomerQueryParams {
        cpf: payload.personal_info.cpf.clone(),
        email: payload.personal_info.email.clone(),
        phone: payload.contact_info.phones.first().map(|p| p.phone.clone()),
        name: Some(payload.personal_info.name.clone()),
    };

    let enrichment_service = EnrichmentService::new(&state.config, state.db.clone());

    match enrichment_service.get_customer_unified(&params).await {
        Ok(customer_data) => {
            // Check if we have useful contact data
            let has_data = !customer_data.contact_info.emails.is_empty()
                || !customer_data.contact_info.phones.is_empty();

            if has_data {
                tracing::info!(
                    "Lead {} processed successfully with enriched data",
                    payload.lead_id
                );
                Ok(Json(LeadResponse {
                    success: true,
                    message: "Lead processed and enriched successfully".to_string(),
                    data: Some(EnrichedCustomerData {
                        customer: Customer {
                            id: Uuid::new_v4(),
                            party_type: "customer".to_string(),
                            cpf_cnpj: customer_data.personal_info.cpf.unwrap_or_default(),
                            full_name: customer_data
                                .personal_info
                                .name
                                .unwrap_or_else(|| payload.personal_info.name.clone()),
                            normalized_name: None,
                            sex: customer_data.personal_info.gender,
                            birth_date: customer_data
                                .personal_info
                                .birth_date
                                .and_then(|d| d.parse().ok()),
                            mother_name: customer_data.personal_info.mother_name,
                            father_name: customer_data.personal_info.father_name,
                            rg: customer_data.personal_info.rg,
                            fantasy_name: None,
                            normalized_fantasy_name: None,
                            opening_date: None,
                            registration_status_date: None,
                            company_type: None,
                            company_size: None,
                            enriched: Some(customer_data.metadata.enriched),
                            created_at: chrono::Utc::now(),
                            updated_at: None,
                        },
                        emails: customer_data
                            .contact_info
                            .emails
                            .iter()
                            .map(|e| Email {
                                id: Uuid::new_v4(),
                                email: e.email.clone(),
                                created_at: chrono::Utc::now(),
                            })
                            .collect(),
                        phones: customer_data
                            .contact_info
                            .phones
                            .iter()
                            .map(|p| Phone {
                                id: Uuid::new_v4(),
                                number: p.phone.clone(),
                                country_code: None,
                                created_at: chrono::Utc::now(),
                            })
                            .collect(),
                        enrichment_data: None,
                    }),
                }))
            } else {
                tracing::warn!(
                    "Lead {} processed but no contact data found",
                    payload.lead_id
                );
                Ok(Json(LeadResponse {
                    success: false,
                    message: "No contact data found for lead".to_string(),
                    data: None,
                }))
            }
        }
        Err(e) => {
            tracing::error!("Failed to process lead {}: {:?}", payload.lead_id, e);
            Ok(Json(LeadResponse {
                success: false,
                message: format!("Failed to enrich lead: {}", e),
                data: None,
            }))
        }
    }
}

/// POST /api/v1/c2s/enrich/:lead_id
/// Complete C2S integration flow:
/// 1. Fetch lead from C2S
/// 2. Enrich with Work API
/// 3. Send enriched data back to C2S
pub async fn c2s_enrich_lead(
    State(state): State<Arc<AppState>>,
    Path(lead_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    tracing::info!("C2S Enrich Lead: {}", lead_id);

    // Initialize services
    let diretrix_service = DiretrixService::new(&state.config);
    let work_api_service = WorkApiService::new(&state.config);

    // Step 1: Fetch lead from C2S
    tracing::info!("Step 1: Fetching lead from C2S");
    
    let gateway = state.gateway_client.as_ref().ok_or_else(|| {
        AppError::InternalError("C2S Client not initialized".to_string())
    })?;

    let response = gateway.get_lead(&lead_id).await?;
    let lead_data: crate::services::C2SLeadResponse = serde_json::from_value(response).map_err(|e| {
        AppError::ExternalApiError(format!("Failed to parse C2S response: {}", e))
    })?;

    let customer = &lead_data.data.attributes.customer;
    tracing::info!(
        "Lead fetched - Customer: {} (CPF/Phone: {})",
        customer.name,
        customer.phone
    );

    // Step 2: Use Diretrix to find CPF from phone/email
    tracing::info!("Step 2: Using Diretrix to find CPF");
    let _phone_opt = if !customer.phone.is_empty() {
        Some(customer.phone.as_str())
    } else {
        None
    };
    let _email_opt = if !customer.email.is_empty() {
        Some(customer.email.as_str())
    } else {
        None
    };

    // Parallel lookup - search by phone AND email separately
    let phone_lookup = if !customer.phone.is_empty() {
        diretrix_service.search_by_phone(&customer.phone).await.ok()
    } else {
        None
    };

    let email_lookup = if !customer.email.is_empty() {
        diretrix_service.search_by_email(&customer.email).await.ok()
    } else {
        None
    };

    // Extract CPFs from both lookups
    let phone_cpf = phone_lookup.as_ref().and_then(|results| {
        if !results.is_empty() {
            Some(results[0].cpf.clone())
        } else {
            None
        }
    });

    let email_cpf = email_lookup.as_ref().and_then(|results| {
        if !results.is_empty() {
            Some(results[0].cpf.clone())
        } else {
            None
        }
    });

    // Check if both found and if they're the same person
    let (cpf_list, same_person) = match (&phone_cpf, &email_cpf) {
        (Some(p_cpf), Some(e_cpf)) if p_cpf == e_cpf => {
            tracing::info!(
                "✓ Phone and email belong to the same person (CPF: {})",
                p_cpf
            );
            (vec![p_cpf.clone()], true)
        }
        (Some(p_cpf), Some(e_cpf)) => {
            tracing::warn!(
                "⚠ Phone and email belong to DIFFERENT people! Phone CPF: {}, Email CPF: {}",
                p_cpf,
                e_cpf
            );
            (vec![p_cpf.clone(), e_cpf.clone()], false)
        }
        (Some(cpf), None) | (None, Some(cpf)) => {
            tracing::info!("Found CPF from single source: {}", cpf);
            (vec![cpf.clone()], false)
        }
        (None, None) => {
            tracing::error!("Could not find CPF from either phone or email");
            return Err(AppError::NotFound(
                "Could not find CPF via Diretrix".to_string(),
            ));
        }
    };

    // Step 3: Enrich all CPFs with Work API
    tracing::info!(
        "Step 3: Enriching {} person(s) with Work API",
        cpf_list.len()
    );

    let mut enriched_data = Vec::new();
    for cpf in &cpf_list {
        tracing::info!("Enriching CPF: {}", cpf);
        match work_api_service.fetch_all_modules(cpf).await {
            Ok(data) => enriched_data.push(data),
            Err(e) => tracing::warn!("Failed to enrich CPF {}: {}", cpf, e),
        }
    }

    if enriched_data.is_empty() {
        return Err(AppError::ExternalApiError(
            "No enrichment data available".to_string(),
        ));
    }

    // Step 4: Format enriched data as message body
    tracing::info!(
        "Step 4: Formatting enriched data (same_person: {})",
        same_person
    );
    let message_body = if same_person {
        let enriched_msg = format_enriched_message(&customer.name, &enriched_data[0]);
        tracing::info!("Enriched message length: {} chars", enriched_msg.len());
        format!("📞📧 Telefone e e-mail da mesma pessoa\n\n{}", enriched_msg)
    } else {
        let mut combined_message =
            String::from("⚠️ Telefone e e-mail relacionados a PESSOAS DIFERENTES!\n\n");

        combined_message.push_str(&format!(
            "═══ PESSOA 1 (Telefone: {}) ═══\n",
            customer.phone
        ));
        combined_message.push_str(&format_enriched_message("", &enriched_data[0]));

        if enriched_data.len() > 1 {
            combined_message.push_str(&format!(
                "\n\n═══ PESSOA 2 (Email: {}) ═══\n",
                customer.email
            ));
            combined_message.push_str(&format_enriched_message("", &enriched_data[1]));
        }

        combined_message
    };

    tracing::info!(
        "Step 4: Sending enriched data back to C2S (message length: {} chars)",
        message_body.len()
    );

    // Step 5: Send back to C2S
    let gateway = state.gateway_client.as_ref().ok_or_else(|| {
        AppError::InternalError("C2S Client not initialized".to_string())
    })?;

    tracing::info!("Using C2S Client to send message");
    gateway.send_message(&lead_id, &message_body).await?;

    // Step 6: Store enriched data in database
    tracing::info!("Step 5: Storing {} person(s) in database", cpf_list.len());
    let storage = crate::db_storage::EnrichmentStorage::new(state.db.clone());

    let mut stored_entity_ids = Vec::new();
    for (idx, cpf) in cpf_list.iter().enumerate() {
        match storage
            .store_enriched_person_with_lead(cpf, &enriched_data[idx], Some(&lead_id))
            .await
        {
            Ok(entity_id) => {
                tracing::info!(
                    "✓ Stored CPF {} → entity_id: {} (lead_id: {})",
                    cpf,
                    entity_id,
                    lead_id
                );
                stored_entity_ids.push(entity_id);
            }
            Err(e) => {
                tracing::error!("✗ Failed to store CPF {}: {}", cpf, e);
                // Don't fail the whole request, just log the error
            }
        }
    }

    Ok(Json(json!({
        "success": true,
        "lead_id": lead_id,
        "customer_name": customer.name,
        "enriched": true,
        "message_sent": true,
        "stored_in_db": stored_entity_ids.len(),
        "entity_ids": stored_entity_ids
    })))
}

/// Helper function to multiply currency values in a range string
/// Example: "De R$ 1630 até R$ 4082" -> "De R$ 3097.00 até R$ 7755.80"
fn multiply_range_values(range_str: &str, multiplier: f64) -> String {
    use regex::Regex;

    // Pattern to match currency values in the format "R$ 1630" or "R$ 4082"
    let re = Regex::new(r"R\$\s*(\d+)").unwrap();

    let result = re.replace_all(range_str, |caps: &regex::Captures| {
        if let Some(num_str) = caps.get(1) {
            if let Ok(value) = num_str.as_str().parse::<f64>() {
                let adjusted = value * multiplier;
                return format!("R$ {:.2}", adjusted);
            }
        }
        caps[0].to_string()
    });

    result.to_string()
}

/// Format enriched Work API data into a readable message for C2S
pub fn format_enriched_message(customer_name: &str, work_data: &WorkApiCompleteResponse) -> String {
    tracing::info!("Formatting message for: {}", customer_name);
    tracing::info!(
        "Work data has keys: {:?}",
        work_data.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );

    let mut message = String::new();

    // Work API returns data directly at root level (not wrapped in modules)
    message.push_str("✅ DADOS PESSOAIS\n");

    if let Some(dados_basicos) = work_data.get("DadosBasicos") {
        tracing::info!("Found DadosBasicos");
        if let Some(nome) = dados_basicos.get("nome").and_then(|v| v.as_str()) {
            message.push_str(&format!("Nome: {}\n", nome));
        }
        if let Some(cpf) = dados_basicos.get("cpf").and_then(|v| v.as_str()) {
            message.push_str(&format!("CPF: {}\n", cpf));
        }
        if let Some(data_nasc) = dados_basicos.get("dataNascimento").and_then(|v| v.as_str()) {
            message.push_str(&format!("Data Nascimento: {}\n", data_nasc));
        }
        if let Some(sexo) = dados_basicos.get("sexo").and_then(|v| v.as_str()) {
            message.push_str(&format!("Sexo: {}\n", sexo));
        }
        if let Some(mae) = dados_basicos.get("nomeMae").and_then(|v| v.as_str()) {
            message.push_str(&format!("Mãe: {}\n", mae));
        }
    }

    // Financial data
    if let Some(dados_econ) = work_data.get("DadosEconomicos") {
        message.push_str("\n💰 DADOS FINANCEIROS\n");

        if let Some(renda_str) = dados_econ.get("renda").and_then(|v| v.as_str()) {
            // Multiply renda by 1.9
            if let Ok(renda_val) = renda_str.replace(",", ".").parse::<f64>() {
                let renda_adjusted = renda_val * 1.9;
                message.push_str(&format!("Renda: R$ {:.2}\n", renda_adjusted));
            } else {
                message.push_str(&format!("Renda: R$ {}\n", renda_str));
            }
        }

        if let Some(poder_aq) = dados_econ.get("poderAquisitivo") {
            if let Some(desc) = poder_aq
                .get("poderAquisitivoDescricao")
                .and_then(|v| v.as_str())
            {
                message.push_str(&format!("Poder Aquisitivo: {}\n", desc));
            }
            if let Some(faixa) = poder_aq
                .get("faixaPoderAquisitivo")
                .and_then(|v| v.as_str())
            {
                // Parse and multiply the range values by 1.9
                // Format: "De R$ 1630 até R$ 4082"
                let faixa_adjusted = multiply_range_values(faixa, 1.9);
                message.push_str(&format!("Faixa de Renda: {}\n", faixa_adjusted));
            }
        }

        if let Some(score) = dados_econ.get("score") {
            if let Some(score_val) = score.get("scoreCSBA").and_then(|v| v.as_str()) {
                message.push_str(&format!("Score de Crédito: {}\n", score_val));
            }
            if let Some(risco) = score.get("scoreCSBAFaixaRisco").and_then(|v| v.as_str()) {
                message.push_str(&format!("Risco: {}\n", risco));
            }
        }
    }

    // Contact info
    if let Some(emails) = work_data.get("emails").and_then(|v| v.as_array()) {
        if !emails.is_empty() {
            message.push_str("\n📧 EMAILS\n");
            for (i, email) in emails.iter().take(3).enumerate() {
                if let Some(email_str) = email.get("email").and_then(|v| v.as_str()) {
                    let prioridade = email
                        .get("prioridade")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    message.push_str(&format!("{}. {} ({})\n", i + 1, email_str, prioridade));
                }
            }
        }
    }

    if let Some(telefones) = work_data.get("telefones").and_then(|v| v.as_array()) {
        if !telefones.is_empty() {
            message.push_str("\n📱 TELEFONES\n");
            for (i, telefone) in telefones.iter().take(3).enumerate() {
                if let Some(tel) = telefone.get("telefone").and_then(|v| v.as_str()) {
                    let tipo = telefone
                        .get("tipo")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    let whats = telefone
                        .get("whatsapp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("NAO");
                    let whats_icon = if whats == "SIM" { "✅" } else { "" };
                    message.push_str(&format!("{}. {} - {} {}\n", i + 1, tel, tipo, whats_icon));
                }
            }
        }
    }

    // Addresses
    if let Some(enderecos) = work_data.get("enderecos").and_then(|v| v.as_array()) {
        if !enderecos.is_empty() {
            message.push_str("\n🏠 ENDEREÇOS\n");
            for (i, endereco) in enderecos.iter().take(2).enumerate() {
                let logradouro = endereco
                    .get("logradouro")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let numero = endereco
                    .get("logradouroNumero")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let bairro = endereco
                    .get("bairro")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let cidade = endereco
                    .get("cidade")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let uf = endereco.get("uf").and_then(|v| v.as_str()).unwrap_or("");
                let cep = endereco.get("cep").and_then(|v| v.as_str()).unwrap_or("");

                message.push_str(&format!(
                    "{}. {} {}, {} - {}/{} - CEP: {}\n",
                    i + 1,
                    logradouro,
                    numero,
                    bairro,
                    cidade,
                    uf,
                    cep
                ));
            }
        }
    }

    // Companies
    if let Some(empresas) = work_data.get("empresas").and_then(|v| v.as_array()) {
        if !empresas.is_empty() {
            message.push_str("\n🏢 EMPRESAS\n");
            for (i, empresa) in empresas.iter().take(3).enumerate() {
                let cnpj = empresa.get("cnpj").and_then(|v| v.as_str()).unwrap_or("");
                let relacao = empresa
                    .get("relacao")
                    .and_then(|v| v.as_str())
                    .unwrap_or("SOCIO");
                message.push_str(&format!("{}. CNPJ: {} - {}\n", i + 1, cnpj, relacao));
            }
        }
    }

    message
}

/// GET /api/v1/leads/process?id={lead_id}
/// Simple trigger endpoint for Make.com integration
/// Accepts lead ID, fetches from C2S, and processes using existing enrichment flow
pub async fn trigger_lead_processing(
    State(state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Extract lead ID from query params
    let lead_id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing 'id' parameter".to_string()))?;

    tracing::info!("=== Trigger Lead Processing: {} ===", lead_id);

    // ATOMIC DEDUPLICATION: Check if this lead is already being processed
    // This prevents concurrent requests from processing the same lead multiple times
    // NOTE: This uses in-memory cache which works for single instance deployments
    // For multi-instance production, replace with Redis: SET lead:{id} NX EX 300
    let now = chrono::Utc::now().timestamp();

    if let Some(processing_since) = state.processing_leads_cache.get(lead_id).await {
        let seconds_ago = now - processing_since;
        tracing::warn!(
            "⏭ DUPLICATE REQUEST BLOCKED - Lead {} already being processed ({} seconds ago)",
            lead_id,
            seconds_ago
        );
        return Ok(Json(json!({
            "success": true,
            "message": format!("Lead already being processed (started {} seconds ago). Duplicate request blocked.", seconds_ago),
            "lead_id": lead_id,
            "duplicate_request": true
        })));
    }

    // Mark lead as being processed IMMEDIATELY (first request wins in most cases)
    state
        .processing_leads_cache
        .insert(lead_id.to_string(), now)
        .await;
    tracing::info!("✓ Lead {} marked as processing at {}", lead_id, now);

    // Small delay to allow cache propagation and catch racing requests
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Double-check: if timestamp changed, another request won the race
    if let Some(cached_time) = state.processing_leads_cache.get(lead_id).await {
        if cached_time != now {
            tracing::warn!(
                "⏭ RACE CONDITION DETECTED - Another request won for lead {}. Backing off.",
                lead_id
            );
            return Ok(Json(json!({
                "success": true,
                "message": "Another concurrent request is processing this lead. Request deduplicated.",
                "lead_id": lead_id,
                "duplicate_request": true
            })));
        }
    }

    // Fetch lead from C2S
    tracing::info!("Step 1: Fetching lead from C2S");
    
    let gateway = state.gateway_client.as_ref().ok_or_else(|| {
        AppError::InternalError("C2S Client not initialized".to_string())
    })?;

    let lead_data: crate::services::C2SLeadResponse = match gateway.get_lead(lead_id).await {
        Ok(response) => match serde_json::from_value(response) {
            Ok(data) => {
                tracing::info!("✓ Successfully fetched lead from C2S");
                data
            }
            Err(e) => {
                tracing::error!("✗ Failed to parse C2S response: {}", e);
                return Ok(Json(json!({
                    "success": false,
                    "message": format!("Failed to parse C2S response: {}", e),
                    "lead_id": lead_id
                })));
            }
        },
        Err(e) => {
            tracing::error!("✗ Failed to fetch lead from C2S: {}", e);
            return Ok(Json(json!({
                "success": false,
                "message": format!("Failed to fetch lead from C2S: {}", e),
                "lead_id": lead_id
            })));
        }
    };

    let customer = &lead_data.data.attributes.customer;
    tracing::info!(
        "Lead details - Customer: {}, Phone: {}, Email: {}",
        customer.name,
        customer.phone,
        customer.email
    );

    // Initialize services for enrichment
    let diretrix_service = DiretrixService::new(&state.config);
    let work_api_service = WorkApiService::new(&state.config);
    let storage = crate::db_storage::EnrichmentStorage::new(state.db.clone());

    // Step 2: Use Diretrix to find CPF from phone/email
    tracing::info!("Step 2: Using Diretrix to find CPF");

    // Parallel lookup - search by phone AND email separately
    let phone_lookup = if !customer.phone.is_empty() {
        diretrix_service.search_by_phone(&customer.phone).await.ok()
    } else {
        None
    };

    let email_lookup = if !customer.email.is_empty() {
        diretrix_service.search_by_email(&customer.email).await.ok()
    } else {
        None
    };

    // Extract CPFs from both lookups
    let phone_cpf = phone_lookup.as_ref().and_then(|results| {
        if !results.is_empty() {
            Some(results[0].cpf.clone())
        } else {
            None
        }
    });

    let email_cpf = email_lookup.as_ref().and_then(|results| {
        if !results.is_empty() {
            Some(results[0].cpf.clone())
        } else {
            None
        }
    });

    // Check if both found and if they're the same person
    let (cpf_list, same_person) = match (&phone_cpf, &email_cpf) {
        (Some(p_cpf), Some(e_cpf)) if p_cpf == e_cpf => {
            tracing::info!(
                "✓ Phone and email belong to the same person (CPF: {})",
                p_cpf
            );
            (vec![p_cpf.clone()], true)
        }
        (Some(p_cpf), Some(e_cpf)) => {
            tracing::warn!(
                "⚠ Phone and email belong to DIFFERENT people! Phone CPF: {}, Email CPF: {}",
                p_cpf,
                e_cpf
            );
            (vec![p_cpf.clone(), e_cpf.clone()], false)
        }
        (Some(cpf), None) | (None, Some(cpf)) => {
            tracing::info!("Found CPF from single source: {}", cpf);
            (vec![cpf.clone()], false)
        }
        (None, None) => {
            tracing::error!("Could not find CPF from either phone or email");
            return Ok(Json(json!({
                "success": false,
                "message": "Could not find CPF from phone or email",
                "lead_id": lead_id
            })));
        }
    };

    // Step 3: Enrich each CPF with Work API (with deduplication)
    tracing::info!("Step 3: Enriching {} CPF(s) with Work API", cpf_list.len());
    let mut enriched_data = Vec::new();
    let mut cpfs_to_process = Vec::new();

    // Check cache for recently processed CPFs
    for cpf in &cpf_list {
        if let Some(timestamp) = state.recent_cpf_cache.get(cpf).await {
            let now = chrono::Utc::now().timestamp();
            let seconds_ago = now - timestamp;

            if seconds_ago < 60 {
                tracing::warn!(
                    "⏭ Skipping CPF {} - already processed {} seconds ago (deduplication)",
                    cpf,
                    seconds_ago
                );
                continue;
            }
        }
        cpfs_to_process.push(cpf.clone());
    }

    if cpfs_to_process.is_empty() {
        tracing::info!("All CPFs recently processed, skipping enrichment");
        return Ok(Json(json!({
            "success": true,
            "message": "CPFs already recently processed (deduplication)",
            "lead_id": lead_id,
            "cpfs_processed": cpf_list,
            "entities_stored": 0
        })));
    }

    // Enrich only CPFs that haven't been recently processed
    for cpf in &cpfs_to_process {
        match work_api_service.fetch_all_modules(cpf).await {
            Ok(data) => {
                tracing::info!("✓ Enriched CPF: {}", cpf);
                enriched_data.push(data);
                // Mark as processed immediately after successful enrichment
                let now = chrono::Utc::now().timestamp();
                state.recent_cpf_cache.insert(cpf.clone(), now).await;
            }
            Err(e) => {
                tracing::error!("✗ Failed to enrich CPF {}: {}", cpf, e);
            }
        }
    }

    if enriched_data.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "Failed to enrich any CPFs",
            "lead_id": lead_id
        })));
    }

    // Step 4: Format enriched message
    tracing::info!("Step 4: Formatting enriched data for C2S");
    let mut full_message = String::new();

    // Add phone/email match indicator if both were found
    if same_person && phone_cpf.is_some() && email_cpf.is_some() {
        full_message.push_str("📞📧 Telefone e e-mail da mesma pessoa\n\n");
    }

    // Format enriched data for each person
    for (idx, data) in enriched_data.iter().enumerate() {
        if idx > 0 {
            full_message.push_str("\n---\n\n");
        }
        let formatted = format_enriched_message(&customer.name, data);
        full_message.push_str(&formatted);
    }

    tracing::info!("Formatted message length: {} chars", full_message.len());

    // Step 5: Store enriched data in database
    tracing::info!(
        "Step 5: Storing {} person(s) in database",
        cpfs_to_process.len()
    );
    let mut stored_entity_ids = Vec::new();

    for (idx, cpf) in cpfs_to_process.iter().enumerate() {
        match storage
            .store_enriched_person_with_lead(cpf, &enriched_data[idx], Some(lead_id))
            .await
        {
            Ok(entity_id) => {
                tracing::info!(
                    "✓ Stored CPF {} → entity_id: {} (lead_id: {})",
                    cpf,
                    entity_id,
                    lead_id
                );
                stored_entity_ids.push(entity_id);
            }
            Err(e) => {
                tracing::error!("✗ Failed to store CPF {}: {}", cpf, e);
            }
        }
    }

    // Step 6: Send enriched data back to C2S
    tracing::info!("Step 6: Sending enriched data to C2S");
    
    let gateway = state.gateway_client.as_ref().ok_or_else(|| {
        AppError::InternalError("C2S Client not initialized".to_string())
    })?;

    tracing::info!("Using C2S Client to send message");
    let send_result = gateway.send_message(lead_id, &full_message).await;

    match send_result {
        Ok(_) => {
            tracing::info!(
                "✓ Successfully sent enriched data to C2S for lead: {}",
                lead_id
            );
            Ok(Json(json!({
                "success": true,
                "message": format!("Successfully processed and enriched lead. Stored {} entities in database.", stored_entity_ids.len()),
                "lead_id": lead_id,
                "cpfs_processed": cpf_list,
                "entities_stored": stored_entity_ids.len()
            })))
        }
        Err(e) => {
            tracing::error!("✗ Failed to send message to C2S: {}", e);
            Ok(Json(json!({
                "success": false,
                "message": format!("Enriched data but failed to send to C2S: {}", e),
                "lead_id": lead_id
            })))
        }
    }
}
