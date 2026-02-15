//! Noble/premium neighborhood lookup for São Paulo and Rio de Janeiro.
//! Port of ts:src/utils/neighborhoods.ts

use std::collections::HashSet;
use std::sync::LazyLock;

static SP_NOBLE: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // Jardins region
        "jardim europa", "jardim america", "jardim paulista", "jardim paulistano", "jardins",
        // Itaim / Vila Nova
        "itaim bibi", "itaim", "vila nova conceicao", "vila nova conceição",
        // Moema / Vila Olímpia
        "moema", "vila olimpia", "vila olímpia",
        // Pinheiros region
        "pinheiros", "alto de pinheiros", "alto pinheiros",
        // Higienópolis / Perdizes
        "higienopolis", "higienópolis", "perdizes", "pacaembu",
        // Morumbi region
        "morumbi", "cidade jardim", "real parque",
        // Other premium
        "brooklin", "brooklin novo", "campo belo", "vila mariana",
        "paraiso", "paraíso", "consolacao", "consolação",
        "cerqueira cesar", "cerqueira césar", "bela vista",
        // Zona oeste
        "butanta", "butantã",
        // Alphaville
        "alphaville", "tambore", "tamboré",
    ].into_iter().collect()
});

static RJ_NOBLE: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "leblon", "ipanema", "gavea", "gávea",
        "jardim botanico", "jardim botânico", "lagoa",
        "humaita", "humaitá", "botafogo", "flamengo",
        "laranjeiras", "cosme velho", "urca",
        "copacabana", "leme", "barra da tijuca",
        "sao conrado", "são conrado", "joatinga",
    ].into_iter().collect()
});

/// Check if a neighborhood is noble/premium.
pub fn is_noble_neighborhood(neighborhood: &str) -> bool {
    if neighborhood.is_empty() {
        return false;
    }
    let normalized = neighborhood.to_lowercase();
    let trimmed = normalized.trim();
    SP_NOBLE.contains(trimmed) || RJ_NOBLE.contains(trimmed)
}

/// Find the first noble neighborhood mentioned in an address string.
pub fn find_noble_neighborhood(address: &str) -> Option<&'static str> {
    if address.is_empty() {
        return None;
    }
    let normalized = address.to_lowercase();
    for &n in SP_NOBLE.iter().chain(RJ_NOBLE.iter()) {
        if normalized.contains(n) {
            return Some(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noble_neighborhoods() {
        assert!(is_noble_neighborhood("Jardim Europa"));
        assert!(is_noble_neighborhood("ITAIM BIBI"));
        assert!(is_noble_neighborhood("leblon"));
        assert!(!is_noble_neighborhood("Bairro Comum"));
        assert!(!is_noble_neighborhood(""));
    }

    #[test]
    fn test_find_noble() {
        assert!(find_noble_neighborhood("Rua X, Jardim Europa, SP").is_some());
        assert!(find_noble_neighborhood("Rua Y, Centro, SP").is_none());
    }
}
