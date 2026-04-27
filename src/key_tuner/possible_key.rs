#![allow(dead_code)]

use std::collections::HashMap;

use crate::key_tuner::stats::{max_by, mean_by, variance_by};

// Characters tracked for letter-frequency features (matches Ruby FEATURE_LETTERS)
const FEATURE_CHARS: &[char] = &['+', '-', '_', '/', 'A', 'z', 'Z', 'q', 'Q', 'X', 'x'];

// The full BASE_64 alphabet used for letter counting
// (VOWELS + CONSONANTS + digits + special chars, matching the Ruby PossibleKey::BASE_64)
const VOWELS: &[char] = &['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U'];

const CONSONANTS: &[char] = &[
    'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'q', 'r', 's', 't', 'v', 'w',
    'x', 'y', 'z', 'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'S',
    'T', 'V', 'W', 'X', 'Y', 'Z',
];

// BASE_64 = VOWELS + CONSONANTS + digits + [- _ + / =]
fn base64_alphabet() -> Vec<char> {
    let mut v: Vec<char> = Vec::with_capacity(67);
    v.extend_from_slice(VOWELS);
    v.extend_from_slice(CONSONANTS);
    v.extend("0123456789-_+/=".chars());
    v
}

pub struct PossibleKey<'a> {
    pub string: &'a str,
}

impl<'a> PossibleKey<'a> {
    pub fn new(string: &'a str) -> Self {
        Self { string }
    }

    /// Compute the full feature map used by the Naive Bayes classifier.
    pub fn features(&self) -> HashMap<String, f64> {
        let lfd = self.letter_frequency_difference();

        let mut map = HashMap::new();

        // Per-character frequency-difference features
        for &ch in FEATURE_CHARS {
            let key = ch.to_string();
            let val = lfd.get(&ch).copied().unwrap_or(0.0);
            map.insert(key, val);
        }

        // Scalar features
        map.insert("equal".into(), self.letter_count().get(&'=').copied().unwrap_or(0) as f64);
        map.insert("length".into(), self.string.chars().count() as f64);
        map.insert(
            "hex".into(),
            if self.character_set() == Some("hex") { 1.0 } else { 0.0 },
        );
        map.insert(
            "lower36".into(),
            if self.character_set() == Some("lower36") { 1.0 } else { 0.0 },
        );
        map.insert(
            "upper36".into(),
            if self.character_set() == Some("upper36") { 1.0 } else { 0.0 },
        );
        map.insert(
            "base64".into(),
            if self.character_set() == Some("base64") { 1.0 } else { 0.0 },
        );

        let title  = self.title_chunks();
        let lower  = self.lower_chunks();
        let upper  = self.upper_chunks();
        let alpha  = self.alpha_chunks();
        let alnum  = self.alnum_chunks();
        let digits = self.digit_chunks();

        map.insert("mean_title_chunk_size".into(),     mean_by(&title,  |s| s.len() as f64));
        map.insert("variance_title_chunk_size".into(), variance_by(&title, |s| s.len() as f64));
        map.insert("max_title_chunk_size".into(),      max_by(&title,   |s| s.len() as f64));

        map.insert("mean_lower_chunk_size".into(),     mean_by(&lower,  |s| s.len() as f64));
        map.insert("variance_lower_chunk_size".into(), variance_by(&lower, |s| s.len() as f64));

        map.insert("mean_upper_chunk_size".into(),     mean_by(&upper,  |s| s.len() as f64));
        map.insert("variance_upper_chunk_size".into(), variance_by(&upper, |s| s.len() as f64));

        map.insert("mean_alpha_chunk_size".into(),     mean_by(&alpha,  |s| s.len() as f64));
        map.insert("variance_alpha_chunk_size".into(), variance_by(&alpha, |s| s.len() as f64));

        map.insert("mean_alnum_chunk_size".into(),     mean_by(&alnum,  |s| s.len() as f64));
        map.insert("variance_alnum_chunk_size".into(), variance_by(&alnum, |s| s.len() as f64));

        map.insert("mean_digit_chunk_size".into(),     mean_by(&digits, |s| s.len() as f64));
        map.insert("variance_digit_chunk_size".into(), variance_by(&digits, |s| s.len() as f64));

        map.insert("vowel_consonant_ratio".into(), self.vowel_consonant_ratio());

        map.insert("alpha_chunks".into(),  alpha.len()  as f64);
        map.insert("alnum_chunks".into(),  alnum.len()  as f64);
        map.insert("digit_chunks".into(),  digits.len() as f64);
        map.insert("title_chunks".into(),  title.len()  as f64);

        let lfd_values: Vec<f64> = lfd.values().copied().collect();
        map.insert(
            "mean_letter_frequency_difference".into(),
            crate::key_tuner::stats::mean(&lfd_values),
        );
        map.insert(
            "variance_letter_frequency_difference".into(),
            // Ruby's implementation uses `max` here, NOT statistical variance
            lfd_values.iter().cloned().fold(0.0_f64, f64::max),
        );

        map
    }

