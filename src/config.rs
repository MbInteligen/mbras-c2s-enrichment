use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub c2s_token: String,
    pub c2s_base_url: String,
    pub c2s_gateway_url: Option<String>, // Optional for backward compatibility
    pub worker_api_key: String,
    pub diretrix_base_url: String,
    pub diretrix_user: String,
    pub diretrix_pass: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let config = Self {
            database_url: std::env::var("DB_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .map_err(|_| {
                    anyhow::anyhow!("DB_URL or DATABASE_URL environment variable required")
                })
                .and_then(|url| {
                    if url.trim().is_empty() {
                        anyhow::bail!("DB_URL cannot be empty");
                    }
                    if !url.starts_with("postgresql://") && !url.starts_with("postgres://") {
                        anyhow::bail!("DB_URL must start with postgresql:// or postgres://");
                    }
                    Ok(url)
                })?,
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| anyhow::anyhow!("PORT must be a valid number between 1-65535"))?,
            c2s_token: std::env::var("C2S_TOKEN")
                .map_err(|_| anyhow::anyhow!("C2S_TOKEN environment variable required"))
                .and_then(|token| {
                    if token.trim().is_empty() {
                        anyhow::bail!("C2S_TOKEN cannot be empty");
                    }
                    Ok(token)
                })?,
            c2s_gateway_url: std::env::var("C2S_GATEWAY_URL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            c2s_base_url: std::env::var("C2S_BASE_URL")
                .map_err(|_| anyhow::anyhow!("C2S_BASE_URL environment variable required"))
                .and_then(|url| {
                    if url.trim().is_empty() {
                        anyhow::bail!("C2S_BASE_URL cannot be empty");
                    }
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        anyhow::bail!("C2S_BASE_URL must start with http:// or https://");
                    }
                    Ok(url)
                })?,
            worker_api_key: std::env::var("WORK_API")
                .or_else(|_| std::env::var("WORKER_API_KEY"))
                .map_err(|_| {
                    anyhow::anyhow!("WORK_API or WORKER_API_KEY environment variable required")
                })
                .and_then(|key| {
                    if key.trim().is_empty() {
                        anyhow::bail!("WORK_API cannot be empty");
                    }
                    Ok(key)
                })?,
            diretrix_base_url: std::env::var("DIRETRIX_BASE_URL")
                .map_err(|_| anyhow::anyhow!("DIRETRIX_BASE_URL environment variable required"))
                .and_then(|url| {
                    if url.trim().is_empty() {
                        anyhow::bail!("DIRETRIX_BASE_URL cannot be empty");
                    }
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        anyhow::bail!("DIRETRIX_BASE_URL must start with http:// or https://");
                    }
                    Ok(url)
                })?,
            diretrix_user: std::env::var("DIRETRIX_USER")
                .map_err(|_| anyhow::anyhow!("DIRETRIX_USER environment variable required"))
                .and_then(|user| {
                    if user.trim().is_empty() {
                        anyhow::bail!("DIRETRIX_USER cannot be empty");
                    }
                    Ok(user)
                })?,
            diretrix_pass: std::env::var("DIRETRIX_PASS")
                .map_err(|_| anyhow::anyhow!("DIRETRIX_PASS environment variable required"))
                .and_then(|pass| {
                    if pass.trim().is_empty() {
                        anyhow::bail!("DIRETRIX_PASS cannot be empty");
                    }
                    Ok(pass)
                })?,
        };

        // Log successful configuration load (without sensitive values)
        tracing::info!("Configuration loaded successfully");
        tracing::debug!(
            "Database URL: {}...",
            &config.database_url[..20.min(config.database_url.len())]
        );
        tracing::debug!("C2S Base URL: {}", config.c2s_base_url);
        if let Some(ref gateway) = config.c2s_gateway_url {
            tracing::info!("C2S Gateway URL configured: {}", gateway);
        }
        tracing::debug!("Diretrix Base URL: {}", config.diretrix_base_url);
        tracing::debug!("Server Port: {}", config.port);

        Ok(config)
    }
}
