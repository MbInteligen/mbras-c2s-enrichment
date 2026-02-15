//! Prometheus metrics for the C2S enrichment API.
//!
//! Metrics spec (from UPGRADE_PLAN.md Phase 0):
//! - enrichment_requests_total: Counter with status=success|error|partial
//! - enrichment_duration_seconds: Histogram with tier=1|2|3|4|5
//! - cpf_discovery_total: Counter with tier + result labels
//! - http_requests_total: Counter with method, route_template, status

use prometheus::{
    register_counter_vec, register_histogram_vec, CounterVec, Encoder, HistogramVec, TextEncoder,
};
use std::sync::LazyLock;

/// Enrichment request outcomes (success, error, partial).
pub static ENRICHMENT_REQUESTS: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "enrichment_requests_total",
        "Total enrichment requests by status",
        &["status"]
    )
    .expect("enrichment_requests_total counter")
});

/// Enrichment duration histogram bucketed by lead tier.
pub static ENRICHMENT_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "enrichment_duration_seconds",
        "Enrichment duration in seconds by tier",
        &["tier"],
        vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]
    )
    .expect("enrichment_duration_seconds histogram")
});

/// CPF discovery outcomes by tier (work_phone, work_name, duckdb, diretrix, dbase) and result.
pub static CPF_DISCOVERY: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "cpf_discovery_total",
        "CPF discovery attempts by tier and result",
        &["tier", "result"]
    )
    .expect("cpf_discovery_total counter")
});

/// HTTP requests by method, route_template, and status code.
pub static HTTP_REQUESTS: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "http_requests_total",
        "HTTP requests by method, route, and status",
        &["method", "route_template", "status"]
    )
    .expect("http_requests_total counter")
});

/// Initialize all metrics (forces LazyLock evaluation).
pub fn init() {
    LazyLock::force(&ENRICHMENT_REQUESTS);
    LazyLock::force(&ENRICHMENT_DURATION);
    LazyLock::force(&CPF_DISCOVERY);
    LazyLock::force(&HTTP_REQUESTS);
}

/// Render all metrics as Prometheus text exposition format.
pub fn render() -> String {
    let encoder = TextEncoder::new();
    let families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
