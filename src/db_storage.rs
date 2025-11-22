use crate::errors::AppError;
use crate::models::WorkApiCompleteResponse;
use bigdecimal::BigDecimal;
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
        self.store_enriched_person_with_lead(cpf, work_data, None)
            .await
    }

    /// Store enriched person data with optional lead_id for C2S tracking
    pub async fn store_enriched_person_with_lead(
        &self,
        cpf: &str,
        work_data: &WorkApiCompleteResponse,
        lead_id: Option<&str>,
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

        let _escolaridade = dados_basicos
            .and_then(|d| d.get("escolaridade"))
            .and_then(|v| v.as_str());

        let estado_civil = dados_basicos
            .and_then(|d| d.get("estadoCivil"))
            .and_then(|v| v.as_str());

        let _nacionalidade = dados_basicos
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
        // Create canonical name (uppercase, normalized)
        let canonical_name = nome.to_uppercase();

        // Build payload for enrichment (attach lead_id if present) and normalized data (addresses)
        let mut enrichment_payload = work_data.clone();
        if let Some(lid) = lead_id {
            enrichment_payload["lead_id"] = json!(lid);
        }
        let mut normalized_data = json!({});
        if let Some(enderecos) = work_data.get("enderecos").and_then(|e| e.as_array()) {
            normalized_data["addresses"] = serde_json::Value::Array(enderecos.to_vec());
        }

        // Step 1: Upsert party
        let party_id = match sqlx::query_as::<_, (Uuid,)>(
            "SELECT id FROM core.parties WHERE cpf_cnpj = $1 LIMIT 1",
        )
        .bind(cpf)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::DatabaseError)?
        {
            Some(existing) => {
                sqlx::query(
                    r#"
                    UPDATE core.parties
                    SET party_type = COALESCE(party_type, $2),
                        full_name = COALESCE(full_name, $3),
                        normalized_name = COALESCE(normalized_name, $4),
                        enriched = true,
                        birth_date = COALESCE(birth_date, $5),
                        sex = COALESCE(sex, $6),
                        mother_name = COALESCE(mother_name, $7),
                        opening_date = COALESCE(opening_date, $8),
                        company_type = COALESCE(company_type, $9),
                        company_size = COALESCE(company_size, $10),
                        updated_at = now()
                    WHERE id = $1
                    "#,
                )
                .bind(existing.0)
                .bind("person")
                .bind(nome)
                .bind(&canonical_name)
                .bind(data_nasc)
                .bind(Some(sexo.to_string()))
                .bind(nome_mae)
                .bind(None::<chrono::NaiveDate>)
                .bind(None::<String>)
                .bind(None::<String>)
                .execute(&self.pool)
                .await
                .map_err(AppError::DatabaseError)?;
                existing.0
            }
            None => {
                let inserted: (Uuid,) = sqlx::query_as(
                    r#"
                    INSERT INTO core.parties (
                        id, party_type, cpf_cnpj, full_name, normalized_name, enriched,
                        birth_date, sex, mother_name, opening_date, company_type, company_size,
                        created_at, updated_at
                    )
                    VALUES (gen_random_uuid(), $1, $2, $3, $4, true, $5, $6, $7, $8, $9, $10, now(), now())
                    RETURNING id
                    "#,
                )
                .bind("person")
                .bind(cpf)
                .bind(nome)
                .bind(&canonical_name)
                .bind(data_nasc)
                .bind(Some(sexo.to_string()))
                .bind(nome_mae)
                .bind(None::<chrono::NaiveDate>)
                .bind(None::<String>)
                .bind(None::<String>)
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::DatabaseError)?;
                inserted.0
            }
        };

        // Step 2: Upsert people
        sqlx::query(
            r#"
            INSERT INTO core.people (
                party_id, full_name, mothers_name, birth_date, sex,
                marital_status, document_cpf, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
            ON CONFLICT (party_id) DO UPDATE
            SET full_name = EXCLUDED.full_name,
                mothers_name = COALESCE(EXCLUDED.mothers_name, core.people.mothers_name),
                birth_date = COALESCE(EXCLUDED.birth_date, core.people.birth_date),
                sex = COALESCE(EXCLUDED.sex, core.people.sex),
                marital_status = COALESCE(EXCLUDED.marital_status, core.people.marital_status),
                document_cpf = COALESCE(EXCLUDED.document_cpf, core.people.document_cpf),
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(party_id)
        .bind(nome)
        .bind(nome_mae)
        .bind(data_nasc)
        .bind(Some(sexo.to_string()))
        .bind(estado_civil)
        .bind(cpf)
        .execute(&self.pool)
        .await
        .map_err(AppError::DatabaseError)?;

        // Step 3: Store contacts
        if let Some(emails) = work_data.get("emails").and_then(|e| e.as_array()) {
            self.store_party_emails(party_id, emails).await?;
        }
        if let Some(telefones) = work_data.get("telefones").and_then(|t| t.as_array()) {
            self.store_party_phones(party_id, telefones).await?;
        }

        // Step 4: Store enrichment snapshot
        let quality_score = risk_score
            .as_ref()
            .and_then(|bd| bd.to_string().parse::<f64>().ok())
            .unwrap_or(0.5);

        sqlx::query(
            r#"
            INSERT INTO core.party_enrichments (
                enrichment_id, party_id, provider, raw_payload, normalized_data,
                quality_score, enriched_at, created_at
            )
            VALUES (gen_random_uuid(), $1, 'work_api', $2, '{}'::jsonb, $3, now(), now())
            ON CONFLICT (party_id) DO UPDATE
            SET provider = EXCLUDED.provider,
                raw_payload = EXCLUDED.raw_payload,
                quality_score = GREATEST(core.party_enrichments.quality_score, EXCLUDED.quality_score),
                enriched_at = EXCLUDED.enriched_at
            "#,
        )
        .bind(party_id)
        .bind(&enrichment_payload)
        .bind(quality_score)
        .execute(&self.pool)
        .await
        .map_err(AppError::DatabaseError)?;

        tracing::info!(
            "Successfully stored enriched data for CPF: {} (party_id: {})",
            cpf,
            party_id
        );

        Ok(party_id)
    }

    /// Store emails for a party
    async fn store_party_emails(
        &self,
        party_id: Uuid,
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

                let _ = sqlx::query(
                    r#"
                    INSERT INTO core.party_contacts (
                        contact_id, party_id, contact_type, value,
                        is_primary, is_verified, is_whatsapp, source,
                        confidence, valid_from, valid_to, created_at, updated_at
                    )
                    VALUES (gen_random_uuid(), $1, 'email'::core.contact_type_enum, $2, $3, $4, false, $5, $6, now(), NULL, now(), now())
                    ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING
                    "#,
                )
                .bind(party_id)
                .bind(email_addr.to_lowercase())
                .bind(is_primary)
                .bind(is_verified)
                .bind(metadata.get("prioridade").and_then(|v| v.as_str()))
                .bind(metadata
                    .get("qualidade")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok()))
                .execute(&self.pool)
                .await;
            }
        }

        Ok(())
    }

    /// Store phones for a party
    async fn store_party_phones(
        &self,
        party_id: Uuid,
        telefones: &[serde_json::Value],
    ) -> Result<(), AppError> {
        for (idx, phone_obj) in telefones.iter().enumerate() {
            let telefone = phone_obj.get("telefone").and_then(|t| t.as_str());
            let _tipo = phone_obj.get("tipo").and_then(|t| t.as_str());
            let whatsapp = phone_obj.get("whatsapp").and_then(|w| w.as_str());
            let operadora = phone_obj.get("operadora").and_then(|o| o.as_str());
            let status = phone_obj.get("status").and_then(|s| s.as_str());

            if let Some(phone) = telefone {
                let is_primary = idx == 0;
                let is_whatsapp = whatsapp == Some("SIM");
                let normalized: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

                let _ = sqlx::query(
                    r#"
                    INSERT INTO core.party_contacts (
                        contact_id, party_id, contact_type, value,
                        is_primary, is_verified, is_whatsapp, source,
                        confidence, valid_from, valid_to, created_at, updated_at
                    )
                    VALUES (
                        gen_random_uuid(), $1,
                        CASE WHEN $3 THEN 'whatsapp'::core.contact_type_enum ELSE 'phone'::core.contact_type_enum END,
                        $2, $4, true, $3, $5, $6, now(), NULL, now(), now()
                    )
                    ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING
                    "#,
                )
                .bind(party_id)
                .bind(&normalized)
                .bind(is_whatsapp)
                .bind(is_primary)
                .bind(operadora)
                .bind(status.and_then(|s| s.parse::<f64>().ok()))
                .execute(&self.pool)
                .await;
            }
        }

        Ok(())
    }
}

/// Parse Brazilian date format (DD/MM/YYYY) to chrono::NaiveDate
fn parse_br_date(date_str: &str) -> Result<chrono::NaiveDate, chrono::ParseError> {
    chrono::NaiveDate::parse_from_str(date_str, "%d/%m/%Y")
}
