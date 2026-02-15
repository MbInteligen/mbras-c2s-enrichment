//! Prometheus metrics for the C2S enrichment API.
//!
//! Phase 0: Core metrics (enrichment requests, duration, CPF discovery, HTTP)
//! Phase 4: Extended metrics (API call counters, cache hits, service health, enrichment rate)

use prometheus::{
    register_counter_vec, register_gauge, register_gauge_vec, register_histogram_vec,
    CounterVec, Encoder, Gauge, GaugeVec, HistogramVec, TextEncoder,
};
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Phase 0: Core metrics
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Phase 4: Extended metrics
// ---------------------------------------------------------------------------

/// HTTP request duration histogram by route.
pub static HTTP_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "http_request_duration_seconds",
        "HTTP request duration by route",
        &["method", "route_template"],
        vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .expect("http_request_duration_seconds histogram")
});

/// External API call counter by service and outcome.
pub static API_CALLS: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "external_api_calls_total",
        "External API calls by service and result",
        &["service", "result"]
    )
    .expect("external_api_calls_total counter")
});

/// External API call duration by service.
pub static API_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "external_api_duration_seconds",
        "External API call duration by service",
        &["service"],
        vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]
    )
    .expect("external_api_duration_seconds histogram")
});

/// Cache hit/miss counter by cache name.
pub static CACHE_OPS: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "cache_operations_total",
        "Cache hits and misses by cache name",
        &["cache", "result"]
    )
    .expect("cache_operations_total counter")
});

/// Service health gauge (1 = healthy, 0 = unhealthy).
pub static SERVICE_HEALTH: LazyLock<GaugeVec> = LazyLock::new(|| {
    register_gauge_vec!(
        "service_health",
        "Service health status (1=healthy, 0=unhealthy)",
        &["service"]
    )
    .expect("service_health gauge")
});

/// Current enrichment rate percentage.
pub static ENRICHMENT_RATE: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge!(
        "enrichment_rate_pct",
        "Current enrichment rate percentage (0-100)"
    )
    .expect("enrichment_rate_pct gauge")
});

/// Active alerts gauge by type.
pub static ACTIVE_ALERTS: LazyLock<GaugeVec> = LazyLock::new(|| {
    register_gauge_vec!(
        "active_alerts",
        "Number of active alerts by type",
        &["alert_type"]
    )
    .expect("active_alerts gauge")
});

/// Alerts sent counter by type and channel.
pub static ALERTS_SENT: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "alerts_sent_total",
        "Total alerts sent by type and channel",
        &["alert_type", "channel"]
    )
    .expect("alerts_sent_total counter")
});

/// Leads processed counter by status (completed, partial, failed).
pub static LEADS_PROCESSED: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "leads_processed_total",
        "Total leads processed by final status",
        &["status"]
    )
    .expect("leads_processed_total counter")
});

/// Webhook received counter by source.
pub static WEBHOOKS_RECEIVED: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "webhooks_received_total",
        "Webhooks received by source",
        &["source"]
    )
    .expect("webhooks_received_total counter")
});

/// Meilisearch query counter.
pub static MEILISEARCH_QUERIES: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "meilisearch_queries_total",
        "Meilisearch queries by result",
        &["result"]
    )
    .expect("meilisearch_queries_total counter")
});

/// Fly.io scaling events counter.
pub static FLY_SCALE_EVENTS: LazyLock<CounterVec> = LazyLock::new(|| {
    register_counter_vec!(
        "fly_scale_events_total",
        "Fly.io auto-scaling events",
        &["machine", "direction"]
    )
    .expect("fly_scale_events_total counter")
});

// ---------------------------------------------------------------------------
// Init & Render
// ---------------------------------------------------------------------------

/// Initialize all metrics (forces LazyLock evaluation).
pub fn init() {
    // Phase 0
    LazyLock::force(&ENRICHMENT_REQUESTS);
    LazyLock::force(&ENRICHMENT_DURATION);
    LazyLock::force(&CPF_DISCOVERY);
    LazyLock::force(&HTTP_REQUESTS);
    // Phase 4
    LazyLock::force(&HTTP_DURATION);
    LazyLock::force(&API_CALLS);
    LazyLock::force(&API_DURATION);
    LazyLock::force(&CACHE_OPS);
    LazyLock::force(&SERVICE_HEALTH);
    LazyLock::force(&ENRICHMENT_RATE);
    LazyLock::force(&ACTIVE_ALERTS);
    LazyLock::force(&ALERTS_SENT);
    LazyLock::force(&LEADS_PROCESSED);
    LazyLock::force(&WEBHOOKS_RECEIVED);
    LazyLock::force(&MEILISEARCH_QUERIES);
    LazyLock::force(&FLY_SCALE_EVENTS);
}

/// Render all metrics as Prometheus text exposition format.
pub fn render() -> String {
    let encoder = TextEncoder::new();
    let families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
