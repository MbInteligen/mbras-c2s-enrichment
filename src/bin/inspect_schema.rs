//! Utility to inspect the database schema and print table structures.

use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;

/// Main entry point for the schema inspection utility.
///
/// Connects to the database and lists columns for tables matching specific criteria.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new().connect(&database_url).await?;

    // List all tables starting with 'entity_'
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name FROM information_schema.tables WHERE table_name = 'entities' AND table_schema = 'core'"
    )
    .fetch_all(&pool)
    .await?;

    println!("Found legacy tables:");
    for (table,) in &tables {
        println!("- {}", table);

        // Get columns for this table
        let columns: Vec<(String, String)> = sqlx::query_as(
            "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = $1 ORDER BY ordinal_position"
        )
        .bind(table)
        .fetch_all(&pool)
        .await?;

        for (col, type_) in columns {
            println!("  - {}: {}", col, type_);
        }
        println!();
    }

    Ok(())
}
