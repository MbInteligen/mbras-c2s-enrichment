use anyhow::Result;
use sqlx::PgPool;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Import Enriched JSON Files to Database ===\n");

    // Load environment variables
    dotenv::dotenv().ok();

    let database_url = std::env::var("DB_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("DB_URL or DATABASE_URL must be set");

    // Connect to database
    println!("Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;
    println!("✓ Database connected\n");

    // Find all enriched JSON files
    let json_files: Vec<_> = fs::read_dir(".")?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("temp_enriched_")
                && entry.file_name().to_string_lossy().ends_with(".json")
        })
        .collect();

    let total = json_files.len();
    if total == 0 {
        println!("No enriched JSON files found (temp_enriched_*.json)");
        return Ok(());
    }

    println!("Found {} enriched files to import\n", total);

    let mut success_count = 0;
    let mut fail_count = 0;

    for (idx, entry) in json_files.iter().enumerate() {
        let file_path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Extract CPF from filename: temp_enriched_12345678901.json
        let cpf = file_name_str
            .strip_prefix("temp_enriched_")
            .and_then(|s| s.strip_suffix(".json"))
            .unwrap_or("");

        println!(
            "[{}/{}] Processing {} (CPF: {})",
            idx + 1,
            total,
            file_name_str,
            cpf
        );

        // Read JSON file
        match fs::read_to_string(&file_path) {
            Ok(json_content) => {
                match serde_json::from_str::<serde_json::Value>(&json_content) {
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
                        println!("  ✗ Failed to parse JSON: {}", e);
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Failed to read file: {}", e);
                fail_count += 1;
            }
        }
    }

    println!("\n=== Import Complete ===");
    println!("Total files: {}", total);
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

    // Extract sexo - handle "M - MASCULINO" format by taking first char
    let sexo: Option<&str> = dados_basicos
        .and_then(|d| d.get("sexo"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.trim().chars().next())
        .map(|c| match c {
            'M' => "M",
            'F' => "F",
            _ => "I", // Indeterminado
        });

    // Convert date from DD/MM/YYYY to YYYY-MM-DD
    let data_nasc = dados_basicos
        .and_then(|d| d.get("dataNascimento"))
        .and_then(|v| v.as_str())
        .and_then(|date_str| {
            // Parse DD/MM/YYYY format
            let parts: Vec<&str> = date_str.split('/').collect();
            if parts.len() == 3 {
                Some(format!("{}-{}-{}", parts[2], parts[1], parts[0]))
            } else {
                // If already in correct format or parse fails, return as-is
                Some(date_str.to_string())
            }
        });

    let mae = dados_basicos
        .and_then(|d| d.get("nomeMae"))
        .and_then(|v| v.as_str());

    let pai = dados_basicos
        .and_then(|d| d.get("nomePai"))
        .and_then(|v| v.as_str());

    let rg = dados_basicos
        .and_then(|d| d.get("rg"))
        .and_then(|v| v.as_str());

    // Insert new party (without ON CONFLICT since table doesn't have unique constraint)
    let entity_id = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        INSERT INTO core.parties (
            party_type, cpf_cnpj, full_name, sex, birth_date,
            mother_name, father_name, rg, enriched
        )
        VALUES ($1, $2, $3, $4, $5::date, $6, $7, $8, $9)
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
                // First, try to find existing email by normalized_email
                let email_id: Option<uuid::Uuid> = sqlx::query_scalar(
                    "SELECT id FROM app.emails WHERE normalized_email = LOWER(TRIM($1))",
                )
                .bind(email)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

                let final_email_id: Option<uuid::Uuid> = if let Some(id) = email_id {
                    Some(id)
                } else {
                    // Insert new email
                    sqlx::query_scalar("INSERT INTO app.emails (email) VALUES ($1) RETURNING id")
                        .bind(email)
                        .fetch_one(pool)
                        .await
                        .ok()
                };

                // Link email to party
                if let Some(email_id) = final_email_id {
                    let _ = sqlx::query(
                        "INSERT INTO core.party_emails (party_id, email_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                    )
                    .bind(entity_id)
                    .bind(email_id)
                    .execute(pool)
                    .await;
                }
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
