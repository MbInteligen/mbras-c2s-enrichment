//! Script to cleanup empty parties from the database.

use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;

/// Main entry point for the cleanup script.
///
/// Connects to the database and deletes parties that have no corresponding 'people' or 'companies' records.
/// Includes a safety check to avoid deleting recently created parties.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Database connection
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    tracing::info!("Connected to database. Starting cleanup of empty parties...");

    // Define "empty party": A party record that has NO corresponding record in the 'people' table.
    // We also add a safety buffer (e.g., created more than 1 hour ago) to avoid deleting
    // parties that are currently being created by a running process (though the transaction fix should prevent this).

    let query = r#"
        DELETE FROM core.parties p
        WHERE NOT EXISTS (
            SELECT 1 FROM core.people pp WHERE pp.party_id = p.id
        )
        AND NOT EXISTS (
            SELECT 1 FROM core.companies c WHERE c.party_id = p.id
        )
        AND p.created_at < NOW() - INTERVAL '1 hour'
    "#;

    let result = sqlx::query(query).execute(&pool).await?;

    tracing::info!(
        "Cleanup complete. Deleted {} empty parties.",
        result.rows_affected()
    );

    Ok(())
}
