use sqlx::{postgres::PgPoolOptions, PgPool};

pub struct Database {
    pub pool: PgPool,
}

impl Database {
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
