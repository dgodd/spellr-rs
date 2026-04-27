#![allow(dead_code)]

use crate::key_tuner::naive_bayes::NaiveBayes;
use crate::token_regexps::{
    AFTER_KEY_SKIPS_RE, POSSIBLE_KEY_RE, SKIPS_RE, SPELLR_DISABLE_RE, SPELLR_ENABLE_RE, TERM_RE,
    min_alpha_re,
};

// ── Public types ──────────────────────────────────────────────────────────────

/// A word found by scanning a single source-code line.
#[derive(Debug, Clone)]
pub struct RawToken {
    /// The matched text (borrowed lifetime tied to the line).
    pub value: String,
    /// Byte offset from the start of the line where the token begins.
    pub byte_offset: usize,
    /// Character (Unicode scalar) offset from the start of the line.
    pub char_offset: usize,
}

// ── LineTokenizer ─────────────────────────────────────────────────────────────

/// Scans a single UTF-8 line and yields [`RawToken`]s.
///
/// Mirrors the behaviour of the Ruby `LineTokenizer < StringScanner` class.
pub struct LineTokenizer<'a> {
    /// The line being scanned (a UTF-8 string slice).
    line: &'a str,
    /// Current byte position within `line`.
    pos: usize,
    /// Current character position within `line` (kept in sync with `pos`).
    char_pos: usize,
    /// When `true`, tokens are suppressed (between `spellr:disable` / `spellr:enable`).
    disabled: bool,
    /// When `true`, the key-heuristic (Naive Bayes) scan is active.
    skip_key: bool,
    /// Minimum character length a word must have to be emitted as a token.
    word_minimum_length: usize,
    /// Minimum character length a possible-key candidate must have.
    key_minimum_length: usize,
    /// The `key_heuristic_weight` config value (power-of-10 multiplier for key classes).
    key_heuristic_weight: f64,
}

impl<'a> LineTokenizer<'a> {
    pub fn new(
        line: &'a str,
        skip_key: bool,
        word_minimum_length: usize,
        key_minimum_length: usize,
        key_heuristic_weight: f64,
    ) -> Self {
        Self::new_with_disabled(line, skip_key, word_minimum_length, key_minimum_length, key_heuristic_weight, false)
    }

    /// Like `new`, but carries an existing `disabled` state from the previous line.
    /// This mirrors the Ruby version's reuse of a single LineTokenizer instance
    /// across all lines (where `@disabled` is never reset between lines).
    pub fn new_with_disabled(
        line: &'a str,
        skip_key: bool,
        word_minimum_length: usize,
        key_minimum_length: usize,
        key_heuristic_weight: f64,
        disabled: bool,
    ) -> Self {
        Self {
            line,
            pos: 0,
            char_pos: 0,
            disabled,
            skip_key,
            word_minimum_length,
            key_minimum_length,
            key_heuristic_weight,
        }
    }

    /// Returns the current disabled state (for carrying across lines).
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Collect all tokens from the line.
    ///
    /// Tokens emitted while `disabled == true` are silently dropped.
    pub fn tokens(&mut self) -> Vec<RawToken> {
        let mut out = Vec::new();
        loop {
            if self.pos >= self.line.len() {
                break;
            }

            // Step 1 – try to skip non-word material (may set/clear @disabled).
            if self.try_skip_nonwords_and_flags() {
                continue;
            }

            // Step 2 – try to scan the next word term.
            // Returns Some(Some(tok)) if a long-enough word was found,
            //         Some(None)      if TERM_RE matched but the word was too short
            //                         (input was still consumed — do NOT fallback),
            //         None            if TERM_RE did not match at all.
            match self.try_scan_term() {
                Some(Some(tok)) => {
                    if !self.disabled {
                        out.push(tok);
                    }
                    continue;
                }
                Some(None) => {
                    // Consumed a short word — skip the fallback advance.
                    continue;
                }
                None => {}
            }

            // Step 3 – fallback: advance by one Unicode scalar to avoid infinite loops.
            // This handles edge cases where neither a skip pattern nor TERM_RE matched
            // at the current position (e.g. a lone punctuation character not covered
            // by SKIPS_RE).
            self.advance_one_char();
        }
        out
    }

    // ── Non-word skip logic ───────────────────────────────────────────────────

