//! Enrichment cron — background retry loop.
//!
//! Port of ts-c2s-api's enrichment cron behavior.
//! Retries failed/partial leads with schedule-aware intervals:
//! - Business hours (9-18): every 5 minutes
//! - Evening (18-23): every 20 minutes
//! - Night (23-9): every 60 minutes

use crate::config::Config;
use crate::handlers::AppState;
use crate::retry::{is_retry_eligible, RETRYABLE_STATUSES};
use chrono::{Local, Timelike};
use std::sync::Arc;

/// Start the enrichment cron loop as a background task.
///
/// Runs indefinitely, retrying failed leads at schedule-aware intervals.
/// Call this from main.rs with `tokio::spawn(cron::start_enrichment_cron(state))`.
pub async fn start_enrichment_cron(state: Arc<AppState>) {
    tracing::info!("Enrichment cron started");

    loop {
        let interval_secs = get_current_interval(&state.config);
        tracing::debug!("Cron sleeping for {}s", interval_secs);
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

        // Fetch and retry eligible leads
        match retry_eligible_leads(&state).await {
            Ok((retried, enriched)) => {
                if retried > 0 {
                    tracing::info!(
                        "Cron cycle: retried {}, enriched {} leads",
                        retried,
                        enriched
                    );
                }
            }
            Err(e) => {
                tracing::error!("Cron cycle failed: {}", e);
            }
        }
    }
}

/// Get the appropriate interval based on current hour.
fn get_current_interval(config: &Config) -> u64 {
    let hour = Local::now().hour();
    if hour >= 9 && hour < 18 {
        config.cron_interval_business_secs
    } else if hour >= 18 && hour < 23 {
        config.cron_interval_evening_secs
    } else {
        config.cron_interval_night_secs
    }
}

/// Retry all eligible leads in one cron cycle.
async fn retry_eligible_leads(state: &Arc<AppState>) -> Result<(u32, u32), String> {
    let db = &state.db;

    // Fetch up to 10 retryable leads per cycle
    let rows = sqlx::query_as::<_, RetryRow>(
        r#"
        SELECT id, lead_id, customer_name, customer_phone_normalized, customer_email,
               enrichment_status, retry_count, last_retry_at, last_error, received_at
        FROM analytics.c2s_leads
        WHERE enrichment_status = ANY($1)
          AND (retry_count < 5 OR retry_count IS NULL)
        ORDER BY received_at ASC
        LIMIT 10
        "#,
    )
    .bind(&RETRYABLE_STATUSES.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    .fetch_all(db)
    .await
    .map_err(|e| format!("DB query failed: {}", e))?;

    if rows.is_empty() {
        return Ok((0, 0));
    }

    let mut retried = 0u32;
    let mut enriched = 0u32;

    for row in &rows {
        let retry_lead = crate::retry::RetryableLead {
            id: row.id,
            lead_id: row.lead_id.clone(),
            name: row.customer_name.clone(),
            phone: row.customer_phone_normalized.clone(),
            email: row.customer_email.clone(),
            enrichment_status: row.enrichment_status.clone(),
            retry_count: row.retry_count.unwrap_or(0),
            last_retry_at: row.last_retry_at,
            last_error: row.last_error.clone(),
            created_at: row.received_at,
        };

        if !is_retry_eligible(&retry_lead).eligible {
            continue;
        }

        retried += 1;

        match crate::enrichment::enrich_and_send_workflow(
            state.clone(),
            &row.lead_id,
            row.customer_name.as_deref().unwrap_or(""),
            row.customer_phone_normalized.as_deref(),
            row.customer_email.as_deref(),
        )
        .await
        {
            Ok(_) => {
                enriched += 1;
                update_retry_status(db, &row.lead_id, "completed", row.retry_count.unwrap_or(0) + 1, None).await;
            }
            Err(e) => {
                let err_msg = e.to_string();
                let status = row.enrichment_status.as_deref().unwrap_or("failed");
                update_retry_status(db, &row.lead_id, status, row.retry_count.unwrap_or(0) + 1, Some(&err_msg)).await;
            }
        }

        // Rate limit: 2s between retries
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    Ok((retried, enriched))
}

async fn update_retry_status(db: &sqlx::PgPool, lead_id: &str, status: &str, retry_count: i32, error: Option<&str>) {
    let _ = sqlx::query(
        "UPDATE analytics.c2s_leads SET enrichment_status = $1, retry_count = $2, last_retry_at = NOW(), last_error = $3, updated_at = NOW() WHERE lead_id = $4"
    )
    .bind(status)
    .bind(retry_count)
    .bind(error)
    .bind(lead_id)
    .execute(db)
    .await;
}

/// Internal row type for query.
#[derive(sqlx::FromRow)]
struct RetryRow {
    id: uuid::Uuid,
    lead_id: String,
    customer_name: Option<String>,
    customer_phone_normalized: Option<String>,
    customer_email: Option<String>,
    enrichment_status: Option<String>,
    retry_count: Option<i32>,
    last_retry_at: Option<chrono::DateTime<chrono::Utc>>,
    last_error: Option<String>,
    received_at: chrono::DateTime<chrono::Utc>,
}
