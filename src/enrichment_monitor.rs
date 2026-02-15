//! Enrichment rate monitor — periodic check with threshold alerts.
//!
//! Ported from ts:src/services/enrichment-monitor.service.ts
//!
//! Checks enrichment rate every 6 hours, alerts if below 80%.
//! Exposes stats via /stats/enrichment endpoint.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::alert::{AlertService, TrackedService};

/// Default threshold: 80%
const DEFAULT_THRESHOLD: f64 = 0.80;
/// Check interval: 6 hours
const CHECK_INTERVAL_SECS: u64 = 6 * 3600;

#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentStats {
    pub total: u64,
    pub completed: u64,
    pub partial: u64,
    pub failed: u64,
    pub pending: u64,
    pub rate: f64,
    pub rate_pct: f64,
    pub threshold: f64,
    pub healthy: bool,
    pub checked_at: DateTime<Utc>,
}

pub struct EnrichmentMonitor {
    db: PgPool,
    alert_service: Arc<AlertService>,
    threshold: f64,
    last_stats: Arc<RwLock<Option<EnrichmentStats>>>,
}

impl EnrichmentMonitor {
    pub fn new(db: PgPool, alert_service: Arc<AlertService>) -> Self {
        let threshold = std::env::var("ENRICHMENT_RATE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_THRESHOLD);

        Self {
            db,
            alert_service,
            threshold,
            last_stats: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the latest stats (from cache or fresh query).
    pub async fn get_stats(&self) -> EnrichmentStats {
        let cached = self.last_stats.read().await.clone();
        match cached {
            Some(stats) => stats,
            None => self.check_now().await,
        }
    }

    /// Query the database for current enrichment stats.
    pub async fn check_now(&self) -> EnrichmentStats {
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            r#"
            SELECT
                COUNT(*)::bigint as total,
                COUNT(*) FILTER (WHERE enrichment_status = 'completed')::bigint as completed,
                COUNT(*) FILTER (WHERE enrichment_status IN ('partial', 'basic'))::bigint as partial,
                COUNT(*) FILTER (WHERE enrichment_status IN ('failed'))::bigint as failed,
                COUNT(*) FILTER (WHERE enrichment_status IN ('pending', 'processing'))::bigint as pending
            FROM analytics.c2s_leads
            WHERE received_at >= NOW() - INTERVAL '30 days'
            "#,
        )
        .fetch_optional(&self.db)
        .await;

        let (total, completed, partial, failed, pending) = match row {
            Ok(Some(r)) => r,
            Ok(None) => (0, 0, 0, 0, 0),
            Err(e) => {
                tracing::error!("Enrichment stats query failed: {}", e);
                (0, 0, 0, 0, 0)
            }
        };

        let rate = if total > 0 {
            completed as f64 / total as f64
        } else {
            1.0 // No leads = healthy
        };

        let stats = EnrichmentStats {
            total: total as u64,
            completed: completed as u64,
            partial: partial as u64,
            failed: failed as u64,
            pending: pending as u64,
            rate,
            rate_pct: rate * 100.0,
            threshold: self.threshold,
            healthy: rate >= self.threshold,
            checked_at: Utc::now(),
        };

        // Update cache
        *self.last_stats.write().await = Some(stats.clone());

        stats
    }

    /// Start the periodic monitoring loop.
    pub async fn start_monitoring(self: Arc<Self>) {
        tracing::info!(
            "Enrichment monitor started (threshold: {:.0}%, interval: {}h)",
            self.threshold * 100.0,
            CHECK_INTERVAL_SECS / 3600
        );

        loop {
            let stats = self.check_now().await;

            if !stats.healthy {
                tracing::warn!(
                    "Enrichment rate below threshold: {:.1}% < {:.1}%",
                    stats.rate_pct,
                    self.threshold * 100.0
                );
                self.alert_service
                    .send_low_rate_alert(stats.rate, self.threshold, stats.total, stats.completed)
                    .await;
            } else {
                tracing::info!(
                    "Enrichment rate healthy: {:.1}% (threshold: {:.1}%)",
                    stats.rate_pct,
                    self.threshold * 100.0
                );
            }

            // Update Prometheus gauge
            crate::obs::metrics::ENRICHMENT_RATE
                .set(stats.rate_pct);

            tokio::time::sleep(std::time::Duration::from_secs(CHECK_INTERVAL_SECS)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrichment_stats_healthy() {
        let stats = EnrichmentStats {
            total: 100,
            completed: 85,
            partial: 10,
            failed: 3,
            pending: 2,
            rate: 0.85,
            rate_pct: 85.0,
            threshold: 0.80,
            healthy: true,
            checked_at: Utc::now(),
        };
        assert!(stats.healthy);
        assert!(stats.rate >= stats.threshold);
    }

    #[test]
    fn test_enrichment_stats_unhealthy() {
        let stats = EnrichmentStats {
            total: 100,
            completed: 70,
            partial: 15,
            failed: 10,
            pending: 5,
            rate: 0.70,
            rate_pct: 70.0,
            threshold: 0.80,
            healthy: false,
            checked_at: Utc::now(),
        };
        assert!(!stats.healthy);
    }
}
