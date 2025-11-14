use anyhow::Result;
use sqlx::PgPool;
use std::time::Duration;

// Since we can't import from the binary crate directly, we'll need to duplicate
// the minimal code needed or refactor the main crate to be a library

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Batch CPF Enrichment ===\n");

    // Load environment variables
    dotenv::dotenv().ok();

    let database_url = std::env::var("DB_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("DB_URL or DATABASE_URL must be set");

    let work_api_key = std::env::var("WORK_API")
        .or_else(|_| std::env::var("WORK_API_KEY"))
        .expect("WORK_API or WORK_API_KEY must be set");

    // Work API endpoint
    let work_api_base = "https://api.workrb.com.br/data/completa";

    // Connect to database
    println!("Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;
    println!("✓ Database connected\n");

    // List of CPFs to enrich
    let cpfs = vec![
        "78706009891",
        "03102582869",
        "21476477809",
        "23477990889",
        "23494180814",
        "23783304806",
        "23803176824",
        "23803341884",
        "23861750813",
        "23862127850",
        "23862128822",
        "24093964882",
        "24129692801",
        "24281151893",
        "31807362833",
        "47602137833",
        "47602346831",
        "47761568812",
        "53357342804",
    ];

    let total = cpfs.len();
    let mut success_count = 0;
    let mut fail_count = 0;

    // Process each CPF
    for (idx, cpf) in cpfs.iter().enumerate() {
        println!("[{}/{}] Processing CPF: {}", idx + 1, total, cpf);

        // Fetch enrichment data from Work API
        let url = format!("{}?chave={}&cpf={}", work_api_base, work_api_key, cpf);

        match reqwest::get(&url).await {
            Ok(response) => {
                match response.json::<serde_json::Value>().await {
                    Ok(data) => {
                        // Store in database
                        match store_enriched_person(&pool, cpf, &data).await {
                            Ok(entity_id) => {
                                println!("  ✓ Stored successfully - entity_id: {}", entity_id);
                                success_count += 1;
                            }
                            Err(e) => {
                                println!("  ✗ Storage failed: {}", e);
                                fail_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Failed to parse response: {}", e);
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                println!("  ✗ API request failed: {}", e);
                fail_count += 1;
            }
        }

        // Rate limiting - wait 1 second between requests
        if idx < total - 1 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    println!("\n=== Batch Enrichment Complete ===");
    println!("Total processed: {}", total);
    println!("✓ Success: {}", success_count);
    println!("✗ Failed: {}", fail_count);
    println!(
        "Success rate: {:.1}%",
        (success_count as f64 / total as f64) * 100.0
    );

    Ok(())
}

async fn store_enriched_person(
    pool: &PgPool,
    cpf: &str,
    data: &serde_json::Value,
) -> Result<uuid::Uuid> {
    // Extract basic data
    let dados_basicos = data.get("DadosBasicos");

    let nome = dados_basicos
        .and_then(|d| d.get("nome"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let sexo = dados_basicos
        .and_then(|d| d.get("sexo"))
        .and_then(|v| v.as_str());

    let data_nasc = dados_basicos
        .and_then(|d| d.get("dataNascimento"))
        .and_then(|v| v.as_str());

    let mae = dados_basicos
        .and_then(|d| d.get("nomeMae"))
        .and_then(|v| v.as_str());

    let pai = dados_basicos
        .and_then(|d| d.get("nomePai"))
        .and_then(|v| v.as_str());

    let rg = dados_basicos
        .and_then(|d| d.get("rg"))
        .and_then(|v| v.as_str());

    // Insert or update party
    let entity_id = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        INSERT INTO core.parties (
            party_type, cpf_cnpj, full_name, sex, birth_date,
            mother_name, father_name, rg, enriched
        )
        VALUES ($1, $2, $3, $4, $5::date, $6, $7, $8, $9)
        ON CONFLICT (cpf_cnpj)
        DO UPDATE SET
            full_name = EXCLUDED.full_name,
            sex = EXCLUDED.sex,
            birth_date = EXCLUDED.birth_date,
            mother_name = EXCLUDED.mother_name,
            father_name = EXCLUDED.father_name,
            rg = EXCLUDED.rg,
            enriched = EXCLUDED.enriched,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind("customer")
    .bind(cpf)
    .bind(nome)
    .bind(sexo)
    .bind(data_nasc)
    .bind(mae)
    .bind(pai)
    .bind(rg)
    .bind(true)
    .fetch_one(pool)
    .await?;

    // Store emails
    if let Some(emails) = data.get("emails").and_then(|v| v.as_array()) {
        for email_obj in emails.iter().take(5) {
            if let Some(email) = email_obj.get("email").and_then(|v| v.as_str()) {
                let _ = sqlx::query(
                    r#"
                    WITH email_insert AS (
                        INSERT INTO app.emails (email)
                        VALUES ($1)
                        ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email
                        RETURNING id
                    )
                    INSERT INTO core.party_emails (party_id, email_id)
                    SELECT $2, id FROM email_insert
                    ON CONFLICT DO NOTHING
                    "#,
                )
                .bind(email)
                .bind(entity_id)
                .execute(pool)
                .await;
            }
        }
    }

    // Store phones
    if let Some(telefones) = data.get("telefones").and_then(|v| v.as_array()) {
        for tel_obj in telefones.iter().take(5) {
            if let Some(telefone) = tel_obj.get("telefone").and_then(|v| v.as_str()) {
                let _ = sqlx::query(
                    r#"
                    WITH phone_insert AS (
                        INSERT INTO app.phones (number)
                        VALUES ($1)
                        ON CONFLICT (number) DO UPDATE SET number = EXCLUDED.number
                        RETURNING id
                    )
                    INSERT INTO core.party_phones (party_id, phone_id)
                    SELECT $2, id FROM phone_insert
                    ON CONFLICT DO NOTHING
                    "#,
                )
                .bind(telefone)
                .bind(entity_id)
                .execute(pool)
                .await;
            }
        }
    }

    Ok(entity_id)
}
