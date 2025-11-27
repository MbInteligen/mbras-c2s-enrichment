use sqlx::{postgres::PgPoolOptions, PgPool};

/// Database connection pool wrapper with resilience features.
///
/// Features:
/// - Connection pooling (max 10 connections).
/// - Automatic reconnection on connection failure.
///
/// # Circuit Breaker Usage
///
/// For critical operations, wrap database calls with the circuit breaker
/// from `crate::circuit_breaker::create_db_circuit_breaker()`:
///
/// ```rust
/// use crate::circuit_breaker::create_db_circuit_breaker;
/// // let cb = create_db_circuit_breaker();
/// // let result = cb.call(async {
/// //     sqlx::query("SELECT * FROM users").fetch_all(&pool).await
/// // }).await;
/// ```
///
/// The circuit breaker protects against cascading failures by:
/// - Opening after 5 consecutive failures.
/// - Failing fast when open (prevents overwhelming unhealthy DB).
/// - Automatically testing recovery with exponential backoff (10s to 60s).
pub struct Database {
    /// The underlying `sqlx::PgPool`.
    pub pool: PgPool,
}

impl Database {
    /// Creates a new database connection pool.
    ///
    /// # Arguments
    ///
    /// * `database_url` - The connection string for the PostgreSQL database.
    ///
    /// # Returns
    ///
    /// * `anyhow::Result<Self>` - The `Database` instance or an error if connection fails.
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        // Run migrations if needed
        sqlx::query("SELECT 1").execute(&pool).await?;

        Ok(Self { pool })
    }
}
