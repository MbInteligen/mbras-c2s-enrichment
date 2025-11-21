mod config;
mod db;
mod db_storage;
mod enrichment;
mod errors;
mod gateway_client;
mod handlers;
mod models;
mod services;
mod webhook_handler;
mod webhook_models;

use axum::{
    routing::{get, post},
    Router,
};
use moka::future::Cache;
use std::time::Duration;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::db::Database;

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

    // Initialize gateway client if URL is configured
    let gateway_client = if let Some(ref gateway_url) = config.c2s_gateway_url {
        match gateway_client::C2sGatewayClient::new(gateway_url.clone()) {
            Ok(client) => {
                tracing::info!("✓ C2S Gateway client initialized: {}", gateway_url);
                Some(client)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize gateway client: {}. Will use direct C2S.",
                    e
                );
                None
            }
        }
    } else {
        tracing::info!("C2S Gateway URL not configured, using direct C2S calls");
        None
    };

    // Build application state
    let app_state = std::sync::Arc::new(crate::handlers::AppState {
        db: db.pool.clone(),
        config: config.clone(),
        gateway_client,
        recent_cpf_cache,
        processing_leads_cache,
    });

    // Build router
    let app = Router::new()
        .route("/health", get(handlers::health))
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
        // Temporary test endpoint for C2S Gateway integration
        .route("/test-gateway", get(handlers::test_gateway))
        .with_state(app_state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
