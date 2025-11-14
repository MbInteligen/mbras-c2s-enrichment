use crate::errors::AppError;
use crate::models::WorkApiCompleteResponse;
use bigdecimal::BigDecimal;
use chrono::Datelike;
use serde_json::json;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

/// Database storage service for enriched person data
pub struct EnrichmentStorage {
    pool: PgPool,
}

impl EnrichmentStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Store or update enriched person data from Work API
    /// Uses sequential queries instead of complex CTEs for better sqlx compatibility
    pub async fn store_enriched_person(
        &self,
        cpf: &str,
        work_data: &WorkApiCompleteResponse,
    ) -> Result<Uuid, AppError> {
        // Extract and prepare data
        let dados_basicos = work_data.get("DadosBasicos");
        let dados_econ = work_data.get("DadosEconomicos");

        // Extract basic fields
        let nome = dados_basicos
            .and_then(|d| d.get("nome"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let sexo = dados_basicos
            .and_then(|d| d.get("sexo"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.chars().next())
            .unwrap_or('M');

        let data_nasc = dados_basicos
            .and_then(|d| d.get("dataNascimento"))
            .and_then(|v| v.as_str())
            .and_then(|d| parse_br_date(d).ok());

        let nome_mae = dados_basicos
            .and_then(|d| d.get("nomeMae"))
            .and_then(|v| v.as_str());

        let nome_pai = dados_basicos
            .and_then(|d| d.get("nomePai"))
            .and_then(|v| v.as_str());

        let escolaridade = dados_basicos
            .and_then(|d| d.get("escolaridade"))
            .and_then(|v| v.as_str());

        let estado_civil = dados_basicos
            .and_then(|d| d.get("estadoCivil"))
            .and_then(|v| v.as_str());

        let nacionalidade = dados_basicos
            .and_then(|d| d.get("nacionalidade"))
            .and_then(|v| v.as_str());

        // Build profile metadata
        let mut profile_metadata = json!({});
        if let Some(mae) = nome_mae {
            profile_metadata["mother_name"] = json!(mae);
        }
        if let Some(pai) = nome_pai {
            if pai != "SEM INFORMAÇÃO" {
                profile_metadata["father_name"] = json!(pai);
            }
        }
        if let Some(cor) = dados_basicos
            .and_then(|d| d.get("cor"))
            .and_then(|v| v.as_str())
        {
            profile_metadata["cor"] = json!(cor);
        }
        if let Some(munic) = dados_basicos
            .and_then(|d| d.get("municipioNascimento"))
            .and_then(|v| v.as_str())
        {
            profile_metadata["municipio_nascimento"] = json!(munic);
        }
        if let Some(cns) = dados_basicos
            .and_then(|d| d.get("cns"))
            .and_then(|v| v.as_str())
        {
            profile_metadata["cns"] = json!(cns);
        }

        // Extract financial data
        let renda = dados_econ
            .and_then(|d| d.get("renda"))
            .and_then(|v| v.as_str())
            .and_then(|r| {
                let normalized = r.replace(",", ".");
                normalized.parse::<f64>().ok()
            })
            .and_then(|r| {
                let adjusted = r * 1.9; // Apply 1.9x multiplier
                BigDecimal::from_str(&adjusted.to_string()).ok()
            });

        let credit_score = dados_econ
            .and_then(|d| d.get("score"))
            .and_then(|s| s.get("scoreCSBA"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i32>().ok());

        let risk_level = dados_econ
            .and_then(|d| d.get("score"))
            .and_then(|s| s.get("scoreCSBAFaixaRisco"))
            .and_then(|v| v.as_str());

        // Map risk level to numeric score
        let risk_score = risk_level.and_then(|r| match r {
            "BAIXISSIMO RISCO" => BigDecimal::from_str("0.1").ok(),
            "BAIXO RISCO" => BigDecimal::from_str("0.3").ok(),
            "MEDIO RISCO" => BigDecimal::from_str("0.5").ok(),
            "ALTO RISCO" => BigDecimal::from_str("0.7").ok(),
            "ALTISSIMO RISCO" => BigDecimal::from_str("0.9").ok(),
            _ => None,
        });

        // Build financial metadata
        let mut financial_metadata = json!({});
        if let Some(poder_aq) = dados_econ.and_then(|d| d.get("poderAquisitivo")) {
            financial_metadata["poder_aquisitivo"] = poder_aq.clone();
        }
        if let Some(mosaic) = dados_econ.and_then(|d| d.get("serasaMosaic")) {
            financial_metadata["mosaic"] = mosaic.clone();
        }

        let current_year = chrono::Utc::now().year();

        // Create canonical name (uppercase, normalized)
        let canonical_name = nome.to_uppercase();

        // Step 1: Try to find existing entity first, then insert if not found
        let entity_id = match sqlx::query_as::<_, (Uuid,)>(
            "SELECT entity_id FROM core.entities WHERE national_id = $1 LIMIT 1",
        )
        .bind(cpf)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::DatabaseError)?
        {
            Some(existing) => {
                // Update existing entity
                sqlx::query(
                    r#"
                    UPDATE core.entities
                    SET is_enriched = true,
                        enriched_at = now(),
                        updated_at = now(),
                        name = COALESCE(name, $2),
                        canonical_name = COALESCE(canonical_name, $3)
                    WHERE national_id = $1
                    "#,
                )
                .bind(cpf)
                .bind(nome)
                .bind(&canonical_name)
                .execute(&self.pool)
                .await
                .map_err(AppError::DatabaseError)?;

                existing.0
            }
            None => {
                // Insert new entity
                let new_entity: (Uuid,) = sqlx::query_as(
                    r#"
                    INSERT INTO core.entities (national_id, name, canonical_name, entity_type, is_enriched, enriched_at, data_source)
                    VALUES ($1, $2, $3, 'person'::core.entity_type_enum, true, now(), 'api'::core.data_source_enum)
                    RETURNING entity_id
                    "#,
                )
                .bind(cpf)
                .bind(nome)
                .bind(&canonical_name)
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::DatabaseError)?;

                new_entity.0
            }
        };

        // Step 2: Upsert profile
        sqlx::query(
            r#"
            INSERT INTO core.entity_profiles (
                entity_id, sex, birth_date, nationality, marital_status,
                education_level, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (entity_id) DO UPDATE
            SET sex = COALESCE(entity_profiles.sex, EXCLUDED.sex),
                birth_date = COALESCE(entity_profiles.birth_date, EXCLUDED.birth_date),
                nationality = COALESCE(entity_profiles.nationality, EXCLUDED.nationality),
                marital_status = COALESCE(entity_profiles.marital_status, EXCLUDED.marital_status),
                education_level = COALESCE(entity_profiles.education_level, EXCLUDED.education_level),
                metadata = entity_profiles.metadata || EXCLUDED.metadata,
                updated_at = now()
            "#,
        )
        .bind(entity_id)
        .bind(sexo.to_string())
        .bind(data_nasc)
        .bind(nacionalidade)
        .bind(estado_civil)
        .bind(escolaridade)
        .bind(profile_metadata)
        .execute(&self.pool)
        .await
        .map_err(AppError::DatabaseError)?;

        // Step 3: Upsert financials (if we have any financial data)
        if renda.is_some() || credit_score.is_some() {
            // Check if financials exist for this year
            let existing = sqlx::query_as::<_, (Uuid,)>(
                "SELECT id FROM core.entity_financials WHERE entity_id = $1 AND financial_year = $2 AND financial_month IS NULL LIMIT 1"
            )
            .bind(entity_id)
            .bind(current_year)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::DatabaseError)?;

            if existing.is_some() {
                // Update existing financial record
                sqlx::query(
                    r#"
                    UPDATE core.entity_financials
                    SET reported_income = $3,
                        credit_score = $4,
                        risk_score = $5,
                        metadata = $6,
                        updated_at = now()
                    WHERE entity_id = $1 AND financial_year = $2 AND financial_month IS NULL
                    "#,
                )
                .bind(entity_id)
                .bind(current_year)
                .bind(&renda)
                .bind(credit_score)
                .bind(&risk_score)
                .bind(&financial_metadata)
                .execute(&self.pool)
                .await
                .map_err(AppError::DatabaseError)?;
            } else {
                // Insert new financial record
                sqlx::query(
                    r#"
                    INSERT INTO core.entity_financials (
                        entity_id, financial_year, reported_income, credit_score,
                        risk_score, source, confidence, metadata
                    )
                    VALUES ($1, $2, $3, $4, $5, 'api'::core.data_source_enum, 'high', $6)
                    "#,
                )
                .bind(entity_id)
                .bind(current_year)
                .bind(&renda)
                .bind(credit_score)
                .bind(&risk_score)
                .bind(&financial_metadata)
                .execute(&self.pool)
                .await
                .map_err(AppError::DatabaseError)?;
            }
        }

        // Store emails (separate query for array handling)
        if let Some(emails) = work_data.get("emails").and_then(|e| e.as_array()) {
            self.store_emails(entity_id, emails).await?;
        }

        // Store phones (separate query for array handling)
        if let Some(telefones) = work_data.get("telefones").and_then(|t| t.as_array()) {
            self.store_phones(entity_id, telefones).await?;
        }

        // Store addresses (separate query)
        if let Some(enderecos) = work_data.get("enderecos").and_then(|e| e.as_array()) {
            self.store_addresses(entity_id, enderecos).await?;
        }

        tracing::info!(
            "Successfully stored enriched data for CPF: {} (entity_id: {})",
            cpf,
            entity_id
        );

        Ok(entity_id)
    }

    /// Store emails for an entity
    async fn store_emails(
        &self,
        entity_id: Uuid,
        emails: &[serde_json::Value],
    ) -> Result<(), AppError> {
        for (idx, email_obj) in emails.iter().enumerate() {
            let email = email_obj.get("email").and_then(|e| e.as_str());
            let prioridade = email_obj.get("prioridade").and_then(|p| p.as_str());
            let qualidade = email_obj.get("qualidade").and_then(|q| q.as_str());

            if let Some(email_addr) = email {
                let is_primary = idx == 0; // First email is primary
                let is_verified = qualidade == Some("BOM");

                let mut metadata = json!({});
                if let Some(prio) = prioridade {
                    metadata["prioridade"] = json!(prio);
                }
                if let Some(qual) = qualidade {
                    metadata["qualidade"] = json!(qual);
                }
                if let Some(pessoal) = email_obj.get("emailPessoal").and_then(|p| p.as_str()) {
                    metadata["email_pessoal"] = json!(pessoal);
                }
                if let Some(blacklist) = email_obj.get("blacklist").and_then(|b| b.as_str()) {
                    metadata["blacklist"] = json!(blacklist);
                }

                // Try to insert, ignore if already exists
                let _ = sqlx::query(
                    r#"
                    INSERT INTO core.entity_emails (entity_id, email, email_type, is_primary, is_verified, metadata)
                    VALUES ($1, $2, 'personal', $3, $4, $5)
                    "#,
                )
                .bind(entity_id)
                .bind(email_addr.to_lowercase())
                .bind(is_primary)
                .bind(is_verified)
                .bind(&metadata)
                .execute(&self.pool)
                .await;
            }
        }

        Ok(())
    }

    /// Store phones for an entity
    async fn store_phones(
        &self,
        entity_id: Uuid,
        telefones: &[serde_json::Value],
    ) -> Result<(), AppError> {
        for (idx, phone_obj) in telefones.iter().enumerate() {
            let telefone = phone_obj.get("telefone").and_then(|t| t.as_str());
            let tipo = phone_obj.get("tipo").and_then(|t| t.as_str());
            let whatsapp = phone_obj.get("whatsapp").and_then(|w| w.as_str());
            let operadora = phone_obj.get("operadora").and_then(|o| o.as_str());
            let status = phone_obj.get("status").and_then(|s| s.as_str());

            if let Some(phone) = telefone {
                let is_primary = idx == 0;
                let is_whatsapp = whatsapp == Some("SIM");

                // Map tipo to phone_type
                let phone_type = match tipo {
                    Some(t) if t.contains("MÓVEL") || t.contains("MOVEL") => "mobile",
                    Some(t) if t.contains("RESIDENCIAL") => "landline",
                    _ => "mobile", // Default to mobile
                };

                let mut metadata = json!({});
                if let Some(st) = status {
                    metadata["status"] = json!(st);
                }

                // Try to insert, ignore if already exists
                let _ = sqlx::query(
                    r#"
                    INSERT INTO core.entity_phones (entity_id, phone, phone_type, is_primary, is_whatsapp, carrier, metadata)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                )
                .bind(entity_id)
                .bind(phone)
                .bind(phone_type)
                .bind(is_primary)
                .bind(is_whatsapp)
                .bind(operadora)
                .bind(&metadata)
                .execute(&self.pool)
                .await;
            }
        }

        Ok(())
    }

    /// Store addresses for an entity
    async fn store_addresses(
        &self,
        entity_id: Uuid,
        enderecos: &[serde_json::Value],
    ) -> Result<(), AppError> {
        for (idx, endereco_obj) in enderecos.iter().enumerate() {
            let street_type = endereco_obj.get("tipoLogradouro").and_then(|t| t.as_str());
            let street = endereco_obj.get("logradouro").and_then(|l| l.as_str());
            let number = endereco_obj
                .get("logradouroNumero")
                .and_then(|n| n.as_str());
            let complement = endereco_obj.get("complemento").and_then(|c| c.as_str());
            let neighborhood = endereco_obj.get("bairro").and_then(|b| b.as_str());
            let city = endereco_obj.get("cidade").and_then(|c| c.as_str());
            let state = endereco_obj.get("uf").and_then(|u| u.as_str());
            let zip_code = endereco_obj.get("cep").and_then(|z| z.as_str());

            if street.is_some() || zip_code.is_some() {
                let is_primary = idx == 0;

                // Step 1: Insert address
                let address_row: Result<(i32,), _> = sqlx::query_as(
                    r#"
                    INSERT INTO app.addresses (street_type, street, number, complement, neighborhood, city, state, zip_code)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    RETURNING id
                    "#,
                )
                .bind(street_type)
                .bind(street)
                .bind(number)
                .bind(complement)
                .bind(neighborhood)
                .bind(city)
                .bind(state)
                .bind(zip_code)
                .fetch_one(&self.pool)
                .await;

                // Step 2: Link address to entity (ignore errors if already linked)
                if let Ok(addr) = address_row {
                    // Try to link, ignore if already linked
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO core.entity_addresses (entity_id, address_id, address_type, is_primary, is_current, data_source)
                        VALUES ($1, $2, 'residential', $3, true, 'api'::core.data_source_enum)
                        "#,
                    )
                    .bind(entity_id)
                    .bind(addr.0)
                    .bind(is_primary)
                    .execute(&self.pool)
                    .await;
                }
            }
        }

        Ok(())
    }
}

/// Parse Brazilian date format (DD/MM/YYYY) to chrono::NaiveDate
fn parse_br_date(date_str: &str) -> Result<chrono::NaiveDate, chrono::ParseError> {
    chrono::NaiveDate::parse_from_str(date_str, "%d/%m/%Y")
}