    /// Returns `true` (and advances `pos`) if any non-word pattern or a
    /// disable/enable control comment was consumed.
    fn try_skip_nonwords_and_flags(&mut self) -> bool {
        self.try_skip_nonwords()
            || self.try_skip_and_track_enable()
            || self.try_skip_and_track_disable()
    }

    fn try_skip_nonwords(&mut self) -> bool {
        self.try_skip_re() || self.try_skip_key_heuristically() || self.try_skip_after_key()
    }

    /// Try to match `SKIPS_RE` at the current position.
    fn try_skip_re(&mut self) -> bool {
        let remaining = &self.line[self.pos..];
        match SKIPS_RE.find(remaining) {
            Ok(Some(m)) if m.start() == 0 => {
                self.advance(m.end());
                true
            }
            _ => false,
        }
    }

    /// Try to match `POSSIBLE_KEY_RE` at the current position, and if the
    /// Naive Bayes classifier thinks it's an API key, skip it.
    fn try_skip_key_heuristically(&mut self) -> bool {
        if !self.skip_key {
            return false;
        }

        let remaining = &self.line[self.pos..];
        let matched = match POSSIBLE_KEY_RE.find(remaining) {
            Ok(Some(m)) if m.start() == 0 => m.as_str().to_string(),
            _ => return false,
        };

        if self.is_key(&matched) {
            self.advance(matched.len());
            true
        } else {
            false
        }
    }

    /// Try to match `AFTER_KEY_SKIPS_RE` at the current position.
    fn try_skip_after_key(&mut self) -> bool {
        let remaining = &self.line[self.pos..];
        match AFTER_KEY_SKIPS_RE.find(remaining) {
            Ok(Some(m)) if m.start() == 0 => {
                self.advance(m.end());
                true
            }
            _ => false,
        }
    }

    /// Skip `spellr:disable` and set `disabled = true`.
    /// Only active when not already disabled.
    fn try_skip_and_track_disable(&mut self) -> bool {
        if self.disabled {
            return false;
        }
        let remaining = &self.line[self.pos..];
        if let Some(m) = SPELLR_DISABLE_RE.find(remaining) {
            if m.start() == 0 {
                self.advance(m.end());
                self.disabled = true;
                return true;
            }
        }
        false
    }

    /// Skip `spellr:enable` and set `disabled = false`.
    /// Only active when currently disabled.
    fn try_skip_and_track_enable(&mut self) -> bool {
        if !self.disabled {
            return false;
        }
        let remaining = &self.line[self.pos..];
        if let Some(m) = SPELLR_ENABLE_RE.find(remaining) {
            if m.start() == 0 {
                self.advance(m.end());
                self.disabled = false;
                return true;
            }
        }
        false
    }

    // ── Term scanning ─────────────────────────────────────────────────────────

    /// Try to match `TERM_RE` at the current position.
    ///
    /// Always advances past the match (even if the word is too short to emit),
    /// mirroring Ruby's `scan(TERM_RE)` which consumes on any match.
    ///
    /// Returns:
    ///   - `None`            – TERM_RE did not match at the current position (nothing consumed).
    ///   - `Some(None)`      – TERM_RE matched but the word was below `word_minimum_length`
    ///                         (input was consumed; caller must NOT apply the fallback advance).
    ///   - `Some(Some(tok))` – TERM_RE matched and the word is long enough to emit.
    fn try_scan_term(&mut self) -> Option<Option<RawToken>> {
        let remaining = &self.line[self.pos..];
        let m = match TERM_RE.find(remaining) {
            Ok(Some(m)) if m.start() == 0 => m,
            _ => return None,
        };

        let word = m.as_str();
        let byte_len = m.end(); // = m.as_str().len() since m.start() == 0
        let char_len = word.chars().count();

        let byte_offset = self.pos;
        let char_offset = self.char_pos;

        // Advance past the match regardless of length
        self.advance(byte_len);

        if char_len >= self.word_minimum_length {
            Some(Some(RawToken {
                value: word.to_string(),
                byte_offset,
                char_offset,
            }))
        } else {
            // Matched but too short — signal "consumed" without emitting a token.
            Some(None)
        }
    }

    // ── Key heuristic ─────────────────────────────────────────────────────────

