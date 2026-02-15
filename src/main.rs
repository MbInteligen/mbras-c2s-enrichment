mod cache_validator;
mod circuit_breaker;
mod config;
mod db;
mod db_storage;
mod enrichment;
mod errors;
mod gateway_client;
mod google_ads_handler;
mod google_ads_models;
mod handlers;
mod models;
mod services;
mod webhook_handler;
mod webhook_models;
mod obs;
mod scoring;
mod cpf;
mod name_matcher;
mod discovery;
mod retry;
mod batch;
mod cron;
mod meilisearch;
mod fly_scale;
mod cnpj_fallback;
mod alert;
mod enrichment_monitor;
mod dashboard;
mod api_auth;
mod lead_analysis;
mod domain_analyzer;
mod risk_detector;
mod web_search;

use axum::{
    extract::{Path, State},
    middleware as axum_middleware,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::db::Database;

/// Serves the OpenAPI specification YAML file
async fn serve_openapi_spec() -> impl IntoResponse {
    match tokio::fs::read_to_string("openapi.yml").await {
        Ok(content) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/yaml")],
            content,
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            "OpenAPI spec not found. Generate with: cargo run --bin generate-openapi",
        )
            .into_response(),
    }
}

/// Serves the Swagger UI HTML page
async fn serve_swagger_ui() -> impl IntoResponse {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Rust C2S API - Swagger UI</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
    <style>
        body { margin: 0; padding: 0; }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = function() {
            window.ui = SwaggerUIBundle({
                url: "/api-docs/openapi.yml",
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                layout: "StandaloneLayout"
            });
        };
    </script>
</body>
</html>
"#;
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rust_c2s_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env()?;
    tracing::info!("Configuration loaded successfully");

    // Initialize database connection pool
    let db = Database::new(&config.database_url).await?;
    tracing::info!("Database connection pool established");

    // Create global CPF deduplication cache (5 minute TTL, 10k max entries)
    let recent_cpf_cache = Cache::builder()
        .time_to_live(Duration::from_secs(300))
        .max_capacity(10_000)
        .build();
    tracing::info!("CPF deduplication cache initialized");

    // Create lead-level deduplication cache to prevent concurrent duplicate requests
    // 5 minute TTL is enough to cover typical request processing time
    let processing_leads_cache = Cache::builder()
        .time_to_live(Duration::from_secs(300))
        .max_capacity(10_000)
        .build();
    tracing::info!("Lead deduplication cache initialized");

    // Create contact -> CPF cache (24 hour TTL)
    // Used to skip external API calls for known contacts
    let contact_to_cpf_cache = Cache::builder()
        .time_to_live(Duration::from_secs(86400))
        .max_capacity(50_000)
        .build();
    tracing::info!("Contact enrichment cache initialized");

    // Create Work API response cache (1 hour TTL, 100k max entries)
    // Caches raw Work API responses to reduce external API calls and improve performance
    let work_api_cache = Cache::builder()
        .time_to_live(Duration::from_secs(3600)) // 1 hour
        .max_capacity(100_000)
        .build();
    tracing::info!("Work API response cache initialized (1h TTL, 100k capacity)");

    // Initialize C2S direct client
    // Formerly "gateway client", now communicates directly with C2S API
    let gateway_client = match gateway_client::C2sGatewayClient::new(
        config.c2s_base_url.clone(),
        config.c2s_token.clone(),
    ) {
        Ok(client) => {
            tracing::info!("✓ C2S Direct Client initialized: {}", config.c2s_base_url);
            Some(client)
        }
        Err(e) => {
            tracing::error!("Failed to initialize C2S client: {}", e);
            None
        }
    };

    // Initialize alert service
    let alert_service = std::sync::Arc::new(crate::alert::AlertService::new());

    // Initialize session store for dashboard
    let sessions = std::sync::Arc::new(crate::dashboard::SessionStore::new());

    // Build application state
    let app_state = std::sync::Arc::new(crate::handlers::AppState {
        db: db.pool.clone(),
        config: config.clone(),
        gateway_client,
        recent_cpf_cache,
        processing_leads_cache,
        contact_to_cpf_cache,
        work_api_cache,
        meilisearch: std::sync::Arc::new(crate::meilisearch::MeilisearchCompanyService::new(
            &config.meilisearch_url,
            &config.meilisearch_key,
        )),
        fly_scale: std::sync::Arc::new(crate::fly_scale::FlyScaleService::new(&config)),
        alert_service: alert_service.clone(),
        enrichment_monitor: std::sync::Arc::new(crate::enrichment_monitor::EnrichmentMonitor::new(
            db.pool.clone(),
            alert_service.clone(),
        )),
        sessions,
    });

    // Configure rate limiter: 10 requests/second per IP, burst of 20
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(10)
            .burst_size(20)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    // Build protected routes with security layers
    let protected_routes = Router::new()
        // API Documentation
        .route("/docs", get(serve_swagger_ui))
        .route("/api-docs/openapi.yml", get(serve_openapi_spec))
        // API endpoints
        .route("/api/v1/leads", post(handlers::process_lead))
        .route("/api/v1/contributor/customer", get(handlers::get_customer))
        .route("/api/v1/customers/:id", get(handlers::get_customer_by_id))
        .route("/api/v1/enrich", post(handlers::enrich_customer))
        // Work API module endpoints
        .route("/api/v1/work/modules/all", get(handlers::fetch_all_modules))
        .route("/api/v1/work/modules/:module", get(handlers::fetch_module))
        // C2S integration endpoints
        .route(
            "/api/v1/c2s/enrich/:lead_id",
            post(handlers::c2s_enrich_lead),
        )
        .route(
            "/api/v1/leads/process",
            get(handlers::trigger_lead_processing),
        )
        // C2S webhook endpoint (replaces Make.com)
        .route("/api/v1/webhooks/c2s", post(webhook_handler::c2s_webhook))
        // Google Ads webhook endpoint (direct lead creation with inline enrichment)
        .route(
            "/api/v1/webhooks/google-ads",
            post(google_ads_handler::google_ads_webhook_handler),
        )
        // Batch enrichment endpoints
        // Company Intelligence (Meilisearch 65M)
        .route("/api/v1/company/cpf/:cpf", get(handlers::get_companies_by_cpf))
        .route("/api/v1/company/cnpj/:cnpj", get(handlers::get_company_by_cnpj))
        .route("/api/v1/company/search", get(handlers::search_companies))
        .route("/batch/enrich-direct", post(batch::enrich_direct))
        .route("/batch/retry-failed", post(batch::retry_failed))
        // Dashboard routes
        .route("/dashboard", get(dashboard::dashboard_page))
        .route("/dashboard/login", get(dashboard::login_page))
        .route("/dashboard/login", post(dashboard::login_submit))
        .route("/dashboard/logout", get(dashboard::logout))
        // Stats routes
        .route("/stats/enrichment", get(enrichment_stats_handler))
        .route("/stats/health", get(service_health_handler))
        // Lead analysis endpoint
        .route("/api/v1/analyze/:lead_id", post(analyze_lead_handler))
        .route("/api/v1/analysis/:lead_id", get(get_analysis_handler))
        .layer(axum_middleware::from_fn(api_auth::api_key_auth))
        .layer(
            ServiceBuilder::new()
                // Request size limit: 5MB max payload (prevents memory exhaustion)
                .layer(RequestBodyLimitLayer::new(5 * 1024 * 1024))
                // Rate limiting: 10 requests/second per IP, burst of 20
                .layer(GovernorLayer {
                    config: governor_conf,
                }),
        );

    // Clone app_state for cron and monitor before router consumes it
    let cron_state = app_state.clone();
    let cron_enabled = config.cron_enabled;
    let monitor_ref = app_state.enrichment_monitor.clone();

    // Build final app with health check (bypasses rate limiting for Fly.io)
    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/metrics", get(serve_metrics))
        .merge(protected_routes)
        .with_state(app_state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    // Initialize Prometheus metrics
    obs::metrics::init();

    // Start enrichment cron if enabled
    if cron_enabled {
        tokio::spawn(cron::start_enrichment_cron(cron_state));
        tracing::info!("Enrichment cron enabled (business: {}s, evening: {}s, night: {}s)",
            config.cron_interval_business_secs,
            config.cron_interval_evening_secs,
            config.cron_interval_night_secs);
    } else {
        tracing::info!("Enrichment cron disabled (set ENABLE_CRON=true to enable)");
    }

    // Start enrichment rate monitor (periodic check every 6 hours)
    tokio::spawn(async move { monitor_ref.start_monitoring().await });
    tracing::info!("Enrichment rate monitor started (threshold: 80%, interval: 6h)");

    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// GET /metrics — Prometheus metrics endpoint
async fn serve_metrics() -> impl IntoResponse {
    let body = obs::metrics::render();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

/// GET /stats/enrichment — enrichment rate stats
async fn enrichment_stats_handler(
    State(state): State<Arc<handlers::AppState>>,
) -> impl IntoResponse {
    let stats = state.enrichment_monitor.get_stats().await;
    (StatusCode::OK, axum::Json(serde_json::json!(stats)))
}

/// GET /stats/health — service health report
async fn service_health_handler(
    State(state): State<Arc<handlers::AppState>>,
) -> impl IntoResponse {
    let health = state.alert_service.get_service_health().await;
    let status = if health.all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, axum::Json(serde_json::json!(health)))
}

/// POST /api/v1/analyze/:lead_id — analyze a lead
async fn analyze_lead_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
) -> impl IntoResponse {
    let svc = lead_analysis::LeadAnalysisService::new(state.db.clone());
    let input = lead_analysis::LeadAnalysisInput {
        lead_id,
        name: "".to_string(), // Will be filled from DB lookup
        email: None,
        phone: None,
        cpf: None,
        income: None,
    };
    let result = svc.analyze(&input).await;
    (StatusCode::OK, axum::Json(serde_json::json!(result)))
}

/// GET /api/v1/analysis/:lead_id — get cached analysis
async fn get_analysis_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
) -> impl IntoResponse {
    let svc = lead_analysis::LeadAnalysisService::new(state.db.clone());
    match svc.get_cached(&lead_id).await {
        Some(result) => (StatusCode::OK, axum::Json(serde_json::json!(result))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "No analysis found for this lead"})),
        ).into_response(),
    }
}
