mod ai_generate;
mod ai_interpret;
mod alert;
mod api_auth;
mod batch;
mod c2s_extended;
mod cache_validator;
mod circuit_breaker;
mod cnpj_fallback;
mod config;
mod context;
mod cpf;
mod cron;
mod dashboard;
mod db;
mod db_storage;
mod discovery;
mod domain_analyzer;
mod enrichment;
mod enrichment_monitor;
mod errors;
mod fly_scale;
mod gateway_client;
mod google_ads_handler;
mod google_ads_models;
mod handlers;
mod ibvi_property;
mod lead_analysis;
mod mcp;
mod meilisearch;
mod models;
mod name_matcher;
mod obs;
mod photo_storage;
mod report;
mod retry;
mod risk_detector;
mod scoring;
mod services;
mod twenty;
mod web_search;
mod webhook_handler;
mod webhook_models;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware as axum_middleware,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use moka::future::Cache;
use std::net::SocketAddr;
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
        .route(
            "/api/v1/contributor/search",
            get(handlers::search_customers),
        )
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
        .route(
            "/api/v1/company/cpf/:cpf",
            get(handlers::get_companies_by_cpf),
        )
        .route(
            "/api/v1/company/cnpj/:cnpj",
            get(handlers::get_company_by_cnpj),
        )
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
        // Property Intelligence endpoint
        .route("/api/v1/property/cpf/:cpf", get(property_by_cpf_handler))
        // Lead analysis endpoint
        .route("/api/v1/analyze/:lead_id", post(analyze_lead_handler))
        .route("/api/v1/analysis/:lead_id", get(get_analysis_handler))
        // AI natural language interpreter (proxy to OpenRouter)
        .route("/api/v1/ai/interpret", post(ai_interpret::ai_interpret))
        .route("/api/v1/ai/models", get(ai_interpret::ai_models))
        .route("/api/v1/ai/generate", post(ai_generate::ai_generate))
        // Unified context endpoint (parallel fan-out to Work API + Meilisearch + IBVI)
        .route("/api/v1/context", get(context::get_context))
        // Report generation endpoints
        .route("/reports/markdown", post(generate_markdown_report_handler))
        .route("/reports/html", post(generate_html_report_handler))
        .route("/reports/pdf", post(generate_pdf_report_handler))
        .route(
            "/reports/from-cpfs",
            post(generate_report_from_cpfs_handler),
        )
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
        tracing::info!(
            "Enrichment cron enabled (business: {}s, evening: {}s, night: {}s)",
            config.cron_interval_business_secs,
            config.cron_interval_evening_secs,
            config.cron_interval_night_secs
        );
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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// GET /metrics — Prometheus metrics endpoint
async fn serve_metrics() -> impl IntoResponse {
    let body = obs::metrics::render();
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
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
async fn service_health_handler(State(state): State<Arc<handlers::AppState>>) -> impl IntoResponse {
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
        )
            .into_response(),
    }
}

/// GET /api/v1/property/cpf/:cpf — property portfolio by CPF
async fn property_by_cpf_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(cpf): Path<String>,
) -> impl IntoResponse {
    let svc = ibvi_property::IbviPropertyService::new(state.db.clone());
    match svc.find_properties_by_cpf(&cpf).await {
        Some(summary) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "success": true,
                "cpf": cpf,
                "summary": summary,
                "message": ibvi_property::IbviPropertyService::format_for_message(&summary),
            })),
        )
            .into_response(),
        None => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "success": true,
                "cpf": cpf,
                "summary": null,
                "message": "No properties found",
            })),
        )
            .into_response(),
    }
}

// ─── Phase 7: C2S Extended Handlers ─────────────────────────────────────────

