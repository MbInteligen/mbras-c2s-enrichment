//! MCP Server binary — stdio transport for AI assistant integration.
//! Run: cargo run --bin mcp_server

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use rust_c2s_api::config::Config;
use rust_c2s_api::db::Database;
use rust_c2s_api::mcp::{McpAppState, McpServer};
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Log to stderr (stdout is used for MCP stdio transport)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting C2S MCP server (66 tools, 3 resources)");

    let config = Config::from_env()?;

    // Connect to database and create fully-wired server
    let db = Database::new(&config.database_url).await?;
    tracing::info!("Database connection pool established");

    let state = McpAppState::new(&config, db.pool.clone());
    let server = McpServer::with_state(config, state);
    tracing::info!("MCP server wired with DB + services (10 live tools, 42 stubs)");

    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP server error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
