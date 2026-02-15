//! Batch enrichment endpoints.
//!
//! Port of ts-c2s-api `src/routes/batch.ts`.
//! Provides direct enrichment, recent lead enrichment, and retry-failed routes.

use crate::config::Config;
use crate::cpf::normalize_cpf;
use crate::discovery::CpfDiscoveryService;
use crate::errors::AppError;
use crate::handlers::AppState;
use crate::retry::{is_retry_eligible, RETRYABLE_STATUSES};
use crate::services::WorkApiService;
use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;

/// POST /batch/enrich-direct request body.
#[derive(Debug, Deserialize)]
pub struct EnrichDirectRequest {
    pub phone: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// POST /batch/enrich-direct response.
#[derive(Debug, Serialize)]
pub struct EnrichDirectResponse {
    pub success: bool,
    pub data: Value,
}

/// POST /batch/retry-failed request body.
#[derive(Debug, Deserialize)]
pub struct RetryFailedRequest {
    pub limit: Option<u32>,
    #[serde(rename = "delayMs")]
    pub delay_ms: Option<u64>,
}

/// POST /batch/enrich-direct
///
/// Direct enrichment without C2S integration. Runs 5-tier CPF discovery,
/// then enriches via Work API. Does NOT write to database.
pub async fn enrich_direct(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EnrichDirectRequest>,
) -> Result<Json<EnrichDirectResponse>, AppError> {
    // Require at least phone or email
    if body.phone.is_none() && body.email.is_none() {
        return Ok(Json(EnrichDirectResponse {
            success: false,
            data: json!({
                "status": "error",
                "message": "At least phone or email is required"
            }),
        }));
    }

    let config = &state.config;
    let discovery = CpfDiscoveryService::new(config);

    // Step 1: CPF Discovery (5-tier phone + 2-tier email)
    let cpf_result = discovery
        .find_cpf(
            body.phone.as_deref(),
            body.email.as_deref(),
            body.name.as_deref(),
        )
        .await;

    let discovery_result = match cpf_result {
        Ok(Some(result)) => result,
        Ok(None) => {
            // Also try legacy tiers (DBase/Mimir) via enrichment module
            match crate::enrichment::find_cpf_via_diretrix(
                body.phone.as_deref(),
                body.email.as_deref(),
                config,
                body.name.as_deref(),
            )
            .await
            {
                Ok(cpf_lookup) if !cpf_lookup.cpfs.is_empty() => {
                    // Convert legacy result to discovery format
                    let cpf = cpf_lookup.cpfs[0].clone();
                    crate::discovery::CpfDiscoveryResult {
                        cpf,
                        found_name: String::new(),
                        name_matches: false,
                        match_score: 0.0,
                        match_method: "legacy-fallback".to_string(),
                        source: "dbase-mimir".to_string(),
                    }
                }
                _ => {
                    return Ok(Json(EnrichDirectResponse {
                        success: true,
                        data: json!({
                            "status": "unenriched",
                            "message": "CPF not found via any discovery tier",
                            "phone": body.phone,
                            "email": body.email,
                            "name": body.name,
                        }),
                    }));
                }
            }
        }
        Err(e) => {
            tracing::warn!("CPF discovery error: {}", e);
            return Ok(Json(EnrichDirectResponse {
                success: true,
                data: json!({
                    "status": "unenriched",
                    "message": format!("CPF discovery failed: {}", e),
                    "phone": body.phone,
                    "email": body.email,
                    "name": body.name,
                }),
            }));
        }
    };

    let cpf = normalize_cpf(&discovery_result.cpf);

    // Step 2: Enrich via Work API (CPF module)
    let work_api = WorkApiService::new(config);
    match work_api.fetch_all_modules(&cpf).await {
        Ok(enriched_data) => {
            // Extract key fields from enriched data
            let dados = enriched_data.get("DadosBasicos").cloned().unwrap_or(json!({}));

            Ok(Json(EnrichDirectResponse {
                success: true,
                data: json!({
                    "status": "completed",
                    "cpf": cpf,
                    "cpfSource": discovery_result.source,
                    "foundName": discovery_result.found_name,
                    "matchScore": discovery_result.match_score,
                    "nameMatches": discovery_result.name_matches,
                    "enrichedName": dados.get("nome").and_then(|v| v.as_str()).unwrap_or(""),
                    "birthDate": dados.get("dataNascimento").and_then(|v| v.as_str()).unwrap_or(""),
                    "gender": dados.get("sexo").and_then(|v| v.as_str()).unwrap_or(""),
                    "motherName": dados.get("nomeMae").and_then(|v| v.as_str()).unwrap_or(""),
                }),
            }))
        }
        Err(e) => {
            tracing::warn!("Work API enrichment failed for CPF {}: {}", cpf, e);
            Ok(Json(EnrichDirectResponse {
                success: true,
                data: json!({
                    "status": "partial",
                    "message": "CPF found but Work API enrichment failed",
                    "cpf": cpf,
                    "cpfSource": discovery_result.source,
                    "foundName": discovery_result.found_name,
                    "matchScore": discovery_result.match_score,
                }),
            }))
        }
    }
}

/// POST /batch/retry-failed
///
/// Retry enrichment for leads with retryable status (partial/unenriched/basic).
/// Uses exponential backoff. Queries analytics.c2s_leads.
pub async fn retry_failed(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RetryFailedRequest>,
) -> Result<Json<Value>, AppError> {
    let limit = body.limit.unwrap_or(25).min(50) as i64;
    let delay_ms = body.delay_ms.unwrap_or(500).max(100).min(5000);
    let db = &state.db;
    let config = &state.config;

    let start = std::time::Instant::now();

    // Fetch retryable leads from analytics.c2s_leads
    let leads = fetch_retryable_leads(db, limit).await?;
    let total_failed = leads.len();

    let mut retried = 0u32;
    let mut now_enriched = 0u32;
    let mut still_failed = 0u32;
    let mut results = Vec::new();

    for (i, lead) in leads.iter().enumerate() {
        let retry_lead = crate::retry::RetryableLead {
            id: lead.id,
            lead_id: lead.lead_id.clone(),
            name: lead.name.clone(),
            phone: lead.phone.clone(),
            email: lead.email.clone(),
            enrichment_status: lead.enrichment_status.clone(),
            retry_count: lead.retry_count,
            last_retry_at: lead.last_retry_at,
            last_error: lead.last_error.clone(),
            created_at: lead.created_at,
        };

        let eligibility = is_retry_eligible(&retry_lead);
        if !eligibility.eligible {
            results.push(json!({
                "leadId": lead.lead_id,
                "name": lead.name,
                "skipped": true,
                "reason": eligibility.reason,
            }));
            continue;
        }

        retried += 1;

        // Attempt enrichment
        match crate::enrichment::enrich_and_send_workflow(
            state.clone(),
            &lead.lead_id,
            lead.name.as_deref().unwrap_or(""),
            lead.phone.as_deref(),
            lead.email.as_deref(),
        )
        .await
        {
            Ok(result) => {
                now_enriched += 1;
                // Update c2s_leads status
                update_lead_status(
                    db,
                    &lead.lead_id,
                    "completed",
                    lead.retry_count + 1,
                    None,
                )
                .await
                .ok();

                results.push(json!({
                    "leadId": lead.lead_id,
                    "name": lead.name,
                    "previousStatus": lead.enrichment_status,
                    "success": true,
                    "enriched": true,
                    "cpfs": result.cpfs_enriched,
                    "message": "Enrichment completed",
                }));
            }
            Err(e) => {
                still_failed += 1;
                let error_msg = e.to_string();
                update_lead_status(
                    db,
                    &lead.lead_id,
                    lead.enrichment_status.as_deref().unwrap_or("failed"),
                    lead.retry_count + 1,
                    Some(&error_msg),
                )
                .await
                .ok();

                results.push(json!({
                    "leadId": lead.lead_id,
                    "name": lead.name,
                    "previousStatus": lead.enrichment_status,
                    "success": false,
                    "enriched": false,
                    "message": format!("Retry failed: {}", error_msg),
                }));
            }
        }

        // Rate limit between leads (skip after last)
        if i < leads.len() - 1 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    }

    let elapsed = start.elapsed().as_millis();

    Ok(Json(json!({
        "success": true,
        "data": {
            "summary": {
                "totalFailed": total_failed,
                "retried": retried,
                "nowEnriched": now_enriched,
                "stillFailed": still_failed,
                "elapsedMs": elapsed,
            },
            "results": results,
        }
    })))
}

/// Database row from analytics.c2s_leads.
struct C2sLeadRow {
    id: uuid::Uuid,
    lead_id: String,
    name: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    enrichment_status: Option<String>,
    retry_count: i32,
    last_retry_at: Option<chrono::DateTime<Utc>>,
    last_error: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

/// Fetch leads with retryable status from analytics.c2s_leads.
async fn fetch_retryable_leads(
    db: &PgPool,
    limit: i64,
) -> Result<Vec<C2sLeadRow>, AppError> {
    use sqlx::Row;

    let status_list: Vec<&str> = RETRYABLE_STATUSES.to_vec();

    let rows = sqlx::query(
        r#"
        SELECT id, lead_id, customer_name, customer_phone_normalized, customer_email,
               enrichment_status, retry_count, last_retry_at, last_error, received_at
        FROM analytics.c2s_leads
        WHERE enrichment_status = ANY($1)
          AND (retry_count < 5 OR retry_count IS NULL)
        ORDER BY received_at ASC
        LIMIT $2
        "#,
    )
    .bind(&status_list)
    .bind(limit)
    .fetch_all(db)
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to fetch retryable leads: {}", e)))?;

    let mut leads = Vec::with_capacity(rows.len());
    for row in rows {
        leads.push(C2sLeadRow {
            id: row.try_get("id").unwrap_or_default(),
            lead_id: row.try_get::<String, _>("lead_id").unwrap_or_default(),
            name: row.try_get("customer_name").ok(),
            phone: row.try_get("customer_phone_normalized").ok(),
            email: row.try_get("customer_email").ok(),
            enrichment_status: row.try_get("enrichment_status").ok(),
            retry_count: row.try_get::<i32, _>("retry_count").unwrap_or(0),
            last_retry_at: row.try_get("last_retry_at").ok(),
            last_error: row.try_get("last_error").ok(),
            created_at: row.try_get("received_at").unwrap_or_else(|_| Utc::now()),
        });
    }

    Ok(leads)
}

/// Update lead enrichment status, retry count, and error in analytics.c2s_leads.
async fn update_lead_status(
    db: &PgPool,
    lead_id: &str,
    status: &str,
    retry_count: i32,
    error: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE analytics.c2s_leads
        SET enrichment_status = $1,
            retry_count = $2,
            last_retry_at = NOW(),
            last_error = $3,
            updated_at = NOW()
        WHERE lead_id = $4
        "#,
    )
    .bind(status)
    .bind(retry_count)
    .bind(error)
    .bind(lead_id)
    .execute(db)
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to update lead status: {}", e)))?;

    Ok(())
}
