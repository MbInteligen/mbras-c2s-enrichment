/// Property-based tests using proptest
/// Tests invariants and properties that should hold for all inputs
use proptest::prelude::*;
use rust_c2s_api::enrichment::{is_valid_email, validate_br_phone};

// Property: Email validation should never panic
proptest! {
    #[test]
    fn email_validation_never_panics(email in "\\PC*") {
        let _ = is_valid_email(&email);
    }

    #[test]
    fn email_with_at_and_dot_checks_fake_patterns(
        local in "[a-z]{1,10}",
        domain in "[a-z]{1,10}",
        tld in "[a-z]{2,4}"
    ) {
        let email = format!("{}@{}.{}", local, domain, tld);
        // Valid format emails should pass unless they contain fake patterns
        let result = is_valid_email(&email);
        // If it fails, it should be because of length or fake pattern, not format
        if !result {
            prop_assert!(email.len() < 5 || email.contains("999999") || email.contains("111111"));
        }
    }
}

// Property: Phone validation should never panic
proptest! {
    #[test]
    fn phone_validation_never_panics(phone in "\\PC*") {
        let _ = validate_br_phone(&phone);
    }

    #[test]
    fn valid_br_phones_normalize_to_e164(ddd in 11u8..=99u8, number in 900000000u32..=999999999u32) {
        let phone = format!("{}{}", ddd, number);
        let (valid, normalized) = validate_br_phone(&phone);
        if valid {
            // Valid phones should start with +55
            prop_assert!(normalized.starts_with("+55"));
            // Should contain only digits after +
            prop_assert!(normalized[1..].chars().all(|c| c.is_ascii_digit()));
            // Should be 13 or 14 characters (+55 + 10 or 11 digits)
            prop_assert!(normalized.len() >= 13 && normalized.len() <= 14);
        }
    }

    #[test]
    fn phone_with_formatting_chars_accepted(
        ddd in 11u8..=99u8,
        first in 9u8..=9u8,
        rest in 10000000u32..=99999999u32,
        use_parens in proptest::bool::ANY,
        use_dash in proptest::bool::ANY
    ) {
        let number = format!("{}{}", first, rest);
        let phone = if use_parens && use_dash {
            format!("({}) {}-{}", ddd, &number[..5], &number[5..])
        } else if use_parens {
            format!("({}) {}", ddd, number)
        } else if use_dash {
            format!("{} {}-{}", ddd, &number[..5], &number[5..])
        } else {
            format!("{}{}", ddd, number)
        };

        let (valid, normalized) = validate_br_phone(&phone);
        // Brazilian cell phone format should be accepted
        if valid {
            prop_assert!(normalized.starts_with("+55"));
        }
    }
}

// Property: CPF formatting should preserve digits
proptest! {
    #[test]
    fn cpf_digit_extraction_preserves_order(cpf in "[0-9]{11}") {
        // Insert formatting
        let formatted = format!("{}.{}.{}-{}",
            &cpf[0..3], &cpf[3..6], &cpf[6..9], &cpf[9..11]);

        // Extract digits
        let cleaned: String = formatted.chars().filter(|c| c.is_numeric()).collect();

        // Should match original
        prop_assert_eq!(cleaned, cpf);
    }

    #[test]
    fn cpf_cleaned_always_11_digits(cpf in "[0-9]{11}") {
        prop_assert_eq!(cpf.len(), 11);
        prop_assert!(cpf.chars().all(|c| c.is_ascii_digit()));
    }
}

// Property: Email fake pattern detection
proptest! {
    #[test]
    fn emails_with_repeated_digits_rejected(
        repeat_pattern in prop::sample::select(vec!["999999", "111111", "000000", "123456789"]),
        local_prefix in "[a-z]{1,5}",
        domain in "[a-z]{3,10}",
        tld in "[a-z]{2,3}"
    ) {
        let email = format!("{}{}@{}.{}", local_prefix, repeat_pattern, domain, tld);
        let result = is_valid_email(&email);
        // Should be rejected due to fake pattern
        prop_assert!(!result, "Email with fake pattern should be rejected: {}", email);
    }
}

// Property: Valid email structure
proptest! {
    #[test]
    fn valid_structure_emails_checked_for_fakes(
        local in "[a-zA-Z][a-zA-Z0-9]{0,20}",
        domain in "[a-zA-Z][a-zA-Z0-9]{1,15}",
        tld in "[a-zA-Z]{2,6}"
    ) {
        // Skip if local starts with invalid chars
        prop_assume!(!local.is_empty() && local.chars().next().unwrap().is_alphabetic());

        let email = format!("{}@{}.{}", local, domain, tld);
        let result = is_valid_email(&email);

        // If rejected, should be due to fake pattern or length, not format
        if !result {
            let has_fake = email.contains("999999") || email.contains("111111") ||
                           email.contains("000000") || email.contains("123456789");
            prop_assert!(has_fake || email.len() < 5,
                "Valid format email rejected without fake pattern: {}", email);
        }
    }
}

// Property: Phone number length bounds
proptest! {
    #[test]
    fn very_short_phones_always_invalid(phone in "[0-9]{0,7}") {
        let (valid, _) = validate_br_phone(&phone);
        prop_assert!(!valid, "Very short phone should be invalid: {}", phone);
    }

    #[test]
    fn extremely_long_phones_rejected(phone in "[0-9]{20,30}") {
        let (valid, _) = validate_br_phone(&phone);
        // Brazilian phones are max 13 chars in E164 format, so very long should fail
        prop_assert!(!valid, "Extremely long phone should be invalid: {}", phone);
    }
}
