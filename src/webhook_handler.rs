use crate::errors::AppError;
use crate::handlers::AppState;
use crate::webhook_models::{WebhookEvent, WebhookPayload, WebhookResponse};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// C2S Webhook Handler
///
/// Receives webhook events from Contact2Sale (C2S) when leads are created/updated.
/// Validates the webhook secret, deduplicates events, and triggers background enrichment.
///
/// Expected payload: Single event object OR array of events
/// Authentication: X-Webhook-Token header must match WEBHOOK_SECRET env var
pub async fn c2s_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<WebhookPayload>,
) -> Result<(StatusCode, Json<WebhookResponse>), AppError> {
    tracing::info!("Received C2S webhook");

    // 1. Validate webhook secret (if configured)
    validate_webhook_secret(&state, &headers)?;

    // 2. Convert payload to vec of events (handles both single and batch)
    let events = payload.into_events();
    let total_received = events.len();
    tracing::info!("Processing {} webhook event(s)", total_received);

    let mut processed = 0;
    let mut duplicates = 0;

    // 3. Process each event
    for event in events {
        match process_webhook_event(&state, event).await {
            Ok(ProcessResult::Processed) => {
                processed += 1;
            }
            Ok(ProcessResult::Duplicate) => {
                duplicates += 1;
                tracing::debug!("Skipped duplicate webhook event");
            }
            Err(e) => {
                tracing::error!("Failed to process webhook event: {}", e);
                // Continue processing other events even if one fails
            }
        }
    }

    tracing::info!(
        "Webhook processing complete: {} received, {} processed, {} duplicates",
        total_received,
        processed,
        duplicates
    );

    // 4. Return 200 immediately (background jobs will handle enrichment)
    Ok((
        StatusCode::OK,
        Json(WebhookResponse {
            status: "received".to_string(),
            received: total_received,
            processed,
            duplicates,
        }),
    ))
}

/// Validate webhook secret from X-Webhook-Token header
fn validate_webhook_secret(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    // If no secret is configured, skip validation (warn was already logged at startup)
    let Some(ref expected_secret) = state.config.webhook_secret else {
        tracing::debug!("Webhook secret not configured, skipping validation");
        return Ok(());
    };

    // Extract token from header (optional - C2S doesn't support custom headers)
    let token = headers
        .get("X-Webhook-Token")
        .or_else(|| headers.get("x-webhook-token"))
        .and_then(|v| v.to_str().ok());

    // If token is provided, validate it
    if let Some(token_value) = token {
        // Constant-time comparison to prevent timing attacks
        if !constant_time_compare(token_value, expected_secret) {
            tracing::warn!("Invalid webhook token received");
            return Err(AppError::Unauthorized("Invalid webhook token".to_string()));
        }
        tracing::debug!("Webhook token validated successfully");
    } else {
        // No token provided - this is OK for C2S direct webhooks
        // (C2S doesn't support custom headers in /leads/subscribe API)
        tracing::debug!("No webhook token provided (C2S direct webhook)");
    }

    Ok(())
}

/// Constant-time string comparison (basic implementation)
/// For production, consider using a crypto library like `subtle`
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.as_bytes()
        .iter()
        .zip(b.as_bytes().iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[derive(Debug)]
enum ProcessResult {
    Processed,
    Duplicate,
}

/// Parse timestamp string to DateTime<Utc>
fn parse_timestamp(timestamp_str: &str) -> Result<DateTime<Utc>, AppError> {
    // Try ISO 8601 / RFC3339 format first (standard)
    chrono::DateTime::parse_from_rfc3339(timestamp_str)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Fallback: try custom format with timezone
            chrono::DateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S%.f %z")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .or_else(|_| {
            // Fallback: try naive datetime and assume UTC
            chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S%.f")
                .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
        })
        .map_err(|e| {
            AppError::BadRequest(format!(
                "Invalid timestamp format '{}': {}. Expected ISO 8601 (RFC3339)",
                timestamp_str, e
            ))
        })
}

/// Process a single webhook event
async fn process_webhook_event(
    state: &Arc<AppState>,
    event: WebhookEvent,
) -> Result<ProcessResult, AppError> {
    let lead_id = event.id.clone();

    // Extract updated_at timestamp (required for idempotency)
    let updated_at_str =
        event.attributes.updated_at.as_ref().ok_or_else(|| {
            AppError::BadRequest("Missing updated_at in webhook event".to_string())
        })?;

    // Parse timestamp immediately for type safety
    let updated_at_ts = parse_timestamp(updated_at_str)?;

    tracing::debug!(
        "Processing webhook event: lead_id={}, updated_at={}",
        lead_id,
        updated_at_str
    );

    // 1. Atomic claim: INSERT ... ON CONFLICT DO NOTHING
    //    Replaces the old two-step check+insert which had a TOCTOU race
    //    when multiple Fly.io instances receive the same webhook.
    let hook_action = event.hook_action.clone();
    let payload_raw = serde_json::to_value(&event)
        .map_err(|e| AppError::InternalError(format!("Failed to serialize event: {}", e)))?;

    let claimed = try_claim_webhook(
        &state.db,
        &lead_id,
        &updated_at_ts,
        hook_action.as_deref(),
        payload_raw,
    )
    .await?;

    if !claimed {
        return Ok(ProcessResult::Duplicate);
    }

    // 2. Auto-save lead to analytics.c2s_leads (before enrichment, never lose a lead)
    if let Err(e) = upsert_c2s_lead(&state.db, &event).await {
        tracing::warn!("Failed to auto-save lead {}: {} (enrichment continues)", lead_id, e);
    }

    // 3. Spawn background enrichment job (with advisory lock inside)
    spawn_enrichment_job(state.clone(), lead_id.clone(), updated_at_ts, event);

    Ok(ProcessResult::Processed)
}

