/// Shared enrichment logic for both webhook and HTTP handlers
///
/// This module provides reusable functions for the enrichment workflow:
/// 1. Find CPF via Diretrix (phone/email lookup)
/// 2. Enrich CPF data via Work API
/// 3. Format enriched message
/// 4. Send message to C2S
/// 5. Store in database
use crate::config::Config;
use crate::db_storage::EnrichmentStorage;
use crate::errors::AppError;
use crate::gateway_client::C2sGatewayClient;
use crate::services::{C2SService, DiretrixService, WorkApiService};
use phonenumber::country::Id as CountryId;
use phonenumber::Mode;
use regex::Regex;
use serde_json::{json, Value};
use sqlx::PgPool;

/// Result of CPF lookup via Diretrix
#[derive(Debug)]
pub struct CpfLookupResult {
    pub cpfs: Vec<String>,
    pub same_person: bool,
}

/// Validate email address
///
/// Checks for:
/// - Basic email format (contains @ and .)
/// - Fake/placeholder patterns (repeated digits like 9999, 1111)
/// - Minimum length requirements
/// - Valid domain structure
pub fn is_valid_email(email: &str) -> bool {
    // Basic checks
    if email.len() < 5 || !email.contains('@') || !email.contains('.') {
        return false;
    }

    // Detect fake patterns (repeated digits)
    let fake_patterns = [
        "999999",    // Common fake: 1199999999333@gmail.com
        "111111",    // Common fake: 1111111111@
        "000000",    // Common fake: 000000@
        "123456789", // Sequential fake
    ];

    for pattern in &fake_patterns {
        if email.contains(pattern) {
            tracing::warn!(
                "❌ Invalid email detected (fake pattern '{}'): {}",
                pattern,
                email
            );
            return false;
        }
    }

    // RFC 5322 simplified email regex
    // Matches: local@domain.tld
    let email_regex = Regex::new(
        r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"
    ).unwrap();

    if !email_regex.is_match(email) {
        tracing::warn!("❌ Invalid email format: {}", email);
        return false;
    }

    true
}

/// Validate and normalize Brazilian phone number
///
/// Uses phonenumber library (port of Google's libphonenumber) to:
/// - Parse phone number with Brazilian region (BR)
/// - Validate if it's a valid Brazilian number
/// - Return normalized E.164 format (+5511987654321)
///
/// Returns: (is_valid, normalized_phone_or_error_msg)
pub fn validate_br_phone(raw: &str) -> (bool, String) {
    // Skip empty or very short strings
    if raw.trim().is_empty() || raw.len() < 8 {
        return (false, "Phone too short".to_string());
    }

    // Parse with Brazilian country code
    match phonenumber::parse(Some(CountryId::BR), raw) {
        Ok(number) => {
            // Check if valid
            if phonenumber::is_valid(&number) {
                // Format to E.164 (+5511987654321)
                let formatted = number.format().mode(Mode::E164).to_string();
                tracing::debug!("✓ Valid BR phone: {} → {}", raw, formatted);
                (true, formatted)
            } else {
                tracing::warn!("❌ Invalid BR phone number: {}", raw);
                (false, "Invalid Brazilian phone number".to_string())
            }
        }
        Err(e) => {
            tracing::warn!("❌ Failed to parse BR phone '{}': {:?}", raw, e);
            (false, format!("Parse error: {:?}", e))
        }
    }
}