    // ── Character-set detection ───────────────────────────────────────────────

    /// Identifies which restricted alphabet the string belongs to (if any).
    pub fn character_set(&self) -> Option<&'static str> {
        let s = self.string;
        if s.chars().all(|c| matches!(c, 'a'..='f' | 'A'..='F' | '0'..='9' | '-')) {
            Some("hex")
        } else if s.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9')) {
            Some("lower36")
        } else if s.chars().all(|c| matches!(c, 'A'..='Z' | '0'..='9')) {
            Some("upper36")
        } else if s
            .trim_end_matches('=')
            .chars()
            .all(|c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '+' | '/'))
            && s.chars().filter(|&c| c == '=').count() <= 2
        {
            Some("base64")
        } else {
            None
        }
    }

    /// Number of distinct characters in the detected character set.
    pub fn character_set_total(&self) -> usize {
        match self.character_set() {
            Some("hex")     => 16,
            Some("lower36") => 36,
            Some("upper36") => 36,
            Some("base64")  => 64,
            _               => 0,
        }
    }

    /// Expected count of each character under a uniform distribution over the character set.
    pub fn ideal_letter_frequency(&self) -> f64 {
        let total = self.character_set_total();
        if total == 0 {
            return 0.0;
        }
        self.string.chars().count() as f64 / total as f64
    }

    // ── Letter statistics ─────────────────────────────────────────────────────

    /// Count of each character in `string` across the full BASE_64 alphabet.
    /// Characters not in BASE_64 are counted under their literal key but default to 0.
    pub fn letter_count(&self) -> HashMap<char, usize> {
        // Initialise every character in the BASE_64 alphabet to 0
        let mut counts: HashMap<char, usize> = base64_alphabet()
            .into_iter()
            .map(|c| (c, 0usize))
            .collect();

        for ch in self.string.chars() {
            *counts.entry(ch).or_insert(0) += 1;
        }
        counts
    }

    /// `letter_count[c] / string.len()` for every character in the alphabet.
    pub fn letter_frequency(&self) -> HashMap<char, f64> {
        let len = self.string.chars().count();
        if len == 0 {
            return base64_alphabet().into_iter().map(|c| (c, 0.0)).collect();
        }
        self.letter_count()
            .into_iter()
            .map(|(c, n)| (c, n as f64 / len as f64))
            .collect()
    }

    /// `|letter_frequency[c] - ideal_letter_frequency|` for every character in the alphabet.
    pub fn letter_frequency_difference(&self) -> HashMap<char, f64> {
        let ideal = self.ideal_letter_frequency();
        // When there is no recognised character set, ideal == 0.0, so the
        // difference is just the raw frequency – still a useful feature.
        self.letter_frequency()
            .into_iter()
            .map(|(c, f)| (c, (f - ideal).abs()))
            .collect()
    }

    /// Ratio of vowels to consonants (consonants clamped to 1 to avoid division by zero).
    pub fn vowel_consonant_ratio(&self) -> f64 {
        let counts = self.letter_count();
        let vowel_count: usize = VOWELS.iter().map(|c| counts.get(c).copied().unwrap_or(0)).sum();
        let consonant_count: usize =
            CONSONANTS.iter().map(|c| counts.get(c).copied().unwrap_or(0)).sum();
        vowel_count as f64 / consonant_count.max(1) as f64
    }

    // ── Chunk helpers ─────────────────────────────────────────────────────────
    // All chunk helpers return Vec<&str> borrowing from self.string.
    // They use simple ASCII-only patterns (matching the Ruby originals).

    /// Runs of `[A-Z][a-z]+` (title-case words).
    pub fn title_chunks(&self) -> Vec<&str> {
        let bytes = self.string.as_bytes();
        let mut result = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_uppercase() {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_lowercase() {
                    i += 1;
                }
                // Must have at least one lowercase char after the uppercase
                if i > start + 1 {
                    result.push(&self.string[start..i]);
                }
            } else {
                i += 1;
            }
        }
        result
    }

    /// Runs of `[a-z]+`.
    pub fn lower_chunks(&self) -> Vec<&str> {
        scan_chunks(self.string, |b| b.is_ascii_lowercase())
    }

    /// Runs of `[A-Z]+`.
    pub fn upper_chunks(&self) -> Vec<&str> {
        scan_chunks(self.string, |b| b.is_ascii_uppercase())
    }

    /// Runs of `[A-Za-z]+`.
    pub fn alpha_chunks(&self) -> Vec<&str> {
        scan_chunks(self.string, |b| b.is_ascii_alphabetic())
    }

    /// Runs of `[A-Za-z0-9]+`.
    pub fn alnum_chunks(&self) -> Vec<&str> {
        scan_chunks(self.string, |b| b.is_ascii_alphanumeric())
    }

    /// Runs of `[0-9]+`.
    pub fn digit_chunks(&self) -> Vec<&str> {
        scan_chunks(self.string, |b| b.is_ascii_digit())
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Collect all maximal contiguous substrings whose bytes satisfy `pred`.
fn scan_chunks<'s, F>(s: &'s str, pred: F) -> Vec<&'s str>
where
    F: Fn(u8) -> bool,
{
    let bytes = s.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if pred(bytes[i]) {
            let start = i;
            while i < bytes.len() && pred(bytes[i]) {
                i += 1;
            }
            result.push(&s[start..i]);
        } else {
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_set_lower36() {
        // g-z are outside the hex range [a-f], so this is unambiguously lower36
        assert_eq!(PossibleKey::new("ghijkl123").character_set(), Some("lower36"));
    }

    #[test]
    fn character_set_upper36() {
        // G-Z are outside the hex range [A-F], so this is unambiguously upper36
        assert_eq!(PossibleKey::new("GHIJKL123").character_set(), Some("upper36"));
    }

    #[test]
    fn character_set_hex() {
        // All chars in [a-fA-F0-9-]
        assert_eq!(PossibleKey::new("deadbeef").character_set(), Some("hex"));
        assert_eq!(PossibleKey::new("DEADBEEF").character_set(), Some("hex"));
        // Note: "abc123" is also hex because a,b,c are valid hex digits
        assert_eq!(PossibleKey::new("abc123").character_set(), Some("hex"));
    }

    #[test]
    fn character_set_base64() {
        assert_eq!(
            PossibleKey::new("SGVsbG8gV29ybGQ=").character_set(),
            Some("base64")
        );
    }

    #[test]
    fn character_set_none() {
        assert_eq!(PossibleKey::new("hello world").character_set(), None);
    }

    #[test]
    fn letter_count_counts_chars() {
        let pk = PossibleKey::new("aab");
        let counts = pk.letter_count();
        assert_eq!(counts.get(&'a'), Some(&2));
        assert_eq!(counts.get(&'b'), Some(&1));
        assert_eq!(counts.get(&'z'), Some(&0));
    }

    #[test]
    fn vowel_consonant_ratio_basic() {
        // "aeiou" has 5 vowels, 0 consonants -> 5/1 = 5.0
        let pk = PossibleKey::new("aeiou");
        assert!((pk.vowel_consonant_ratio() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn title_chunks_basic() {
        let pk = PossibleKey::new("HelloWorld");
        let chunks = pk.title_chunks();
        assert_eq!(chunks, vec!["Hello", "World"]);
    }

    #[test]
    fn alpha_chunks_basic() {
        let pk = PossibleKey::new("abc123def");
        let chunks = pk.alpha_chunks();
        assert_eq!(chunks, vec!["abc", "def"]);
    }

    #[test]
    fn digit_chunks_basic() {
        let pk = PossibleKey::new("abc123def456");
        let chunks = pk.digit_chunks();
        assert_eq!(chunks, vec!["123", "456"]);
    }

    #[test]
    fn features_has_expected_keys() {
        let pk = PossibleKey::new("abc123DEF");
        let f = pk.features();
        for key in &[
            "+", "-", "_", "/", "A", "z", "Z", "q", "Q", "X", "x",
            "equal", "length", "hex", "lower36", "upper36", "base64",
            "mean_title_chunk_size", "variance_title_chunk_size", "max_title_chunk_size",
            "vowel_consonant_ratio", "alpha_chunks", "alnum_chunks", "digit_chunks",
            "title_chunks", "mean_letter_frequency_difference",
            "variance_letter_frequency_difference",
        ] {
            assert!(f.contains_key(*key), "missing feature: {}", key);
        }
    }
}
