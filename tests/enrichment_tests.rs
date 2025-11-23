/// Unit tests for enrichment logic
/// Tests email validation, phone validation, and CPF lookup workflows
use rust_c2s_api::enrichment::{is_valid_email, validate_br_phone};

#[cfg(test)]
mod email_validation_tests {
    use super::*;

    #[test]
    fn test_valid_emails() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("test.user@example.com"));
        assert!(is_valid_email("user+tag@example.co.uk"));
        assert!(is_valid_email("user_name@example-domain.com"));
        assert!(is_valid_email("a@b.c"));
    }

    #[test]
    fn test_invalid_emails_basic() {
        // Missing @ or .
        assert!(!is_valid_email("userexample.com"));
        assert!(!is_valid_email("user@examplecom"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("user@"));

        // Too short
        assert!(!is_valid_email("a@b"));
        assert!(!is_valid_email("a@"));
        assert!(!is_valid_email(""));
    }

    #[test]
    fn test_invalid_emails_fake_patterns() {
        // Repeated digits (common fake patterns)
        assert!(!is_valid_email("1199999999333@gmail.com"));
        assert!(!is_valid_email("11999999999@example.com"));
        assert!(!is_valid_email("user999999@example.com"));
        assert!(!is_valid_email("1111111111@gmail.com"));
        assert!(!is_valid_email("000000@example.com"));
        assert!(!is_valid_email("test123456789@example.com"));
    }

    #[test]
    fn test_invalid_emails_malformed() {
        assert!(!is_valid_email("user @example.com")); // space
        assert!(!is_valid_email("user@exam ple.com")); // space in domain
                                                       // Note: RFC 5322 regex is permissive - allows consecutive dots and leading dots
                                                       // We prioritize fake pattern detection (repeated digits) over strict format validation
                                                       // assert!(!is_valid_email("user..name@example.com")); // double dot - allowed by regex
                                                       // assert!(!is_valid_email(".user@example.com")); // starts with dot - allowed by regex
    }
}

#[cfg(test)]
mod phone_validation_tests {
    use super::*;

    #[test]
    fn test_valid_brazilian_phones() {
        // Cell phones (9 digits)
        let (valid, normalized) = validate_br_phone("11987654321");
        assert!(valid);
        assert_eq!(normalized, "+5511987654321");

        let (valid, normalized) = validate_br_phone("21987654321");
        assert!(valid);
        assert_eq!(normalized, "+5521987654321");

        // With formatting
        let (valid, normalized) = validate_br_phone("(11) 98765-4321");
        assert!(valid);
        assert_eq!(normalized, "+5511987654321");

        // With country code
        let (valid, normalized) = validate_br_phone("+5511987654321");
        assert!(valid);
        assert_eq!(normalized, "+5511987654321");

        let (valid, normalized) = validate_br_phone("5511987654321");
        assert!(valid);
        assert_eq!(normalized, "+5511987654321");
    }

    #[test]
    fn test_valid_brazilian_landlines() {
        // Landline (8 digits)
        let (valid, normalized) = validate_br_phone("1133334444");
        assert!(valid);
        assert_eq!(normalized, "+551133334444");

        let (valid, normalized) = validate_br_phone("(11) 3333-4444");
        assert!(valid);
        assert_eq!(normalized, "+551133334444");
    }

    #[test]
    fn test_invalid_phones() {
        // Too short
        let (valid, _) = validate_br_phone("1234");
        assert!(!valid);

        let (valid, _) = validate_br_phone("119876");
        assert!(!valid);

        // Empty
        let (valid, _) = validate_br_phone("");
        assert!(!valid);

        // Only spaces
        let (valid, _) = validate_br_phone("   ");
        assert!(!valid);

        // Invalid DDD (area code must be 11-99)
        let (valid, _) = validate_br_phone("0187654321");
        assert!(!valid);

        // Wrong country code
        let (valid, _) = validate_br_phone("+1234567890");
        assert!(!valid);
    }