/// Find CPF(s) from phone and/or email using Diretrix API
pub async fn find_cpf_via_diretrix(
    phone: Option<&str>,
    email: Option<&str>,
    config: &Config,
) -> Result<CpfLookupResult, AppError> {
    let diretrix_service = DiretrixService::new(config);

    // Validate and normalize phone before lookup
    let validated_phone = if let Some(phone_number) = phone {
        if !phone_number.is_empty() {
            let (is_valid, normalized) = validate_br_phone(phone_number);
            if is_valid {
                Some(normalized)
            } else {
                tracing::warn!(
                    "Skipping invalid phone for Diretrix lookup: {}",
                    phone_number
                );
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Validate email before lookup
    let validated_email = if let Some(email_addr) = email {
        if !email_addr.is_empty() && is_valid_email(email_addr) {
            Some(email_addr.to_string())
        } else {
            if !email_addr.is_empty() {
                tracing::warn!(
                    "Skipping invalid/fake email for Diretrix lookup: {}",
                    email_addr
                );
            }
            None
        }
    } else {
        None
    };

    // Parallel lookup - search by phone AND email separately (only if validated)
    let phone_lookup = if let Some(ref phone_number) = validated_phone {
        diretrix_service.search_by_phone(phone_number).await.ok()
    } else {
        None
    };

    let email_lookup = if let Some(ref email_addr) = validated_email {
        diretrix_service.search_by_email(email_addr).await.ok()
    } else {
        None
    };

    // Extract CPFs from both lookups
    let phone_cpf = phone_lookup.as_ref().and_then(|results| {
        if !results.is_empty() {
            Some(results[0].cpf.clone())
        } else {
            None
        }
    });

    let email_cpf = email_lookup.as_ref().and_then(|results| {
        if !results.is_empty() {
            Some(results[0].cpf.clone())
        } else {
            None
        }
    });

    // Check if both found and if they're the same person
    let (cpfs, same_person) = match (&phone_cpf, &email_cpf) {
        (Some(p_cpf), Some(e_cpf)) if p_cpf == e_cpf => {
            tracing::info!(
                "✓ Phone and email belong to the same person (CPF: {})",
                p_cpf
            );
            (vec![p_cpf.clone()], true)
        }
        (Some(p_cpf), Some(e_cpf)) => {
            tracing::warn!(
                "⚠ Phone and email belong to DIFFERENT people! Phone CPF: {}, Email CPF: {}",
                p_cpf,
                e_cpf
            );
            (vec![p_cpf.clone(), e_cpf.clone()], false)
        }
        (Some(cpf), None) | (None, Some(cpf)) => {
            tracing::info!("Found CPF from single source: {}", cpf);
            (vec![cpf.clone()], false)
        }
        (None, None) => {
            tracing::error!("Could not find CPF from either phone or email");
            return Err(AppError::NotFound(
                "Could not find CPF via Diretrix".to_string(),
            ));
        }
    };

    Ok(CpfLookupResult { cpfs, same_person })
}

/// Enrich multiple CPFs with Work API
pub async fn enrich_cpfs_with_work_api(
    cpfs: &[String],
    config: &Config,
) -> Result<Vec<Value>, AppError> {
    let work_api_service = WorkApiService::new(config);

    let mut enriched_data = Vec::new();
    for cpf in cpfs {
        tracing::info!("Enriching CPF: {}", cpf);
        match work_api_service.fetch_all_modules(cpf).await {
            Ok(data) => enriched_data.push(data),
            Err(e) => {
                tracing::warn!("Failed to enrich CPF {}: {}", cpf, e);
                // Continue with other CPFs even if one fails
            }
        }
    }

    if enriched_data.is_empty() {
        return Err(AppError::ExternalApiError(
            "No enrichment data available".to_string(),
        ));
    }

    Ok(enriched_data)
}

/// Format enriched data as message body
pub fn format_enriched_message_body(
    customer_name: &str,
    phone: &str,
    email: &str,
    enriched_data: &[Value],
    same_person: bool,
) -> String {
    if same_person {
        let enriched_msg =
            crate::handlers::format_enriched_message(customer_name, &enriched_data[0]);
        tracing::info!("Enriched message length: {} chars", enriched_msg.len());
        format!("📞📧 Telefone e e-mail da mesma pessoa\n\n{}", enriched_msg)
    } else {
        let mut combined_message =
            String::from("⚠️ Telefone e e-mail relacionados a PESSOAS DIFERENTES!\n\n");

        combined_message.push_str(&format!("═══ PESSOA 1 (Telefone: {}) ═══\n", phone));
        combined_message.push_str(&crate::handlers::format_enriched_message(
            "",
            &enriched_data[0],
        ));

        if enriched_data.len() > 1 {
            combined_message.push_str(&format!("\n\n═══ PESSOA 2 (Email: {}) ═══\n", email));
            combined_message.push_str(&crate::handlers::format_enriched_message(
                "",
                &enriched_data[1],
            ));
        }

        combined_message
    }
}

/// Send enriched message to C2S (via gateway if available)
pub async fn send_message_to_c2s(
    lead_id: &str,
    message: &str,
    gateway_client: Option<&C2sGatewayClient>,
    config: &Config,
) -> Result<(), AppError> {
    if let Some(gateway) = gateway_client {
        tracing::info!("Using C2S Gateway to send message");
        gateway.send_message(lead_id, message).await?;
    } else {
        tracing::info!("Using direct C2S API to send message");
        let c2s_service = C2SService::new(config);
        c2s_service.send_message(lead_id, message).await?;
    }

    Ok(())
}

/// Store enriched data in database
pub async fn store_enriched_data(
    db: &PgPool,
    cpfs: &[String],
    enriched_data: &[Value],
    lead_id: Option<&str>,
) -> Result<Vec<uuid::Uuid>, AppError> {
    let storage = EnrichmentStorage::new(db.clone());

    let mut stored_entity_ids = Vec::new();
    for (idx, cpf) in cpfs.iter().enumerate() {
        if idx >= enriched_data.len() {
            tracing::warn!("No enriched data for CPF {}", cpf);
            continue;
        }

        match storage
            .store_enriched_person_with_lead(cpf, &enriched_data[idx], lead_id)
            .await
        {
            Ok(entity_id) => {
                tracing::info!(
                    "✓ Stored CPF {} → entity_id: {} (lead_id: {:?})",
                    cpf,
                    entity_id,
                    lead_id
                );
                stored_entity_ids.push(entity_id);
            }
            Err(e) => {
                tracing::error!("✗ Failed to store CPF {}: {}", cpf, e);
                // Don't fail the whole request, just log the error
            }
        }
    }

    Ok(stored_entity_ids)
}

/// Complete enrichment workflow for a lead
///
/// This is the main entry point that orchestrates the entire enrichment process:
/// 1. Find CPF(s) via Diretrix
/// 2. Enrich with Work API
/// 3. Format message
/// 4. Send to C2S
/// 5. Store in database
pub async fn enrich_and_send_workflow(
    lead_id: &str,
    customer_name: &str,
    phone: Option<&str>,
    email: Option<&str>,
    db: &PgPool,
    config: &Config,
    gateway_client: Option<&C2sGatewayClient>,
) -> Result<EnrichmentResult, AppError> {
    tracing::info!("Starting enrichment workflow for lead_id: {}", lead_id);

    // Step 1: Find CPF(s) via Diretrix
    tracing::info!("Step 1: Finding CPF via Diretrix");
    let cpf_result = find_cpf_via_diretrix(phone, email, config).await?;
    tracing::info!(
        "Found {} CPF(s), same_person: {}",
        cpf_result.cpfs.len(),
        cpf_result.same_person
    );

    // Step 2: Enrich with Work API
    tracing::info!(
        "Step 2: Enriching {} CPF(s) with Work API",
        cpf_result.cpfs.len()
    );
    let enriched_data = enrich_cpfs_with_work_api(&cpf_result.cpfs, config).await?;

    // Step 3: Format message
    tracing::info!("Step 3: Formatting enriched message");
    let message_body = format_enriched_message_body(
        customer_name,
        phone.unwrap_or(""),
        email.unwrap_or(""),
        &enriched_data,
        cpf_result.same_person,
    );

    // Step 4: Send to C2S
    tracing::info!(
        "Step 4: Sending message to C2S (length: {} chars)",
        message_body.len()
    );
    send_message_to_c2s(lead_id, &message_body, gateway_client, config).await?;

    // Step 5: Store in database
    tracing::info!(
        "Step 5: Storing {} person(s) in database",
        cpf_result.cpfs.len()
    );
    let stored_entity_ids =
        store_enriched_data(db, &cpf_result.cpfs, &enriched_data, Some(lead_id)).await?;

    Ok(EnrichmentResult {
        lead_id: lead_id.to_string(),
        cpfs_enriched: cpf_result.cpfs.clone(),
        same_person: cpf_result.same_person,
        message_sent: true,
        stored_count: stored_entity_ids.len(),
        entity_ids: stored_entity_ids,
    })
}

/// Result of enrichment workflow
#[derive(Debug)]
pub struct EnrichmentResult {
    pub lead_id: String,
    pub cpfs_enriched: Vec<String>,
    pub same_person: bool,
    pub message_sent: bool,
    pub stored_count: usize,
    pub entity_ids: Vec<uuid::Uuid>,
}

impl EnrichmentResult {
    pub fn to_json(&self) -> Value {
        json!({
            "success": true,
            "lead_id": self.lead_id,
            "enriched": true,
            "cpfs_enriched": self.cpfs_enriched,
            "same_person": self.same_person,
            "message_sent": self.message_sent,
            "stored_in_db": self.stored_count,
            "entity_ids": self.entity_ids,
        })
    }
}