async fn list_sellers_handler(
    State(state): State<Arc<handlers::AppState>>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.list_sellers().await {
        Ok(sellers) => Ok(axum::Json(serde_json::json!({ "data": sellers }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn get_seller_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.get_seller(&id).await {
        Ok(Some(seller)) => Ok(axum::Json(serde_json::json!({ "data": seller }))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Seller not found".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn create_seller_handler(
    State(state): State<Arc<handlers::AppState>>,
    axum::Json(input): axum::Json<c2s_extended::SellerCreateInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.create_seller(&input).await {
        Ok(seller) => Ok(axum::Json(serde_json::json!({ "data": seller }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn update_seller_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(id): Path<String>,
    axum::Json(input): axum::Json<c2s_extended::SellerUpdateInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.update_seller(&id, &input).await {
        Ok(seller) => Ok(axum::Json(serde_json::json!({ "data": seller }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn list_tags_handler(
    State(state): State<Arc<handlers::AppState>>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.list_tags().await {
        Ok(tags) => Ok(axum::Json(serde_json::json!({ "data": tags }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn create_tag_handler(
    State(state): State<Arc<handlers::AppState>>,
    axum::Json(input): axum::Json<c2s_extended::TagCreateInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.create_tag(&input).await {
        Ok(tag) => Ok(axum::Json(serde_json::json!({ "data": tag }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn get_lead_tags_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.get_lead_tags(&lead_id).await {
        Ok(tags) => Ok(axum::Json(serde_json::json!({ "data": tags }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn add_tag_to_lead_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let tag_id = body
        .get("tag_id")
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "Missing tag_id".to_string()))?;
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.add_tag_to_lead(&lead_id, tag_id).await {
        Ok(_) => Ok(axum::Json(serde_json::json!({ "success": true }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn register_activity_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
    axum::Json(input): axum::Json<c2s_extended::ActivityInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.register_activity(&lead_id, &input).await {
        Ok(result) => Ok(axum::Json(result)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn add_note_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let note = body
        .get("body")
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "Missing body".to_string()))?;
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.add_note(&lead_id, note).await {
        Ok(_) => Ok(axum::Json(serde_json::json!({ "success": true }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn forward_lead_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
    axum::Json(input): axum::Json<c2s_extended::ForwardInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.forward_lead(&lead_id, &input).await {
        Ok(_) => Ok(axum::Json(serde_json::json!({ "success": true }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn search_by_phone_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(phone): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.search_by_phone(&phone).await {
        Ok(results) => Ok(axum::Json(serde_json::json!({ "data": results }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn search_by_email_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(email): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.search_by_email(&email).await {
        Ok(results) => Ok(axum::Json(serde_json::json!({ "data": results }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn enrichment_status_handler(
    State(state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    match c2s_extended::C2sExtendedService::get_enrichment_status(&state.db, &lead_id).await {
        Ok(Some(record)) => Ok(axum::Json(serde_json::json!({ "data": record }))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Lead not found".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn distribute_leads_handler(
    State(state): State<Arc<handlers::AppState>>,
    axum::Json(input): axum::Json<c2s_extended::QueueDistributeInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.distribute_leads(&input).await {
        Ok(result) => Ok(axum::Json(result)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn auto_assign_handler(
    State(state): State<Arc<handlers::AppState>>,
    axum::Json(input): axum::Json<c2s_extended::QueueAutoAssignInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc =
        c2s_extended::C2sExtendedService::new(&state.config.c2s_base_url, &state.config.c2s_token);
    match svc.auto_assign(&input).await {
        Ok(result) => Ok(axum::Json(result)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

// ─── Phase 8: Twenty CRM Handlers ──────────────────────────────────────────

async fn twenty_create_lead_handler(
    State(_state): State<Arc<handlers::AppState>>,
    axum::Json(input): axum::Json<twenty::TwentyLeadInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    match svc.create_lead(&input).await {
        Ok(lead) => Ok(axum::Json(serde_json::json!({ "data": lead }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn twenty_get_lead_handler(
    State(_state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    match svc.find_lead(&lead_id).await {
        Ok(Some(lead)) => Ok(axum::Json(serde_json::json!({ "data": lead }))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Lead not found".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn twenty_delegate_handler(
    State(_state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    let tier = body.get("tier").and_then(|v| v.as_str()).unwrap_or("C");
    let reason: twenty::DelegationReason = serde_json::from_value(
        body.get("reason")
            .cloned()
            .unwrap_or(serde_json::json!("workload")),
    )
    .unwrap_or(twenty::DelegationReason::Workload);

    let input = twenty::DelegateInput {
        lead_id: lead_id.clone(),
        to_workspace: twenty::Workspace::WsGeneral,
        reason,
        delegated_by: body
            .get("delegated_by")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    let delegation = svc.create_delegation_with_tier(&input, tier);
    Ok(axum::Json(serde_json::json!({ "data": delegation })))
}

async fn twenty_sla_handler(
    State(_state): State<Arc<handlers::AppState>>,
    Path(lead_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    let tier = params.get("tier").map(|s| s.as_str()).unwrap_or("C");
    let created_at = params
        .get("created_at")
        .ok_or((StatusCode::BAD_REQUEST, "Missing created_at".to_string()))?;

    match svc.check_sla(tier, created_at) {
        Ok(check) => Ok(axum::Json(serde_json::json!({ "data": check }))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

async fn twenty_next_action_handler(
    State(_state): State<Arc<handlers::AppState>>,
    Path(_lead_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    let status = params.get("status").map(|s| s.as_str()).unwrap_or("novo");
    let tier = params.get("tier").map(|s| s.as_str()).unwrap_or("C");
    let action = svc.get_next_action(status, tier);
    Ok(axum::Json(serde_json::json!({ "data": action })))
}

async fn twenty_intent_signal_handler(
    State(_state): State<Arc<handlers::AppState>>,
    axum::Json(input): axum::Json<twenty::IntentSignalInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    let signal = svc.calculate_intent_signal(&input);
    Ok(axum::Json(serde_json::json!({ "signal": signal })))
}

async fn twenty_pipeline_stats_handler(
    State(_state): State<Arc<handlers::AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    let ws = match params.get("workspace").map(|s| s.as_str()) {
        Some("senior") => twenty::Workspace::WsSenior,
        Some("ops") => twenty::Workspace::WsOps,
        _ => twenty::Workspace::WsGeneral,
    };
    match svc.get_pipeline_stats(ws).await {
        Ok(stats) => Ok(axum::Json(serde_json::json!({ "data": stats }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn twenty_broker_stats_handler(
    State(_state): State<Arc<handlers::AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    let ws = match params.get("workspace").map(|s| s.as_str()) {
        Some("senior") => twenty::Workspace::WsSenior,
        Some("ops") => twenty::Workspace::WsOps,
        _ => twenty::Workspace::WsGeneral,
    };
    match svc.get_broker_stats(ws).await {
        Ok(stats) => Ok(axum::Json(serde_json::json!({ "data": stats }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn twenty_sla_violations_handler(
    State(_state): State<Arc<handlers::AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    let ws = match params.get("workspace").map(|s| s.as_str()) {
        Some("senior") => twenty::Workspace::WsSenior,
        Some("ops") => twenty::Workspace::WsOps,
        _ => twenty::Workspace::WsGeneral,
    };
    match svc.check_sla_violations(ws).await {
        Ok(violations) => Ok(axum::Json(serde_json::json!({ "data": violations }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn twenty_bulk_import_handler(
    State(_state): State<Arc<handlers::AppState>>,
    axum::Json(input): axum::Json<twenty::BulkImportInput>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let svc = twenty_service_from_config(&_state.config);
    match svc.bulk_import(&input).await {
        Ok(result) => Ok(axum::Json(serde_json::json!({ "data": result }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

// ─── Phase 9: Report Handlers ───────────────────────────────────────────────

async fn generate_markdown_report_handler(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let persons: Vec<report::ReportPerson> = serde_json::from_value(
        body.get("persons")
            .cloned()
            .unwrap_or(serde_json::json!([])),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid persons: {}", e)))?;

    let options: report::ReportOptions = serde_json::from_value(
        body.get("options")
            .cloned()
            .unwrap_or(serde_json::json!({"title": "Report"})),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid options: {}", e)))?;

    let svc = report::ProfileReportService::new();
    let result = svc.generate_markdown(&persons, &options);
    Ok(axum::Json(serde_json::json!(result)))
}

async fn generate_html_report_handler(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let persons: Vec<report::ReportPerson> = serde_json::from_value(
        body.get("persons")
            .cloned()
            .unwrap_or(serde_json::json!([])),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid persons: {}", e)))?;

    let options: report::ReportOptions = serde_json::from_value(
        body.get("options")
            .cloned()
            .unwrap_or(serde_json::json!({"title": "Report"})),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid options: {}", e)))?;

    let svc = report::ProfileReportService::new();
    let result = svc.generate_html(&persons, &options);
    Ok(axum::Json(serde_json::json!(result)))
}

async fn generate_pdf_report_handler(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let persons: Vec<report::ReportPerson> = serde_json::from_value(
        body.get("persons")
            .cloned()
            .unwrap_or(serde_json::json!([])),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid persons: {}", e)))?;

    let options: report::ReportOptions = serde_json::from_value(
        body.get("options")
            .cloned()
            .unwrap_or(serde_json::json!({"title": "Report"})),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid options: {}", e)))?;

    let svc = report::ProfileReportService::new();
    let result = svc.generate_pdf(&persons, &options).await;
    Ok(axum::Json(serde_json::json!(result)))
}

/// POST /reports/from-cpfs — Look up persons by CPF from DB and generate report
async fn generate_report_from_cpfs_handler(
    State(state): State<Arc<handlers::AppState>>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let cpfs: Vec<String> = body
        .get("cpfs")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if cpfs.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "cpfs array is required".to_string(),
        ));
    }

    let format = body
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("html");

    let title = body
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Lead Report — MBRAS");

    let subtitle = body
        .get("subtitle")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Look up each CPF from the database
    let mut persons = Vec::new();
    for cpf in &cpfs {
        let digits: String = cpf.chars().filter(|c| c.is_ascii_digit()).collect();

        // Fetch party
        let party = sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, Option<chrono::NaiveDate>, Option<String>, Option<String>)>(
            "SELECT id, full_name, cpf_cnpj, birth_date, sex, mother_name FROM core.parties WHERE cpf_cnpj = $1 AND party_type = 'person' LIMIT 1"
        )
        .bind(&digits)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

        let Some((party_id, name, _cpf_val, birth_date, gender, _mother)) = party else {
            continue;
        };

        // Fetch contacts
        let contacts: Vec<(String, String)> = sqlx::query_as(
            "SELECT contact_type::text, value FROM core.party_contacts WHERE party_id = $1",
        )
        .bind(party_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;

        let phones: Vec<String> = contacts
            .iter()
            .filter(|(t, _)| t == "phone" || t == "whatsapp")
            .map(|(_, v)| v.clone())
            .collect();

        let emails: Vec<String> = contacts
            .iter()
            .filter(|(t, _)| t == "email")
            .map(|(_, v)| v.clone())
            .collect();

        // Fetch income from latest enrichment
        let income: Option<f64> = sqlx::query_scalar(
            "SELECT (raw_payload->'DadosEconomicos'->>'renda')::float8 * 1.9 FROM core.party_enrichments WHERE party_id = $1 ORDER BY enriched_at DESC LIMIT 1"
        )
        .bind(party_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        persons.push(report::ReportPerson {
            cpf: digits,
            name,
            occupation: None,
            company: None,
            birth_date: birth_date.map(|d| d.format("%d/%m/%Y").to_string()),
            gender,
            income,
            phones,
            emails,
            address: None,
        });
    }

    if persons.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            "No persons found for the given CPFs".to_string(),
        ));
    }

    let options = report::ReportOptions {
        title: title.to_string(),
        subtitle,
        classification: "Confidencial - Uso Interno".to_string(),
        include_contacts: true,
        include_income: true,
        output_dir: None,
    };

    let svc = report::ProfileReportService::new();
    let result = match format {
        "pdf" => svc.generate_pdf(&persons, &options).await,
        "md" | "markdown" => svc.generate_markdown(&persons, &options),
        _ => svc.generate_html(&persons, &options),
    };

    Ok(axum::Json(serde_json::json!(result)))
}

// ─── Phase 10: Photo Handlers ───────────────────────────────────────────────

async fn upload_photo_handler(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let cpf = body
        .get("cpf")
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "Missing cpf".to_string()))?;
    let base64_data = body
        .get("photo")
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "Missing photo".to_string()))?;

    let r2_config = photo_storage_config_from_env();
    let svc = photo_storage::PhotoStorageService::new(r2_config);

    match svc.upload_and_get_url(cpf, base64_data).await {
        Some(url) => Ok(axum::Json(
            serde_json::json!({ "success": true, "url": url }),
        )),
        None => Ok(axum::Json(
            serde_json::json!({ "success": false, "error": "Upload failed or not configured" }),
        )),
    }
}

async fn photo_url_handler(
    Path(key): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let r2_config = photo_storage_config_from_env();
    let svc = photo_storage::PhotoStorageService::new(r2_config);

    match svc.get_photo_url(&key) {
        Some(url) => Ok(axum::Json(serde_json::json!({ "url": url }))),
        None => Err((
            StatusCode::NOT_FOUND,
            "Photo not found or R2 not configured".to_string(),
        )),
    }
}

fn photo_storage_config_from_env() -> Option<photo_storage::R2Config> {
    let ak = std::env::var("R2_ACCESS_KEY_ID").ok()?;
    let sk = std::env::var("R2_SECRET_ACCESS_KEY").ok()?;
    if ak.is_empty() || sk.is_empty() {
        return None;
    }
    Some(photo_storage::R2Config {
        access_key_id: ak,
        secret_access_key: sk,
        endpoint: std::env::var("R2_ENDPOINT")
            .unwrap_or_else(|_| "https://r2.cloudflarestorage.com".to_string()),
        bucket: std::env::var("R2_BUCKET").unwrap_or_else(|_| "photos".to_string()),
        signed_url_expiry_seconds: std::env::var("R2_SIGNED_URL_EXPIRY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(604_800),
    })
}

/// Helper to construct TwentyService from config
fn twenty_service_from_config(config: &config::Config) -> twenty::TwentyService {
    twenty::TwentyService::new(
        &config.twenty_base_url,
        &config.twenty_api_key,
        config.twenty_api_key_ws_ops.as_deref(),
        config.twenty_api_key_ws_senior.as_deref(),
        config.twenty_api_key_ws_general.as_deref(),
        config.twenty_enabled,
    )
}