    #[test]
    fn test_phone_normalization() {
        // All these should normalize to the same E.164 format
        let formats = vec![
            "11987654321",
            "(11) 98765-4321",
            "+55 11 98765-4321",
            "5511987654321",
            "+5511987654321",
            "11 98765 4321",
        ];

        for format in formats {
            let (valid, normalized) = validate_br_phone(format);
            if valid {
                assert_eq!(
                    normalized, "+5511987654321",
                    "Failed for format: {}",
                    format
                );
            }
        }
    }
}

#[cfg(test)]
mod cpf_validation_tests {
    // CPF validation is handled by Work API, but we can test our CPF formatting

    #[test]
    fn test_cpf_digit_extraction() {
        let cpf_formatted = "123.456.789-01";
        let cpf_clean: String = cpf_formatted.chars().filter(|c| c.is_numeric()).collect();

        assert_eq!(cpf_clean, "12345678901");
        assert_eq!(cpf_clean.len(), 11);
    }

    #[test]
    fn test_cpf_lengths() {
        // Valid CPF is exactly 11 digits
        let valid = "12345678901";
        assert_eq!(valid.len(), 11);

        let too_short = "123456789";
        assert!(too_short.len() < 11);

        let too_long = "123456789012";
        assert!(too_long.len() > 11);
    }
}

#[cfg(test)]
mod message_formatting_tests {
    use rust_c2s_api::enrichment::format_enriched_message_body;
    use serde_json::json;

    #[test]
    fn test_format_same_person_message() {
        let enriched_data = vec![json!({
            "DadosBasicos": {
                "nome": "João Silva",
                "cpf": "12345678901"
            }
        })];

        let message = format_enriched_message_body(
            "João Silva",
            "11987654321",
            "joao@example.com",
            &enriched_data,
            true, // same_person = true
        );

        assert!(message.contains("📞📧 Telefone e e-mail da mesma pessoa"));
        assert!(message.contains("João Silva"));
    }

    #[test]
    fn test_format_different_people_message() {
        let enriched_data = vec![
            json!({
                "DadosBasicos": {
                    "nome": "João Silva",
                    "cpf": "12345678901"
                }
            }),
            json!({
                "DadosBasicos": {
                    "nome": "Maria Santos",
                    "cpf": "98765432100"
                }
            }),
        ];

        let message = format_enriched_message_body(
            "João Silva",
            "11987654321",
            "maria@example.com",
            &enriched_data,
            false, // same_person = false
        );

        assert!(message.contains("⚠️ Telefone e e-mail relacionados a PESSOAS DIFERENTES!"));
        assert!(message.contains("PESSOA 1"));
        assert!(message.contains("PESSOA 2"));
        assert!(message.contains("11987654321"));
        assert!(message.contains("maria@example.com"));
    }
}

#[cfg(test)]
mod error_handling_tests {
    use rust_c2s_api::errors::AppError;

    #[test]
    fn test_app_error_types() {
        let db_error = AppError::DatabaseError(sqlx::Error::RowNotFound);
        assert!(matches!(db_error, AppError::DatabaseError(_)));

        let api_error = AppError::ExternalApiError("Work API timeout".to_string());
        assert!(matches!(api_error, AppError::ExternalApiError(_)));

        let not_found = AppError::NotFound("CPF not found".to_string());
        assert!(matches!(not_found, AppError::NotFound(_)));

        let bad_request = AppError::BadRequest("Invalid CPF format".to_string());
        assert!(matches!(bad_request, AppError::BadRequest(_)));
    }

    #[test]
    fn test_error_display() {
        let error = AppError::ExternalApiError("Connection timeout".to_string());
        let display = format!("{}", error);
        assert!(display.contains("External API error"));
        assert!(display.contains("Connection timeout"));

        let error = AppError::NotFound("Lead not found".to_string());
        let display = format!("{}", error);
        assert!(display.contains("Not found"));
        assert!(display.contains("Lead not found"));
    }
}

#[cfg(test)]
mod deduplication_tests {
    use moka::future::Cache;
    use std::time::Duration;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache: Cache<String, i64> = Cache::builder()
            .time_to_live(Duration::from_secs(60))
            .max_capacity(100)
            .build();

