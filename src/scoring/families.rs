//! Notable family and rare surname detection.
//! Port of ts:src/utils/surname-analyzer.ts (family/surname parts only).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Notable Brazilian families: surname → context.
static NOTABLE_FAMILIES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        // Banking/Finance
        ("rudge", "Família bancária de SP (ex-VP Itaú)"),
        ("safra", "Família do Banco Safra"),
        ("setúbal", "Família fundadora do Itaú"),
        ("setubal", "Família fundadora do Itaú"),
        ("villela", "Acionistas do Itaú Unibanco"),
        ("simonsen", "Economistas e empresários"),
        ("lemann", "3G Capital / AB InBev"),
        ("sicupira", "3G Capital"),
        // Media
        ("marinho", "Organizações Globo"),
        ("civita", "Grupo Abril"),
        ("frias", "Grupo Folha"),
        ("mesquita", "O Estado de S. Paulo"),
        // Industrial
        ("ermírio", "Grupo Votorantim"),
        ("steinbruch", "CSN"),
        ("gerdau", "Grupo Gerdau"),
        ("odebrecht", "Construtora Odebrecht"),
        ("johannpeter", "Grupo Gerdau"),
        // Retail
        ("trajano", "Magazine Luiza"),
        ("feffer", "Suzano Papel e Celulose"),
        // Real Estate
        ("horn", "Cyrela / Lindenberg"),
        ("lindenberg", "Lindenberg Construtora"),
        ("safdie", "Safdie Construtora"),
        ("zarzur", "EZTEC"),
        ("auriemo", "JHSF"),
        ("nigri", "Tecnisa"),
        // Health
        ("moll", "Rede D'Or São Luiz"),
        // Finance/Tech
        ("vélez", "Nubank"),
        ("velez", "Nubank"),
        // Industry
        ("klabin", "Klabin S.A."),
        ("lafer", "Industriais e políticos"),
        ("mindlin", "Brasilpar / colecionador"),
        ("ometto", "Cosan / Raízen"),
    ].into_iter().collect()
});

/// Surnames too common to flag as notable even if in the map.
static TOO_COMMON_FOR_NOTABLE: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "camargo", "andrade", "batista", "diniz", "moreira", "bueno",
        "constantino", "amaro", "torre", "klein", "trajano", "telles",
        "esteves", "maggi",
    ].into_iter().collect()
});

/// Rare surnames (non-Brazilian origin or uncommon).
static RARE_SURNAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // Italian
        "passafaro", "falabella", "trussardi", "ferragamo", "agnelli",
        // German
        "rosenbauer", "rothschild", "krupp", "porsche", "quandt",
        // Arab/Lebanese
        "khoury", "haddad", "jafet", "maluf", "kassab", "gebara",
        // Japanese
        "yamazaki", "nakashima", "watanabe", "fujimori",
        // Korean
        "kim", "park", "choi", "kang", "yoon",
        // Chinese
        "wang", "zhang", "chen", "huang", "wong",
        // Indian
        "patel", "sharma", "ambani", "tata", "mittal",
        // Jewish
        "cohen", "levy", "goldberg", "rosenberg", "steinberg", "friedman",
        // Other
        "rabello", "penteado", "buarque",
    ].into_iter().collect()
});

/// Common Brazilian surnames.
static COMMON_SURNAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "silva", "santos", "oliveira", "souza", "sousa", "lima", "pereira",
        "costa", "rodrigues", "almeida", "nascimento", "ferreira", "araújo",
        "araujo", "carvalho", "gomes", "martins", "rocha", "ribeiro", "alves",
        "monteiro", "mendes", "barros", "freitas", "barbosa", "pinto", "moura",
        "cavalcanti", "dias", "castro", "campos", "cardoso", "andrade", "vieira",
        "moreira", "nunes", "lopes", "fernandes", "ramos", "gonçalves", "machado",
        "marques", "melo", "correia", "azevedo", "teixeira", "batista",
    ].into_iter().collect()
});

#[derive(Debug, Clone)]
pub struct SurnameAnalysis {
    pub surname: String,
    pub is_rare: bool,
    pub is_notable_family: bool,
    pub family_context: Option<String>,
    pub confidence: u32,
}

/// Extract surnames from a full name (skip prepositions).
pub fn extract_surnames(full_name: &str) -> Vec<String> {
    let prepositions: HashSet<&str> = ["de", "da", "do", "das", "dos", "e"].into_iter().collect();
    full_name
        .to_lowercase()
        .split_whitespace()
        .skip(1) // skip first name
        .filter(|p| !prepositions.contains(p) && p.len() > 2)
        .map(String::from)
        .collect()
}

/// Analyze a single surname.
pub fn analyze_surname(surname: &str) -> SurnameAnalysis {
    let normalized = surname.to_lowercase();
    let trimmed = normalized.trim();

    // Notable family (excluding too-common)
    if let Some(&context) = NOTABLE_FAMILIES.get(trimmed) {
        if !TOO_COMMON_FOR_NOTABLE.contains(trimmed) {
            return SurnameAnalysis {
                surname: trimmed.to_string(),
                is_rare: true,
                is_notable_family: true,
                family_context: Some(context.to_string()),
                confidence: 95,
            };
        }
    }

    if RARE_SURNAMES.contains(trimmed) {
        return SurnameAnalysis {
            surname: trimmed.to_string(),
            is_rare: true,
            is_notable_family: false,
            family_context: None,
            confidence: 80,
        };
    }

    if COMMON_SURNAMES.contains(trimmed) {
        return SurnameAnalysis {
            surname: trimmed.to_string(),
            is_rare: false,
            is_notable_family: false,
            family_context: None,
            confidence: 100,
        };
    }

    // Unknown — heuristic
    let is_likely_rare = trimmed.len() > 10 || (trimmed.len() < 5 && trimmed.len() > 2);
    SurnameAnalysis {
        surname: trimmed.to_string(),
        is_rare: is_likely_rare,
        is_notable_family: false,
        family_context: None,
        confidence: if is_likely_rare { 50 } else { 30 },
    }
}

/// Analyze all surnames in a full name.
pub fn analyze_full_name(full_name: &str) -> Vec<SurnameAnalysis> {
    extract_surnames(full_name)
        .iter()
        .map(|s| analyze_surname(s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notable_family() {
        let analysis = analyze_surname("Safra");
        assert!(analysis.is_notable_family);
        assert!(analysis.family_context.is_some());
        assert_eq!(analysis.confidence, 95);
    }

    #[test]
    fn test_too_common_notable() {
        let analysis = analyze_surname("trajano");
        // trajano is in NOTABLE_FAMILIES but also in TOO_COMMON
        assert!(!analysis.is_notable_family);
    }

    #[test]
    fn test_rare_surname() {
        let analysis = analyze_surname("rothschild");
        assert!(analysis.is_rare);
        assert!(!analysis.is_notable_family);
        assert_eq!(analysis.confidence, 80);
    }

    #[test]
    fn test_common_surname() {
        let analysis = analyze_surname("Silva");
        assert!(!analysis.is_rare);
        assert!(!analysis.is_notable_family);
        assert_eq!(analysis.confidence, 100);
    }

    #[test]
    fn test_extract_surnames() {
        let surnames = extract_surnames("Ronald Leite Soares");
        assert_eq!(surnames, vec!["leite", "soares"]);
    }

    #[test]
    fn test_extract_with_prepositions() {
        let surnames = extract_surnames("Maria de Souza Lima");
        assert_eq!(surnames, vec!["souza", "lima"]);
    }
}
