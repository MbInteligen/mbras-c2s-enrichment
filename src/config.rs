use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub c2s_token: String,
    pub c2s_base_url: String,
    pub webhook_secret: Option<String>,  // Optional webhook secret for C2S webhooks
    pub worker_api_key: String,
    pub diretrix_base_url: String,
    pub diretrix_user: String,
    pub diretrix_pass: String,

    // Google Ads integration (optional - only required if using Google Ads webhooks)
    pub google_ads_webhook_key: Option<String>, // Webhook verification key
    pub c2s_default_seller_id: Option<String>,  // Default seller for new leads
    pub c2s_description_max_length: usize,      // Max description length
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
            webhook_secret: std::env::var("WEBHOOK_SECRET")
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
            google_ads_webhook_key: std::env::var("GOOGLE_ADS_WEBHOOK_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            c2s_default_seller_id: std::env::var("C2S_DEFAULT_SELLER_ID")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            c2s_description_max_length: std::env::var("C2S_DESCRIPTION_MAX_LENGTH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5000), // Default to 5000 chars
        };

        // Log successful configuration load (without sensitive values)
        tracing::info!("Configuration loaded successfully");
        tracing::debug!(
            "Database URL: {}...",
            &config.database_url[..20.min(config.database_url.len())]
        );
        tracing::debug!("C2S Base URL: {}", config.c2s_base_url);
        if config.webhook_secret.is_some() {
            tracing::info!("Webhook secret configured for C2S webhooks");
        } else {
            tracing::warn!(
                "No webhook secret configured - C2S webhooks will not validate authentication"
            );
        }
        tracing::debug!("Diretrix Base URL: {}", config.diretrix_base_url);
        tracing::debug!("Server Port: {}", config.port);

        // Google Ads configuration
        if config.google_ads_webhook_key.is_some() {
            tracing::info!("Google Ads webhook key configured");
            if let Some(ref seller_id) = config.c2s_default_seller_id {
                tracing::info!("C2S default seller ID: {}", seller_id);
            } else {
                tracing::warn!(
                    "C2S_DEFAULT_SELLER_ID not set - Google Ads leads will have no seller assigned"
                );
            }
        } else {
            tracing::warn!(
                "GOOGLE_ADS_WEBHOOK_KEY not configured - Google Ads webhooks will be rejected"
            );
        }
        tracing::info!(
            "C2S description max length: {} chars",
            config.c2s_description_max_length
        );

        Ok(config)
    }
}
