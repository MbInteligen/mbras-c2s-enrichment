//! MCP Server binary — stdio transport for AI assistant integration.
//! Run: cargo run --bin mcp-server

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use rust_c2s_api::config::Config;
use rust_c2s_api::mcp::McpServer;
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
    let server = McpServer::new(config);

    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP server error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
