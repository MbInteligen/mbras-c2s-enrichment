use std::env;
use uuid::Uuid;

use rust_c2s_api::data::db_storage::EnrichmentStorage;
use rust_c2s_api::db::Database;
use rust_c2s_api::models::WorkApiCompleteResponse;

/// Integration smoke test for enrichment storage writing to the Party Model.
/// Marked ignored to avoid running against production by accident; set TEST_DATABASE_URL to run.
#[tokio::test]
#[ignore]
async fn store_enriched_person_smoke_test() -> anyhow::Result<()> {
    let db_url = env::var("TEST_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| anyhow::anyhow!("Set TEST_DATABASE_URL or DATABASE_URL to run this test"))?;

    let db = Database::new(&db_url)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let storage = EnrichmentStorage::new(db.pool.clone());

    // Minimal Work API payload; storage is resilient to missing optional fields.
    let payload: WorkApiCompleteResponse = serde_json::json!({
        "DadosBasicos": {
            "nome": "Test User Party",
            "sexo": "M",
            "dataNascimento": "01/01/1990"
        },
        // No emails/phones/addresses provided; function will skip those sections.
    });

    // Use a unique CPF to avoid conflicts on repeated runs.
    let cpf = format!("999{:09}", Uuid::new_v4().as_u128() % 1_000_000_000);

    let party_id = storage
        .store_enriched_person_with_lead(&cpf, &payload, Some("test-lead-id"))
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    assert_ne!(party_id, Uuid::nil());
    Ok(())
}
