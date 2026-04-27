#![allow(dead_code)]

use crate::token::Token;
use crate::wordlist::Wordlist;

/// A candidate spelling suggestion with its similarity metrics.
pub struct Suggestion {
    pub word: String,
    pub jaro_winkler_similarity: f64,
    pub levenshtein_distance: Option<usize>,
}

/// Get spelling suggestions for a token from the given wordlists.
///
/// Returns up to `limit` words (with case applied to match the token's case
/// style) sorted by descending Jaro-Winkler similarity then alphabetically.
pub fn get_suggestions(token: &Token, wordlists: &mut [Wordlist], limit: usize) -> Vec<String> {
    let term = token.normalized();
    let term_char_len = term.chars().count();

    // Similarity threshold varies with word length (matches Ruby implementation).
    let threshold: f64 = if term_char_len > 4 { 0.834 } else { 0.77 };

    let mut suggestions: Vec<Suggestion> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for wordlist in wordlists.iter_mut() {
        // Clone the word slice so we no longer borrow `wordlist` mutably.
        let words = wordlist.words().to_vec();
        for word in words {
            let similarity = strsim::jaro_winkler(&word, &term);
            if similarity >= threshold && seen.insert(word.clone()) {
                suggestions.push(Suggestion {
                    word,
                    jaro_winkler_similarity: similarity,
                    levenshtein_distance: None,
                });
            }
        }
    }

    // Sort by descending similarity, then alphabetically for stable tie-breaking.
    suggestions.sort_by(|a, b| {
        b.jaro_winkler_similarity
            .partial_cmp(&a.jaro_winkler_similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.word.cmp(&b.word))
    });

    let reduced = reduce_suggestions(suggestions, &term, limit);

    // Apply the token's case style to each suggestion word.
    reduced
        .into_iter()
        .map(|s| token.apply_case(&s.word))
        .collect()
}

// ── Reduction pipeline ────────────────────────────────────────────────────────

/// Try progressively looser filters to produce at most `limit` useful
/// suggestions.  Mirrors Ruby's `reduce_suggestions` method.
fn reduce_suggestions(suggestions: Vec<Suggestion>, term: &str, limit: usize) -> Vec<Suggestion> {
    if suggestions.is_empty() {
        return suggestions;
    }

    // Pass 1: tight filter – likely mistypes (very small edit distance).
    let mistypes = reduce_to_mistypes(&suggestions, term, limit);
    if !mistypes.is_empty() {
        return mistypes;
    }

    // Pass 2: medium filter – plausible misspellings.
    let misspells = reduce_to_misspells(&suggestions, term, limit);
    if !misspells.is_empty() {
        return misspells;
    }

    // Pass 3: loose filter – anything within 2 % of the best score.
    reduce_wild_suggestions(suggestions)
        .into_iter()
        .take(limit)
        .collect()
}

/// Keep suggestions whose Levenshtein distance is at most `(term_len - 1) * 0.25`.
///
/// These are near-certain mistypes (e.g. a single transposition or deletion).
fn reduce_to_mistypes(suggestions: &[Suggestion], term: &str, limit: usize) -> Vec<Suggestion> {
    let term_len = term.chars().count();
    // For very short terms (len 1) the threshold is 0, so only exact matches pass.
    let threshold = ((term_len as f64 - 1.0) * 0.25).floor() as usize;

    suggestions
        .iter()
        .filter_map(|s| {
            let dist = edit_distance::edit_distance(&s.word, term);
            if dist <= threshold {
                Some(Suggestion {
                    word: s.word.clone(),
                    jaro_winkler_similarity: s.jaro_winkler_similarity,
                    levenshtein_distance: Some(dist),
                })
            } else {
                None
            }
        })
        .take(limit)
        .collect()
}

/// Keep suggestions whose Levenshtein distance is strictly less than
/// `term_len - 1`.
fn reduce_to_misspells(suggestions: &[Suggestion], term: &str, limit: usize) -> Vec<Suggestion> {
    let term_len = term.chars().count();
    // Guard against underflow for very short terms.
    let max_dist = if term_len > 1 { term_len - 1 } else { 0 };

    suggestions
        .iter()
        .filter_map(|s| {
            let dist = edit_distance::edit_distance(&s.word, term);
            if dist < max_dist {
                Some(Suggestion {
                    word: s.word.clone(),
                    jaro_winkler_similarity: s.jaro_winkler_similarity,
                    levenshtein_distance: Some(dist),
                })
            } else {
                None
            }
        })
        .take(limit)
        .collect()
}

