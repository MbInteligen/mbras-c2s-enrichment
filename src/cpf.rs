//! CPF validation and normalization.
//!
//! Implements the Brazilian CPF mod-11 check digit algorithm.
//! Port of ts-c2s-api `WorkApiService.isValidCpf()`.

/// Normalize a CPF string: strip non-digits, take last 11 characters.
///
/// Handles Work API 14-digit format (leading zeros) and formatted input (dots/dash).
pub fn normalize_cpf(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 11 {
        digits[digits.len() - 11..].to_string()
    } else {
        digits
    }
}

/// Validate a CPF using the mod-11 check digit algorithm.
///
/// Input must be exactly 11 ASCII digits (call `normalize_cpf` first).
/// Rejects all-same-digit CPFs and invalid check digits.
pub fn is_valid_cpf(cpf: &str) -> bool {
    let bytes: Vec<u8> = cpf.bytes().collect();

    // Must be exactly 11 ASCII digits
    if bytes.len() != 11 || !bytes.iter().all(|b| b.is_ascii_digit()) {
        return false;
    }

    // Reject all-same-digit CPFs (e.g. "11111111111")
    if bytes.iter().all(|&b| b == bytes[0]) {
        return false;
    }

    let d = |i: usize| -> u32 { (bytes[i] - b'0') as u32 };

    // First check digit (position 9)
    let sum1: u32 = (0..9).map(|i| d(i) * (10 - i as u32)).sum();
    let mut d1 = 11 - (sum1 % 11);
    if d1 >= 10 {
        d1 = 0;
    }
    if d(9) != d1 {
        return false;
    }

    // Second check digit (position 10)
    let sum2: u32 = (0..10).map(|i| d(i) * (11 - i as u32)).sum();
    let mut d2 = 11 - (sum2 % 11);
    if d2 >= 10 {
        d2 = 0;
    }
    d(10) == d2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_cpf() {
        assert!(is_valid_cpf("52998224725"));
    }

    #[test]
    fn test_all_same_digits() {
        assert!(!is_valid_cpf("11111111111"));
        assert!(!is_valid_cpf("00000000000"));
    }

    #[test]
    fn test_invalid_check_digits() {
        assert!(!is_valid_cpf("12345678900"));
    }

    #[test]
    fn test_normalize_14_digit() {
        let normalized = normalize_cpf("00052998224725");
        assert_eq!(normalized, "52998224725");
        assert!(is_valid_cpf(&normalized));
    }

    #[test]
    fn test_normalize_formatted() {
        let normalized = normalize_cpf("529.982.247-25");
        assert_eq!(normalized, "52998224725");
        assert!(is_valid_cpf(&normalized));
    }

    #[test]
    fn test_leading_zero_cpf() {
        assert!(is_valid_cpf("01234567890"));
        assert!(is_valid_cpf("00000000191"));
    }

    #[test]
    fn test_short_input() {
        assert!(!is_valid_cpf("1234567"));
    }
}