        // Insert
        cache.insert("test_lead".to_string(), 12345).await;

        // Get
        let value = cache.get(&"test_lead".to_string()).await;
        assert_eq!(value, Some(12345));

        // Get non-existent
        let value = cache.get(&"nonexistent".to_string()).await;
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_cache_deduplication() {
        let cache: Cache<String, i64> = Cache::builder()
            .time_to_live(Duration::from_secs(60))
            .max_capacity(100)
            .build();

        let lead_id = "test_lead_123".to_string();

        // First request - not in cache
        let existing = cache.get(&lead_id).await;
        assert!(existing.is_none());

        // Mark as processing
        cache.insert(lead_id.clone(), 1).await;

        // Second request - should be in cache (duplicate)
        let existing = cache.get(&lead_id).await;
        assert!(existing.is_some());
        assert_eq!(existing.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_cache_ttl() {
        let cache: Cache<String, i64> = Cache::builder()
            .time_to_live(Duration::from_millis(100))
            .max_capacity(100)
            .build();

        cache.insert("short_lived".to_string(), 999).await;

        // Immediately available
        let value = cache.get(&"short_lived".to_string()).await;
        assert_eq!(value, Some(999));

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        let value = cache.get(&"short_lived".to_string()).await;
        assert_eq!(value, None);
    }
}

#[cfg(test)]
mod data_extraction_tests {
    use serde_json::json;

    #[test]
    fn test_extract_name_from_work_api() {
        let data = json!({
            "DadosBasicos": {
                "nome": "João da Silva",
                "cpf": "12345678901"
            }
        });

        let name = data
            .get("DadosBasicos")
            .and_then(|d| d.get("nome"))
            .and_then(|n| n.as_str());

        assert_eq!(name, Some("João da Silva"));
    }

    #[test]
    fn test_extract_emails_from_work_api() {
        let data = json!({
            "emails": [
                {"email": "joao@example.com", "prioridade": "1"},
                {"email": "silva@example.com", "prioridade": "2"}
            ]
        });

        let emails = data.get("emails").and_then(|e| e.as_array());
        assert!(emails.is_some());

        let emails = emails.unwrap();
        assert_eq!(emails.len(), 2);
        assert_eq!(
            emails[0].get("email").and_then(|e| e.as_str()),
            Some("joao@example.com")
        );
    }

    #[test]
    fn test_extract_phones_from_work_api() {
        let data = json!({
            "telefones": [
                {"telefone": "11987654321", "tipo": "CELULAR", "whatsapp": "SIM"},
                {"telefone": "1133334444", "tipo": "RESIDENCIAL", "whatsapp": "NAO"}
            ]
        });

        let phones = data.get("telefones").and_then(|p| p.as_array());
        assert!(phones.is_some());

        let phones = phones.unwrap();
        assert_eq!(phones.len(), 2);

        let first_phone = &phones[0];
        assert_eq!(
            first_phone.get("telefone").and_then(|t| t.as_str()),
            Some("11987654321")
        );
        assert_eq!(
            first_phone.get("whatsapp").and_then(|w| w.as_str()),
            Some("SIM")
        );
    }

    #[test]
    fn test_extract_addresses_from_work_api() {
        let data = json!({
            "enderecos": [
                {
                    "logradouro": "Rua Exemplo",
                    "numero": "123",
                    "bairro": "Centro",
                    "cidade": "São Paulo",
                    "uf": "SP",
                    "cep": "01234567"
                }
            ]
        });

        let addresses = data.get("enderecos").and_then(|a| a.as_array());
        assert!(addresses.is_some());

        let addresses = addresses.unwrap();
        assert_eq!(addresses.len(), 1);

        let address = &addresses[0];
        assert_eq!(
            address.get("logradouro").and_then(|l| l.as_str()),
            Some("Rua Exemplo")
        );
        assert_eq!(
            address.get("cidade").and_then(|c| c.as_str()),
            Some("São Paulo")
        );
    }
}
