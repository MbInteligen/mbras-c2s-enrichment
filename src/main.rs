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

use axum::{
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

/// Serves the OpenAPI specification YAML file.
///
/// This endpoint reads the `openapi.yml` file from the filesystem and serves it
/// with the appropriate content type. If the file is not found, it returns a 404 error.
///
/// # Returns
///
/// * `impl IntoResponse` - The HTTP response containing the OpenAPI YAML content or an error message.
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

/// Serves the Swagger UI HTML page.
///
/// This endpoint returns an HTML page that embeds the Swagger UI, configured to
/// load the OpenAPI specification served by `serve_openapi_spec`.
///
/// # Returns
///
/// * `impl IntoResponse` - The HTTP response containing the Swagger UI HTML.
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

/// Main entry point for the application.
///
/// This function initializes the application, including:
/// - Logging and tracing.
/// - Configuration loading.
/// - Database connection.
/// - Caches (CPF, Lead, Contact, Work API).
/// - External API clients.
/// - HTTP routes and middleware (CORS, Rate Limiting).
///
/// It then starts the Axum server.
///
/// # Returns
///
/// * `anyhow::Result<()>` - Ok if the server runs successfully, or an error if initialization fails.
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

    // Build application state
    let app_state = std::sync::Arc::new(crate::handlers::AppState {
        db: db.pool.clone(),
        config: config.clone(),
        gateway_client,
        recent_cpf_cache,
        processing_leads_cache,
        contact_to_cpf_cache,
        work_api_cache,
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
        .layer(
            ServiceBuilder::new()
                // Request size limit: 5MB max payload (prevents memory exhaustion)
                .layer(RequestBodyLimitLayer::new(5 * 1024 * 1024))
                // Rate limiting: 10 req/sec per IP, burst of 20 (prevents DDoS)
                .layer(GovernorLayer {
                    config: governor_conf,
                }),
        );

    // Build final app with health check (bypasses rate limiting for Fly.io)
    let app = Router::new()
        .route("/health", get(handlers::health))
        .merge(protected_routes)
        .with_state(app_state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
