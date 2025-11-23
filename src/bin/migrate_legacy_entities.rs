use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use sqlx::FromRow;
use std::env;
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct LegacyEntity {
    entity_id: Uuid,
    national_id: Option<String>,
    name: Option<String>,
    canonical_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, FromRow)]
struct LegacyProfile {
    sex: Option<String>,
    birth_date: Option<chrono::NaiveDate>,
    #[allow(dead_code)]
    mother_name: Option<String>, // Note: Schema inspection didn't show mother_name in entity_profiles, checking if it exists or if we need to skip
}

#[derive(Debug, FromRow)]
struct LegacyPhone {
    phone: String,
    is_primary: bool,
    is_whatsapp: bool,
}

#[derive(Debug, FromRow)]
struct LegacyEmail {
    email: String,
    is_primary: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    tracing::info!("Starting migration of legacy entities...");

    // Fetch all legacy entities
    let entities: Vec<LegacyEntity> = sqlx::query_as(
        "SELECT entity_id, national_id, name, canonical_name, created_at FROM core.entities",
    )
    .fetch_all(&pool)
    .await?;

    let total_entities = entities.len();
    tracing::info!("Found {} legacy entities to process.", total_entities);

    let mut migrated_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    let mut processed_count = 0;
    for entity in entities {
        processed_count += 1;
        if processed_count % 1000 == 0 {
            tracing::info!(
                "Processed {}/{} entities (Migrated: {}, Skipped: {}, Errors: {})",
                processed_count,
                total_entities,
                migrated_count,
                skipped_count,
                error_count
            );
        }

        let cpf = match &entity.national_id {
            Some(id) => id,
            None => {
                tracing::warn!("Skipping entity {} (no national_id)", entity.entity_id);
                continue;
            }
        };

        // Check if already exists in parties
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM core.parties WHERE cpf_cnpj = $1)")
                .bind(cpf)
                .fetch_one(&pool)
                .await?;

        if exists {
            tracing::debug!(
                "Skipping entity {} (CPF {} already exists)",
                entity.entity_id,
                cpf
            );
            skipped_count += 1;
            continue;
        }

        // Start transaction
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!("Failed to start transaction: {}", e);
                error_count += 1;
                continue;
            }
        };

        // 1. Insert Party
        // Try to use original ID, but if it conflicts (unlikely since we checked CPF, but ID might exist), handle it?
        // Actually, let's just use the original ID. If it conflicts on ID but not CPF, that's a weird state, but we'll let it error.
        let party_id = entity.entity_id;

        let insert_party_result = sqlx::query(
            r#"
            INSERT INTO core.parties (
                id, party_type, cpf_cnpj, full_name, normalized_name, enriched,
                created_at, updated_at
            )
            VALUES ($1, 'person', $2, $3, $4, false, $5, now())
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(party_id)
        .bind(cpf)
        .bind(&entity.name)
        .bind(&entity.canonical_name)
        .bind(entity.created_at)
        .execute(&mut *tx)
        .await;

        if let Err(e) = insert_party_result {
            tracing::error!("Failed to insert party {}: {}", party_id, e);
            error_count += 1;
            continue; // Transaction rolls back on drop
        }

        // 2. Fetch Profile Data
        // Note: Based on previous schema inspection, entity_profiles has: sex, birth_date.
        // It does NOT have mother_name in the list I saw earlier?
        // Let's check the schema output again...
        // "entity_profiles ... sex, birth_date, death_date, nationality, marital_status..."
        // It does NOT list mother_name. So we will skip mother_name.

        let profile: Option<LegacyProfile> = sqlx::query_as(
            "SELECT sex, birth_date, NULL as mother_name FROM core.entity_profiles WHERE entity_id = $1"
        )
        .bind(party_id)
        .fetch_optional(&pool) // Read from pool, not tx (legacy data is stable)
        .await
        .unwrap_or(None);

        // 3. Insert People
        if let Some(prof) = profile {
            let insert_people = sqlx::query(
                r#"
                INSERT INTO core.people (
                    party_id, full_name, birth_date, sex, document_cpf, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, now())
                ON CONFLICT (party_id) DO NOTHING
                "#,
            )
            .bind(party_id)
            .bind(&entity.name)
            .bind(prof.birth_date)
            .bind(prof.sex.map(|s| s.to_string()))
            .bind(cpf)
            .bind(entity.created_at)
            .execute(&mut *tx)
            .await;

            if let Err(e) = insert_people {
                tracing::error!("Failed to insert people {}: {}", party_id, e);
                error_count += 1;
                continue;
            }
        } else {
            // Insert minimal person record
            let insert_people = sqlx::query(
                r#"
                INSERT INTO core.people (
                    party_id, full_name, document_cpf, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, now())
                ON CONFLICT (party_id) DO NOTHING
                "#,
            )
            .bind(party_id)
            .bind(&entity.name)
            .bind(cpf)
            .bind(entity.created_at)
            .execute(&mut *tx)
            .await;

            if let Err(e) = insert_people {
                tracing::error!("Failed to insert people {}: {}", party_id, e);
                error_count += 1;
                continue;
            }
        }

        // 4. Migrate Phones
        let phones: Vec<LegacyPhone> = sqlx::query_as(
            "SELECT phone, is_primary, is_whatsapp FROM core.entity_phones WHERE entity_id = $1",
        )
        .bind(party_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        for phone in phones {
            let contact_type = if phone.is_whatsapp {
                "whatsapp"
            } else {
                "phone"
            };
            // Normalize phone (digits only)
            let normalized: String = phone.phone.chars().filter(|c| c.is_ascii_digit()).collect();

            let _ = sqlx::query(
                r#"
                INSERT INTO core.party_contacts (
                    contact_id, party_id, contact_type, value,
                    is_primary, is_whatsapp, created_at, updated_at
                )
                VALUES (gen_random_uuid(), $1, $2::core.contact_type_enum, $3, $4, $5, now(), now())
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(party_id)
            .bind(contact_type)
            .bind(normalized)
            .bind(phone.is_primary)
            .bind(phone.is_whatsapp)
            .execute(&mut *tx)
            .await;
        }

        // 5. Migrate Emails
        let emails: Vec<LegacyEmail> =
            sqlx::query_as("SELECT email, is_primary FROM core.entity_emails WHERE entity_id = $1")
                .bind(party_id)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

        for email in emails {
            let _ = sqlx::query(
                r#"
                INSERT INTO core.party_contacts (
                    contact_id, party_id, contact_type, value,
                    is_primary, created_at, updated_at
                )
                VALUES (gen_random_uuid(), $1, 'email'::core.contact_type_enum, $2, $3, now(), now())
                ON CONFLICT DO NOTHING
                "#
            )
            .bind(party_id)
            .bind(email.email)
            .bind(email.is_primary)
            .execute(&mut *tx)
            .await;
        }

        // Commit
        if let Err(e) = tx.commit().await {
            tracing::error!("Failed to commit transaction for {}: {}", party_id, e);
            error_count += 1;
        } else {
            migrated_count += 1;
            if migrated_count % 100 == 0 {
                tracing::info!("Migrated {} entities...", migrated_count);
            }
        }
    }

    tracing::info!("Migration complete.");
    tracing::info!("Migrated: {}", migrated_count);
    tracing::info!("Skipped (already exists): {}", skipped_count);
    tracing::info!("Errors: {}", error_count);

    Ok(())
}