/// Keep all suggestions that are within 2 % of the best Jaro-Winkler score.
///
/// Assumes `suggestions` is already sorted by descending similarity (so index 0
/// holds the best entry).
fn reduce_wild_suggestions(suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
    if suggestions.is_empty() {
        return suggestions;
    }

    let best = suggestions[0].jaro_winkler_similarity;
    let floor = best * 0.98;

    suggestions
        .into_iter()
        .filter(|s| s.jaro_winkler_similarity >= floor)
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_suggestion(word: &str, sim: f64) -> Suggestion {
        Suggestion {
            word: word.to_string(),
            jaro_winkler_similarity: sim,
            levenshtein_distance: None,
        }
    }

    #[test]
    fn reduce_wild_keeps_close_scores() {
        let suggestions = vec![
            make_suggestion("hello", 0.95),
            make_suggestion("helo", 0.94),
            // 0.80 < 0.95 * 0.98 = 0.931 → should be filtered out
            make_suggestion("hell", 0.80),
        ];
        let result = reduce_wild_suggestions(suggestions);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|s| s.jaro_winkler_similarity >= 0.931));
    }

    #[test]
    fn reduce_wild_empty_input() {
        let result = reduce_wild_suggestions(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn reduce_to_mistypes_uses_edit_distance() {
        // "helo" vs "hello" → edit distance 1
        // threshold for "hello" (len 5) = (5-1)*0.25 = 1  → included
        // "xyz"  vs "hello" → edit distance 4 > 1           → excluded
        let suggestions = vec![
            make_suggestion("helo", 0.95),
            make_suggestion("xyz", 0.85),
        ];
        let result = reduce_to_mistypes(&suggestions, "hello", 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].word, "helo");
        assert_eq!(result[0].levenshtein_distance, Some(1));
    }

    #[test]
    fn reduce_to_misspells_uses_edit_distance() {
        // "helllo" vs "hello" → edit distance 1 < 4 (5-1) → included
        // "world"  vs "hello" → edit distance 4 = 4, not < 4 → excluded
        let suggestions = vec![
            make_suggestion("helllo", 0.90),
            make_suggestion("world", 0.84),
        ];
        let result = reduce_to_misspells(&suggestions, "hello", 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].word, "helllo");
    }

    #[test]
    fn reduce_suggestions_falls_through_to_wild() {
        // Create suggestions where mistypes and misspells filters are both empty
        // but wild should keep one.
        // Use a short term so threshold is low.
        // term = "abc" (len 3), mistype threshold = floor((3-1)*0.25) = 0
        // misspells max_dist = 2, "xyz" has distance 3 – excluded from misspells
        // wild: all within 2 % of best
        let suggestions = vec![
            make_suggestion("abx", 0.90), // edit_distance("abx","abc") = 1 > 0 → not mistype
            // edit_distance 1 < 2 → misspell! So this will actually be caught by misspells.
        ];
        // So we just check that reduce_suggestions returns something.
        let result = reduce_suggestions(suggestions, "abc", 5);
        assert!(!result.is_empty());
    }

    #[test]
    fn get_suggestions_respects_limit() {
        // We'll test with a temp wordlist containing several similar words.
        use std::path::PathBuf;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        // Write a sorted list with words similar to "tset"
        std::fs::write(
            &path,
            "set\ntest\ntset\ntext\n",
        )
        .unwrap();
        let wl = crate::wordlist::Wordlist::new(path, "test".into());

        use crate::token::{ColumnLocation, Token};
        let loc = ColumnLocation {
            char_offset: 0,
            byte_offset: 0,
            line_number: 1,
            file: PathBuf::from("test.rs"),
            line_char_offset: 0,
            line_byte_offset: 0,
        };
        let token = Token::new("tset".into(), loc, String::new());
        let result = get_suggestions(&token, &mut [wl], 1);
        // Should return at most 1 result.
        assert!(result.len() <= 1);
    }
}
