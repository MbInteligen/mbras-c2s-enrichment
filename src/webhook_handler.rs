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
        match process_webhook_event(&state.db, event).await {
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
        return Ok(());
    };

    // Extract token from header
    let token = headers
        .get("X-Webhook-Token")
        .or_else(|| headers.get("x-webhook-token"))
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing X-Webhook-Token header".to_string()))?;

    // Constant-time comparison to prevent timing attacks
    if !constant_time_compare(token, expected_secret) {
        tracing::warn!("Invalid webhook token received");
        return Err(AppError::Unauthorized("Invalid webhook token".to_string()));
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
    db: &PgPool,
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

    // 1. Check if already processed (idempotency)
    if already_processed(db, &lead_id, &updated_at_ts).await? {
        return Ok(ProcessResult::Duplicate);
    }

    // 2. Store webhook receipt
    let hook_action = event.hook_action.clone();
    let payload_raw = serde_json::to_value(&event)
        .map_err(|e| AppError::InternalError(format!("Failed to serialize event: {}", e)))?;

    store_webhook_receipt(
        db,
        &lead_id,
        &updated_at_ts,
        hook_action.as_deref(),
        payload_raw,
    )
    .await?;

    // 3. Spawn background enrichment job
    spawn_enrichment_job(db.clone(), lead_id.clone(), updated_at_ts, event);

    Ok(ProcessResult::Processed)
}

/// Check if webhook event was already processed (idempotency check)
async fn already_processed(
    db: &PgPool,
    lead_id: &str,
    updated_at: &DateTime<Utc>,
) -> Result<bool, AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM webhook_events
            WHERE lead_id = $1 AND updated_at = $2
        )
        "#,
    )
    .bind(lead_id)
    .bind(updated_at)
    .fetch_one(db)
    .await?;

    Ok(exists)
}

/// Store webhook receipt in database
async fn store_webhook_receipt(
    db: &PgPool,
    lead_id: &str,
    updated_at: &DateTime<Utc>,
    hook_action: Option<&str>,
    payload_raw: Value,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO webhook_events (lead_id, updated_at, hook_action, payload_raw, status)
        VALUES ($1, $2, $3, $4, 'received')
        "#,
    )
    .bind(lead_id)
    .bind(updated_at)
    .bind(hook_action)
    .bind(payload_raw)
    .execute(db)
    .await?;

    tracing::debug!("Stored webhook receipt for lead_id={}", lead_id);
    Ok(())
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
    db: PgPool,
    lead_id: String,
    updated_at: DateTime<Utc>,
    event: WebhookEvent,
) {
    tokio::spawn(async move {
        tracing::info!("Starting background enrichment for lead_id={}", lead_id);

        // Update status to processing (with specific updated_at to target correct row)
        if let Err(e) = mark_webhook_processing(&db, &lead_id, &updated_at).await {
            tracing::error!("Failed to mark webhook as processing: {}", e);
            return;
        }

        // TODO: Implement full enrichment workflow
        // For now, just log and mark as completed
        // In the next iteration, we'll call the existing c2s_enrich_lead logic

        match enrich_lead_workflow(&db, &lead_id, event).await {
            Ok(_) => {
                tracing::info!("Successfully enriched lead_id={}", lead_id);
                if let Err(e) = mark_webhook_completed(&db, &lead_id, &updated_at).await {
                    tracing::error!("Failed to mark webhook as completed: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to enrich lead_id={}: {}", lead_id, e);
                if let Err(e) =
                    mark_webhook_failed(&db, &lead_id, &updated_at, &e.to_string()).await
                {
                    tracing::error!("Failed to mark webhook as failed: {}", e);
                }
            }
        }
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

/// Placeholder for full enrichment workflow
///
/// TODO: Extract and refactor existing c2s_enrich_lead logic into reusable functions
/// For now, this is a minimal implementation
async fn enrich_lead_workflow(
    _db: &PgPool,
    lead_id: &str,
    event: WebhookEvent,
) -> Result<(), AppError> {
    tracing::debug!("Enrichment workflow for lead_id={}", lead_id);

    // Extract customer data
    if let Some(customer) = event.attributes.customer {
        tracing::debug!(
            "Customer data: name={:?}, email={:?}, phone={:?}",
            customer.name,
            customer.email,
            customer.phone
        );
    }

    // TODO:
    // 1. Extract CPF from customer data or use Diretrix to find it
    // 2. Call Work API to enrich
    // 3. Store enriched data in database
    // 4. Send message back to C2S with enriched info

    // For now, just simulate success
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    Ok(())
}
