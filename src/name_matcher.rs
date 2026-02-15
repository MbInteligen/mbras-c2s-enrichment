//! Smart name matching — fuzzy matching with Levenshtein distance,
//! abbreviation expansion, and initials detection.
//!
//! Port of ts-c2s-api `src/utils/name-matcher.ts`.

use std::collections::HashMap;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// Abbreviation expansions for Brazilian names.
static ABBREVIATIONS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("MA.", "MARIA");
    m.insert("M.", "MARIA");
    m.insert("JO.", "JOSE");
    m.insert("J.", "JOSE");
    m.insert("ANT.", "ANTONIO");
    m.insert("FCO.", "FRANCISCO");
    m.insert("DR.", "DOUTOR");
    m.insert("DRA.", "DOUTORA");
    m.insert("SR.", "SENHOR");
    m.insert("SRA.", "SENHORA");
    m.insert("S.", "SANTOS");
    m.insert("STO.", "SANTO");
    m.insert("STA.", "SANTA");
    m
});

/// Remove Unicode combining marks (accents) after NFD decomposition.
fn remove_accents(s: &str) -> String {
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

/// Expand abbreviations (e.g., "MA." → "MARIA").
fn expand_abbreviations(name: &str) -> String {
    let mut result = name.to_string();
    for (&abbr, &full) in ABBREVIATIONS.iter() {
        // Word-boundary aware replacement: match abbr at word start
        let pattern = abbr.replace('.', "\\.");
        if let Ok(re) = regex::Regex::new(&format!(r"(?i)\b{}", pattern)) {
            result = re.replace_all(&result, full).to_string();
        }
    }
    result
}

/// Normalize a name for comparison.
///
/// 1. to_uppercase
/// 2. Remove accents (NFD + strip combining marks)
/// 3. Expand abbreviations
/// 4. Remove suffixes (JUNIOR, JR, FILHO, etc.)
/// 5. Collapse whitespace + trim
pub fn normalize_name(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    let upper = remove_accents(&name.to_uppercase());
    let expanded = expand_abbreviations(&upper);

    // Remove suffixes
    let re = regex::Regex::new(r"\b(JUNIOR|JR\.?|FILHO|NETO|SOBRINHO|SEGUNDO|II|III)\b").unwrap();
    let without_suffix = re.replace_all(&expanded, "").to_string();

    // Collapse whitespace
    let collapsed = regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(without_suffix.trim(), " ");

    collapsed.to_string()
}

/// Compute Levenshtein edit distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_bytes: Vec<char> = a.chars().collect();
    let b_bytes: Vec<char> = b.chars().collect();
    let m = a_bytes.len();
    let n = b_bytes.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// Calculate string similarity using Levenshtein distance.
///
/// Returns a value between 0.0 (completely different) and 1.0 (identical).
pub fn calculate_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - levenshtein_distance(a, b) as f64 / max_len as f64
}

/// Extract first and last name parts.
fn key_parts(name: &str) -> (String, String) {
    let parts: Vec<&str> = name.split_whitespace().filter(|p| !p.is_empty()).collect();
    let first = parts.first().map(|s| s.to_string()).unwrap_or_default();
    let last = parts.last().map(|s| s.to_string()).unwrap_or_default();
    (first, last)
}

/// Result of a name matching operation.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub matches: bool,
    pub score: f64,
    pub method: String,
}

/// Match two names using multiple strategies.
///
/// Returns whether they match, the confidence score, and the method used.
/// Default threshold is 0.75 (can be overridden).
pub fn match_names(lead_name: &str, db_name: &str) -> MatchResult {
    match_names_with_threshold(lead_name, db_name, 0.75)
}

