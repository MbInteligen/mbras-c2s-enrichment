//! Retry service for failed/partial enrichments.
//!
//! Port of ts-c2s-api `src/services/retry.service.ts`.
//! Implements exponential backoff retry with configurable max attempts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Retry delays in seconds: 1h, 2h, 4h, 8h, 16h
const RETRY_DELAYS_SECS: &[u64] = &[3600, 7200, 14400, 28800, 57600];

/// Default maximum retry attempts.
const DEFAULT_MAX_RETRIES: i32 = 5;

/// Statuses eligible for retry.
pub const RETRYABLE_STATUSES: &[&str] = &["partial", "unenriched", "basic"];

/// A lead that may be retried.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryableLead {
    pub id: uuid::Uuid,
    pub lead_id: String,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub enrichment_status: Option<String>,
    pub retry_count: i32,
    pub last_retry_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Retry eligibility result.
#[derive(Debug, Clone, Serialize)]
pub struct RetryEligibility {
    pub eligible: bool,
    pub reason: String,
    pub next_retry_in: Option<Duration>,
}

/// Get the backoff delay for a given retry count (0-indexed).
///
/// Clamps to the last value (16h) for retry counts beyond the array.
pub fn get_retry_delay(retry_count: i32) -> Duration {
    let idx = (retry_count as usize).min(RETRY_DELAYS_SECS.len() - 1);
    Duration::from_secs(RETRY_DELAYS_SECS[idx])
}

/// Human-readable delay string.
pub fn get_retry_delay_human(retry_count: i32) -> String {
    let secs = get_retry_delay(retry_count).as_secs();
    let hours = secs / 3600;
    if hours == 1 {
        "1 hour".to_string()
    } else {
        format!("{} hours", hours)
    }
}

/// Check if a lead has exceeded the maximum retry count.
pub fn has_exceeded_max_retries(lead: &RetryableLead) -> bool {
    lead.retry_count >= DEFAULT_MAX_RETRIES
}

/// Check if a lead is eligible for retry right now.
///
/// A lead is eligible if:
/// 1. Its status is in RETRYABLE_STATUSES
/// 2. retry_count < MAX_RETRIES
/// 3. Enough time has elapsed since last_retry_at (exponential backoff)
pub fn is_retry_eligible(lead: &RetryableLead) -> RetryEligibility {
    // Check status
    let status = lead.enrichment_status.as_deref().unwrap_or("");
    if !RETRYABLE_STATUSES.contains(&status) {
        return RetryEligibility {
            eligible: false,
            reason: format!("Status '{}' is not retryable", status),
            next_retry_in: None,
        };
    }

    // Check max retries
    if lead.retry_count >= DEFAULT_MAX_RETRIES {
        return RetryEligibility {
            eligible: false,
            reason: format!(
                "Exceeded max retries ({}/{})",
                lead.retry_count, DEFAULT_MAX_RETRIES
            ),
            next_retry_in: None,
        };
    }

    // Check backoff timing
    if let Some(last_retry) = lead.last_retry_at {
        let delay = get_retry_delay(lead.retry_count);
        let elapsed = Utc::now()
            .signed_duration_since(last_retry)
            .to_std()
            .unwrap_or(Duration::ZERO);
        if elapsed < delay {
            let remaining = delay - elapsed;
            return RetryEligibility {
                eligible: false,
                reason: format!(
                    "Backoff: {} remaining (retry {})",
                    format_duration(remaining),
                    lead.retry_count
                ),
                next_retry_in: Some(remaining),
            };
        }
    }
    // No last_retry_at means first attempt — eligible immediately

    RetryEligibility {
        eligible: true,
        reason: "Ready for retry".to_string(),
        next_retry_in: None,
    }
}

/// Time remaining until next retry is allowed. Returns None if ready now.
pub fn time_until_next_retry(lead: &RetryableLead) -> Option<Duration> {
    let last_retry = lead.last_retry_at?;
    let delay = get_retry_delay(lead.retry_count);
    let elapsed = Utc::now()
        .signed_duration_since(last_retry)
        .to_std()
        .unwrap_or(Duration::ZERO);
    if elapsed >= delay {
        None
    } else {
        Some(delay - elapsed)
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lead(status: &str, retry_count: i32, last_retry: Option<DateTime<Utc>>) -> RetryableLead {
        RetryableLead {
            id: uuid::Uuid::new_v4(),
            lead_id: "test-lead-1".to_string(),
            name: Some("Test Lead".to_string()),
            phone: Some("11999887766".to_string()),
            email: None,
            enrichment_status: Some(status.to_string()),
            retry_count,
            last_retry_at: last_retry,
            last_error: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_retry_delays() {
        assert_eq!(get_retry_delay(0).as_secs(), 3600);     // 1h
        assert_eq!(get_retry_delay(1).as_secs(), 7200);     // 2h
        assert_eq!(get_retry_delay(2).as_secs(), 14400);    // 4h
        assert_eq!(get_retry_delay(3).as_secs(), 28800);    // 8h
        assert_eq!(get_retry_delay(4).as_secs(), 57600);    // 16h
        assert_eq!(get_retry_delay(5).as_secs(), 57600);    // 16h (clamped)
        assert_eq!(get_retry_delay(100).as_secs(), 57600);  // 16h (clamped)
    }

    #[test]
    fn test_eligible_partial_no_retry_yet() {
        let lead = make_lead("partial", 0, None);
        let result = is_retry_eligible(&lead);
        assert!(result.eligible);
    }

    #[test]
    fn test_eligible_unenriched() {
        let lead = make_lead("unenriched", 0, None);
        assert!(is_retry_eligible(&lead).eligible);
    }

    #[test]
    fn test_eligible_basic() {
        let lead = make_lead("basic", 0, None);
        assert!(is_retry_eligible(&lead).eligible);
    }

    #[test]
    fn test_not_eligible_completed() {
        let lead = make_lead("completed", 0, None);
        assert!(!is_retry_eligible(&lead).eligible);
    }

    #[test]
    fn test_not_eligible_max_retries() {
        let lead = make_lead("partial", 5, None);
        assert!(!is_retry_eligible(&lead).eligible);
        assert!(has_exceeded_max_retries(&lead));
    }

    #[test]
    fn test_not_eligible_backoff() {
        // Last retry was 30 minutes ago, needs 1 hour
        let last = Utc::now() - chrono::Duration::minutes(30);
        let lead = make_lead("partial", 0, Some(last));
        let result = is_retry_eligible(&lead);
        assert!(!result.eligible);
        assert!(result.next_retry_in.is_some());
    }

    #[test]
    fn test_eligible_after_backoff() {
        // Last retry was 2 hours ago, needs 1 hour (retry_count=0)
        let last = Utc::now() - chrono::Duration::hours(2);
        let lead = make_lead("partial", 0, Some(last));
        assert!(is_retry_eligible(&lead).eligible);
    }

    #[test]
    fn test_human_delays() {
        assert_eq!(get_retry_delay_human(0), "1 hour");
        assert_eq!(get_retry_delay_human(1), "2 hours");
        assert_eq!(get_retry_delay_human(4), "16 hours");
    }
}