/// Auto-save webhook lead to analytics.c2s_leads.
///
/// Persists lead data before enrichment starts, ensuring no lead is lost
/// even if enrichment fails. Uses INSERT ... ON CONFLICT to avoid duplicates.
async fn upsert_c2s_lead(
    db: &PgPool,
    event: &WebhookEvent,
) -> Result<(), AppError> {
    let customer = event.attributes.customer.as_ref();
    let name = customer.and_then(|c| c.name.as_deref());
    let email = customer.and_then(|c| c.email.as_deref());
    let phone = customer.and_then(|c| c.phone.as_deref());

    // Normalize phone: strip non-digits
    let phone_normalized = phone.map(|p| {
        p.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
    });

    let hook_action = event.hook_action.as_deref();
    let product_desc = event.attributes.product.as_ref()
        .and_then(|p| p.description.as_deref());
    let lead_status = event.attributes.lead_status.as_ref()
        .and_then(|s| serde_json::to_value(s).ok())
        .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from));

    let raw_payload = serde_json::to_value(event)
        .unwrap_or(serde_json::json!({}));

    sqlx::query(
        r#"
        INSERT INTO analytics.c2s_leads (
            lead_id, customer_name, customer_email, customer_phone,
            customer_phone_normalized, hook_action, product_description,
            lead_status, raw_payload, enrichment_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending')
        ON CONFLICT (lead_id) DO UPDATE SET
            customer_name = COALESCE(EXCLUDED.customer_name, analytics.c2s_leads.customer_name),
            customer_email = COALESCE(EXCLUDED.customer_email, analytics.c2s_leads.customer_email),
            customer_phone = COALESCE(EXCLUDED.customer_phone, analytics.c2s_leads.customer_phone),
            customer_phone_normalized = COALESCE(EXCLUDED.customer_phone_normalized, analytics.c2s_leads.customer_phone_normalized),
            hook_action = EXCLUDED.hook_action,
            raw_payload = EXCLUDED.raw_payload,
            updated_at = NOW()
        "#,
    )
    .bind(&event.id)
    .bind(name)
    .bind(email)
    .bind(phone)
    .bind(phone_normalized.as_deref())
    .bind(hook_action)
    .bind(product_desc)
    .bind(lead_status.as_deref())
    .bind(&raw_payload)
    .execute(db)
    .await
    .map_err(|e| AppError::InternalError(format!("c2s_leads upsert failed: {}", e)))?;

    tracing::debug!("Auto-saved lead {} to c2s_leads", event.id);
    Ok(())
}

/// Atomic webhook claim: INSERT ... ON CONFLICT DO NOTHING
///
/// Returns true if this instance won the claim (row was inserted).
/// Returns false if another instance already claimed it (duplicate).
/// Requires UNIQUE index on (lead_id, updated_at) -- see migration 019.
async fn try_claim_webhook(
    db: &PgPool,
    lead_id: &str,
    updated_at: &DateTime<Utc>,
    hook_action: Option<&str>,
    payload_raw: Value,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        r#"
        INSERT INTO webhook_events (lead_id, updated_at, hook_action, payload_raw, status)
        VALUES ($1, $2, $3, $4, 'received')
        ON CONFLICT (lead_id, updated_at) DO NOTHING
        "#,
    )
    .bind(lead_id)
    .bind(updated_at)
    .bind(hook_action)
    .bind(payload_raw)
    .execute(db)
    .await?;

    let claimed = result.rows_affected() > 0;
    if claimed {
        tracing::debug!("Claimed webhook event: lead_id={}", lead_id);
    }
    Ok(claimed)
}

/// Compute a stable i64 hash for use as pg_try_advisory_lock key.
/// Uses DefaultHasher (SipHash) for distribution quality.
fn lead_lock_key(lead_id: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    lead_id.hash(&mut hasher);
    hasher.finish() as i64
}