/// Match two names with a custom threshold.
pub fn match_names_with_threshold(lead_name: &str, db_name: &str, threshold: f64) -> MatchResult {
    let n_lead = normalize_name(lead_name);
    let n_db = normalize_name(db_name);

    // 1. Exact match
    if n_lead == n_db {
        return MatchResult {
            matches: true,
            score: 1.0,
            method: "exact".to_string(),
        };
    }

    // 2. Full fuzzy
    let full_sim = calculate_similarity(&n_lead, &n_db);
    if full_sim >= threshold {
        return MatchResult {
            matches: true,
            score: full_sim,
            method: "fuzzy-full".to_string(),
        };
    }

    let (l_first, l_last) = key_parts(&n_lead);
    let (d_first, d_last) = key_parts(&n_db);

    // 3. First name exact + last name fuzzy
    if l_first == d_first && l_first.len() >= 3 {
        // Single-word lead name → first-name-only
        if l_first == l_last {
            return MatchResult {
                matches: true,
                score: 0.85,
                method: "first-name-only".to_string(),
            };
        }
        let last_sim = calculate_similarity(&l_last, &d_last);
        if last_sim >= 0.6 {
            return MatchResult {
                matches: true,
                score: (1.0 + last_sim) / 2.0,
                method: "first-exact-last-fuzzy".to_string(),
            };
        }
    }

    // 4. Last name exact + first name fuzzy
    if l_last == d_last && l_last.len() >= 3 {
        let first_sim = calculate_similarity(&l_first, &d_first);
        if first_sim >= 0.6 {
            return MatchResult {
                matches: true,
                score: (1.0 + first_sim) / 2.0,
                method: "last-exact-first-fuzzy".to_string(),
            };
        }
    }

    // 5. Containment
    if n_lead.contains(&n_db) || n_db.contains(&n_lead) {
        let ratio =
            n_lead.len().min(n_db.len()) as f64 / n_lead.len().max(n_db.len()) as f64;
        if ratio >= 0.3 {
            return MatchResult {
                matches: true,
                score: 0.7 + ratio * 0.3,
                method: "contains".to_string(),
            };
        }
    }

    // 6. Abbreviation match (e.g., "MARIA S" vs "MARIA SILVA")
    if l_first == d_first {
        if (l_last.len() <= 2 && d_last.starts_with(&l_last))
            || (d_last.len() <= 2 && l_last.starts_with(&d_last))
        {
            return MatchResult {
                matches: true,
                score: 0.8,
                method: "abbreviation-match".to_string(),
            };
        }
    }

    // 7. Pure initials (e.g., "JP" → "JOAO PAULO")
    let db_words: Vec<&str> = n_db.split_whitespace().filter(|p| !p.is_empty()).collect();
    if n_lead.len() >= 2
        && n_lead.len() <= 3
        && n_lead.chars().all(|c| c.is_uppercase())
    {
        let initials: String = db_words.iter().map(|p| p.chars().next().unwrap_or(' ')).collect();
        if initials.starts_with(&n_lead) || n_lead == initials {
            return MatchResult {
                matches: true,
                score: 0.85,
                method: "initials-match".to_string(),
            };
        }
    }

    // 8. Initials + last name (e.g., "JP Demasi" → "JOAO PAULO BENEVIDES DEMASI")
    let lead_words: Vec<&str> = n_lead.split_whitespace().filter(|p| !p.is_empty()).collect();
    if lead_words.len() >= 2 {
        let maybe_initials = lead_words[0];
        if maybe_initials.len() >= 2 && maybe_initials.len() <= 3 {
            let db_initials: String = db_words
                .iter()
                .take(maybe_initials.len())
                .map(|p| p.chars().next().unwrap_or(' '))
                .collect();
            if let Some(lead_last) = lead_words.last() {
                if let Some(db_last) = db_words.last() {
                    let last_sim = calculate_similarity(lead_last, db_last);
                    if db_initials == maybe_initials && last_sim >= 0.7 {
                        return MatchResult {
                            matches: true,
                            score: 0.9,
                            method: "initials-lastname-match".to_string(),
                        };
                    }
                }
            }
        }
    }

    // No match
    MatchResult {
        matches: false,
        score: full_sim,
        method: "no-match".to_string(),
    }
}

/// Find the best matching candidate from a list.
///
/// Returns (name, cpf, score, method) of the best match above threshold,
/// or None if no candidate matches.
pub fn find_best_match(
    lead_name: &str,
    candidates: &[(String, String)], // (name, cpf)
    threshold: f64,
) -> Option<(String, String, f64, String)> {
    if lead_name.is_empty() || candidates.is_empty() {
        return None;
    }
    let mut best: Option<(String, String, f64, String)> = None;
    for (name, cpf) in candidates {
        let r = match_names_with_threshold(lead_name, name, threshold);
        if r.matches {
            if best.as_ref().map_or(true, |b| r.score > b.2) {
                best = Some((name.clone(), cpf.clone(), r.score, r.method));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_accents() {
        assert_eq!(normalize_name("José María"), "JOSE MARIA");
    }

    #[test]
    fn test_normalize_abbreviation() {
        assert_eq!(normalize_name("Ma. Silva"), "MARIA SILVA");
    }

    #[test]
    fn test_normalize_suffix() {
        assert_eq!(normalize_name("Antonio Junior"), "ANTONIO");
    }

    #[test]
    fn test_exact_match() {
        let r = match_names("João Silva", "João Silva");
        assert!(r.matches);
        assert_eq!(r.score, 1.0);
        assert_eq!(r.method, "exact");
    }

    #[test]
    fn test_fuzzy_match() {
        let r = match_names("Ronald Soares", "Ronaldo Soares");
        assert!(r.matches);
        assert!(r.score >= 0.85);
    }

    #[test]
    fn test_no_match() {
        let r = match_names("Carlos Ferreira", "Ana Borges Mendes");
        assert!(!r.matches);
    }

    #[test]
    fn test_first_name_only() {
        let r = match_names("Ana", "Ana Carolina Borges Mendes");
        assert!(r.matches);
        assert_eq!(r.method, "first-name-only");
        assert_eq!(r.score, 0.85);
    }
}