    /// Decide whether `possible_key` should be treated as an opaque API key.
    fn is_key(&self, possible_key: &str) -> bool {
        let char_len = possible_key.chars().count();

        if char_len < self.key_minimum_length {
            return false;
        }
        // Very long strings are definitely keys (matches Ruby `> 200` check).
        if char_len > 200 {
            return true;
        }
        // Must contain a meaningful alphabetic run before invoking the classifier.
        let alpha_re = min_alpha_re(self.word_minimum_length);
        if !alpha_re.is_match(possible_key) {
            return false;
        }

        NaiveBayes::with_weight(self.key_heuristic_weight).is_key(possible_key)
    }

    // ── Position tracking ─────────────────────────────────────────────────────

    /// Advance `pos` by `bytes` bytes, keeping `char_pos` in sync.
    fn advance(&mut self, bytes: usize) {
        let text = &self.line[self.pos..self.pos + bytes];
        self.char_pos += text.chars().count();
        self.pos += bytes;
    }

    /// Advance by exactly one Unicode scalar (fallback to avoid infinite loops).
    fn advance_one_char(&mut self) {
        if let Some(ch) = self.line[self.pos..].chars().next() {
            self.pos += ch.len_utf8();
            self.char_pos += 1;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(line: &str) -> Vec<String> {
        let mut t = LineTokenizer::new(line, false, 3, 6, 5.0);
        t.tokens().into_iter().map(|t| t.value).collect()
    }

    #[test]
    fn tokenizes_simple_words() {
        assert_eq!(tokenize("hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn skips_short_words() {
        // "ab" is only 2 chars, below the minimum of 3
        let words = tokenize("ab hello");
        assert_eq!(words, vec!["hello"]);
    }

    #[test]
    fn tokenizes_title_case() {
        let words = tokenize("HelloWorld");
        // TITLE_CASE matches "Hello", then "World"
        assert!(words.contains(&"Hello".to_string()) || words.contains(&"World".to_string()));
    }

    #[test]
    fn skips_hex_literals() {
        let words = tokenize("#ff0000 color");
        assert!(!words.iter().any(|w| w.contains("ff")));
        assert!(words.contains(&"color".to_string()));
    }

    #[test]
    fn skips_urls() {
        let words = tokenize("visit https://example.com for info");
        assert!(!words.iter().any(|w| w == "example"));
        assert!(words.contains(&"visit".to_string()));
        assert!(words.contains(&"for".to_string()));
        assert!(words.contains(&"info".to_string()));
    }

    #[test]
    fn respects_disable_enable() {
        // Everything between disable and enable is suppressed
        let words = tokenize("before spellr:disable secret spellr:enable after");
        assert!(words.contains(&"before".to_string()));
        assert!(words.contains(&"after".to_string()));
        assert!(!words.contains(&"secret".to_string()));
    }

    #[test]
    fn char_and_byte_offsets_ascii() {
        let mut t = LineTokenizer::new("foo bar", false, 3, 6, 5.0);
        let tokens = t.tokens();
        assert_eq!(tokens[0].char_offset, 0);
        assert_eq!(tokens[0].byte_offset, 0);
        assert_eq!(tokens[1].char_offset, 4);
        assert_eq!(tokens[1].byte_offset, 4);
    }

    #[test]
    fn skips_repeated_single_letters() {
        // "xxxxxxxx" should be skipped (REPEATED_SINGLE_LETTERS)
        let words = tokenize("xxxxxxxx hello");
        assert!(!words.iter().any(|w| w.starts_with('x')));
        assert!(words.contains(&"hello".to_string()));
    }

    #[test]
    fn skips_sequential_letters() {
        // "abcdef" is a sequential alphabet run
        let words = tokenize("abcdef hello");
        assert!(!words.contains(&"abcdef".to_string()));
        assert!(words.contains(&"hello".to_string()));
    }

    #[test]
    fn tokenizes_io_string_correctly() {
        // "IOString" should split into the uppercase run "IO" (too short, filtered)
        // and the title-case word "String" — NOT "tring".
        // The bug was that try_scan_term would advance past the short "IO" match and
        // return None, causing the fallback advance_one_char() to also fire and eat
        // the "S", leaving only "tring".
        let words = tokenize("IOString");
        assert!(
            words.contains(&"String".to_string()),
            "expected 'String' in {:?}",
            words
        );
        assert!(
            !words.iter().any(|w| w == "tring"),
            "found 'tring' in {:?} — short-word fallback bug is present",
            words
        );
    }
}