/// Spawn background enrichment job (non-blocking)
///
/// This function spawns a tokio task that will:
/// 1. Mark webhook event as 'processing'
/// 2. Fetch full lead data from C2S
/// 3. Extract CPF from customer data
/// 4. Enrich via Work API
/// 5. Store in database
/// 6. Send enriched message back to C2S
/// 7. Mark webhook event as 'completed' or 'failed'
fn spawn_enrichment_job(
    state: Arc<AppState>,
    lead_id: String,
    updated_at: DateTime<Utc>,
    event: WebhookEvent,
) {
    tokio::spawn(async move {
        tracing::info!("Starting background enrichment for lead_id={}", lead_id);

        // Acquire advisory lock to prevent parallel enrichment of the same lead.
        // pg_try_advisory_lock is session-scoped and auto-released on disconnect.
        // This is defense-in-depth: the atomic claim above is the primary guard.
        let lock_key = lead_lock_key(&lead_id);
        let locked: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_lock($1)"
        )
        .bind(lock_key)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

        if !locked {
            tracing::warn!(
                "Advisory lock not acquired for lead_id={} (another instance processing)",
                lead_id
            );
            return;
        }

        // Update status to processing (with specific updated_at to target correct row)
        if let Err(e) = mark_webhook_processing(&state.db, &lead_id, &updated_at).await {
            tracing::error!("Failed to mark webhook as processing: {}", e);
            let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(lock_key)
                .execute(&state.db)
                .await;
            return;
        }

        // Run full enrichment workflow
        match enrich_lead_workflow(&state, &lead_id, event).await {
            Ok(_) => {
                tracing::info!("Successfully enriched lead_id={}", lead_id);
                if let Err(e) = mark_webhook_completed(&state.db, &lead_id, &updated_at).await {
                    tracing::error!("Failed to mark webhook as completed: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to enrich lead_id={}: {}", lead_id, e);
                if let Err(e) =
                    mark_webhook_failed(&state.db, &lead_id, &updated_at, &e.to_string()).await
                {
                    tracing::error!("Failed to mark webhook as failed: {}", e);
                }
            }
        }

        // Release advisory lock (session-level, must be explicitly released)
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(lock_key)
            .execute(&state.db)
            .await;
    });
}

/// Mark webhook event as processing (scoped by lead_id AND updated_at)
async fn mark_webhook_processing(
    db: &PgPool,
    lead_id: &str,
    updated_at: &DateTime<Utc>,
) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE webhook_events
        SET status = 'processing', updated_at_ts = now()
        WHERE lead_id = $1 AND updated_at = $2 AND status = 'received'
        "#,
    )
    .bind(lead_id)
    .bind(updated_at)
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        tracing::warn!(
            "No webhook event found to mark as processing: lead_id={}, updated_at={}",
            lead_id,
            updated_at
        );
    }

    Ok(())
}

/// Mark webhook event as completed (scoped by lead_id AND updated_at)
async fn mark_webhook_completed(
    db: &PgPool,
    lead_id: &str,
    updated_at: &DateTime<Utc>,
) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE webhook_events
        SET status = 'completed', processed_at = now(), updated_at_ts = now()
        WHERE lead_id = $1 AND updated_at = $2 AND status = 'processing'
        "#,
    )
    .bind(lead_id)
    .bind(updated_at)
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        tracing::warn!(
            "No webhook event found to mark as completed: lead_id={}, updated_at={}",
            lead_id,
            updated_at
        );
    }

    Ok(())
}

/// Mark webhook event as failed (scoped by lead_id AND updated_at)
async fn mark_webhook_failed(
    db: &PgPool,
    lead_id: &str,
    updated_at: &DateTime<Utc>,
    error_message: &str,
) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE webhook_events
        SET status = 'failed', error_message = $2, updated_at_ts = now()
        WHERE lead_id = $1 AND updated_at = $3 AND status = 'processing'
        "#,
    )
    .bind(lead_id)
    .bind(error_message)
    .bind(updated_at)
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        tracing::warn!(
            "No webhook event found to mark as failed: lead_id={}, updated_at={}",
            lead_id,
            updated_at
        );
    }

    Ok(())
}

/// Full enrichment workflow for webhook events
///
/// This function orchestrates the complete enrichment process:
/// 1. Extract customer data from webhook
/// 2. Find CPF via Diretrix (phone/email lookup)
/// 3. Enrich with Work API
/// 4. Format and send message to C2S
/// 5. Store in database
async fn enrich_lead_workflow(
    state: &Arc<AppState>,
    lead_id: &str,
    event: WebhookEvent,
) -> Result<(), AppError> {
    tracing::info!("Starting enrichment workflow for lead_id={}", lead_id);

    // Extract customer data from webhook
    let customer = event
        .attributes
        .customer
        .ok_or_else(|| AppError::BadRequest("Missing customer data in webhook".to_string()))?;

    let customer_name = customer.name.as_deref().unwrap_or("Unknown");
    let phone = customer.phone.as_deref().filter(|s| !s.is_empty());
    let email = customer.email.as_deref().filter(|s| !s.is_empty());

    tracing::info!(
        "Customer: name={}, phone={:?}, email={:?}",
        customer_name,
        phone,
        email
    );

    // Run full enrichment workflow using shared module
    let result = crate::enrichment::enrich_and_send_workflow(
        state.clone(),
        lead_id,
        customer_name,
        phone,
        email,
    )
    .await?;

    tracing::info!(
        "Enrichment complete: {} CPFs enriched, {} stored in DB",
        result.cpfs_enriched.len(),
        result.stored_count
    );

    Ok(())
}
