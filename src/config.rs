use serde::Deserialize;
use url::Url;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub c2s_token: String,
    pub c2s_base_url: String,
    pub webhook_secret: Option<String>, // Optional webhook secret for C2S webhooks
    pub worker_api_key: String,
    pub diretrix_base_url: String,
    pub diretrix_user: String,
    pub diretrix_pass: String,

    // DBase API integration (primary phone lookup)
    pub dbase_key: String,

    // Mimir API integration (Azure IBVI - fallback for DBase)
    pub mimir_token: Option<String>, // DEPRECATED: Mimir being replaced by Work API discovery

    // Google Ads integration (optional - only required if using Google Ads webhooks)
    pub google_ads_webhook_key: Option<String>, // Webhook verification key
    pub c2s_default_seller_id: Option<String>,  // Default seller for new leads
    pub c2s_description_max_length: usize,      // Max description length

    // CPF Lookup API (DuckDB 223M records - Tier 3 fallback)
    pub cpf_lookup_api_url: String,
    pub cpf_lookup_timeout_ms: u64,

    // Income display multiplier (raw income * multiplier)
    pub income_multiplier: f64,

    // Enrichment cron intervals (seconds)
    pub cron_interval_business_secs: u64,   // Business hours (9-18)
    pub cron_interval_evening_secs: u64,    // Evening (18-23)
    pub cron_interval_night_secs: u64,      // Night (23-9)
    pub cron_enabled: bool,

    // Meilisearch (65M companies)
    pub meilisearch_url: String,
    pub meilisearch_key: String,
    pub meilisearch_auto_scale: bool,
    pub meilisearch_app_name: String,
    pub meilisearch_machine_id: Option<String>,
    pub meilisearch_fly_api_token: Option<String>,

    // Twenty CRM integration
    pub twenty_base_url: String,
    pub twenty_api_key: String,
    pub twenty_api_key_ws_ops: Option<String>,
    pub twenty_api_key_ws_senior: Option<String>,
    pub twenty_api_key_ws_general: Option<String>,
    pub twenty_enabled: bool,
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
            port: {
                let port: u16 = std::env::var("PORT")
                    .unwrap_or_else(|_| "3000".to_string())
                    .parse()
                    .map_err(|_| anyhow::anyhow!("PORT must be a valid number between 1-65535"))?;

                if port == 0 {
                    anyhow::bail!("PORT must be greater than 0");
                }

                port
            },
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
            dbase_key: std::env::var("DBASE_KEY")
                .map_err(|_| anyhow::anyhow!("DBASE_KEY environment variable required"))
                .and_then(|key| {
                    if key.trim().is_empty() {
                        anyhow::bail!("DBASE_KEY cannot be empty");
                    }
                    Ok(key)
                })?,
            mimir_token: std::env::var("MIMIR_TOKEN")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            cpf_lookup_api_url: std::env::var("CPF_LOOKUP_API_URL")
                .unwrap_or_else(|_| "https://cpf-lookup-api.fly.dev".to_string()),
            cpf_lookup_timeout_ms: std::env::var("CPF_LOOKUP_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(120_000), // 2 minutes default
            income_multiplier: std::env::var("INCOME_MULTIPLIER")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.9),
            cron_interval_business_secs: std::env::var("CRON_INTERVAL_BUSINESS_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300), // 5 minutes
            cron_interval_evening_secs: std::env::var("CRON_INTERVAL_EVENING_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1200), // 20 minutes
            cron_interval_night_secs: std::env::var("CRON_INTERVAL_NIGHT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600), // 60 minutes
            cron_enabled: std::env::var("ENABLE_CRON")
                .ok()
                .map(|s| s == "true" || s == "1")
                .unwrap_or(false),
            meilisearch_url: std::env::var("MEILISEARCH_URL")
                .unwrap_or_else(|_| "https://ibvi-meilisearch-v2.fly.dev".to_string()),
            meilisearch_key: std::env::var("MEILISEARCH_KEY")
                .unwrap_or_default(),
            meilisearch_auto_scale: std::env::var("MEILISEARCH_AUTO_SCALE")
                .ok()
                .map(|s| s == "true" || s == "1")
                .unwrap_or(false),
            meilisearch_app_name: std::env::var("MEILISEARCH_APP_NAME")
                .unwrap_or_else(|_| "ibvi-meilisearch-v2".to_string()),
            meilisearch_machine_id: std::env::var("MEILISEARCH_MACHINE_ID")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            meilisearch_fly_api_token: std::env::var("MEILISEARCH_FLY_API_TOKEN")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            google_ads_webhook_key: std::env::var("GOOGLE_ADS_WEBHOOK_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            c2s_default_seller_id: std::env::var("C2S_DEFAULT_SELLER_ID")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            twenty_base_url: std::env::var("TWENTY_BASE_URL")
                .unwrap_or_else(|_| "https://twenty-server-production-1c77.up.railway.app".to_string()),
            twenty_api_key: std::env::var("TWENTY_API_KEY")
                .unwrap_or_default(),
            twenty_api_key_ws_ops: std::env::var("TWENTY_API_KEY_WS_OPS")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            twenty_api_key_ws_senior: std::env::var("TWENTY_API_KEY_WS_SENIOR")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            twenty_api_key_ws_general: std::env::var("TWENTY_API_KEY_WS_GENERAL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            twenty_enabled: std::env::var("TWENTY_ENABLED")
                .ok()
                .map(|s| s == "true" || s == "1")
                .unwrap_or(false),
            c2s_description_max_length: {
                let max_len = std::env::var("C2S_DESCRIPTION_MAX_LENGTH")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5000);

                if max_len == 0 {
                    anyhow::bail!("C2S_DESCRIPTION_MAX_LENGTH must be greater than 0");
                }

                max_len
            },
        };

        // Log successful configuration load (without sensitive values)
        tracing::info!("Configuration loaded successfully");
        // Redact DB URL credentials while keeping target info
        if let Ok(db_url) = Url::parse(&config.database_url) {
            let host = db_url.host_str().unwrap_or("unknown");
            let port = db_url
                .port_or_known_default()
                .map(|p| format!(":{}", p))
                .unwrap_or_default();
            let path = db_url.path();
            tracing::debug!(
                "Database URL (redacted): {}://{}{}{}",
                db_url.scheme(),
                host,
                port,
                path
            );
        } else {
            tracing::debug!("Database URL (redacted): <unparsable>");
        }
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
