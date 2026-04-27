#![allow(dead_code)]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use crate::config::Config;
use crate::line_tokenizer::LineTokenizer;
use crate::token::{ColumnLocation, Token, normalize_str};
use crate::token_regexps::SPELLR_LINE_DISABLE_RE;

/// File-level tokenizer: reads a source file line by line and yields [`Token`]s.
///
/// Mirrors the Ruby `Spellr::Tokenizer` class.
pub struct Tokenizer {
    pub path: PathBuf,
    config: Config,
}

impl Tokenizer {
    pub fn new(path: PathBuf, config: Config) -> Self {
        Self { path, config }
    }

    /// Iterate over every spellable token in the file.
    ///
    /// `skip_term` is called for each candidate word; returning `true` suppresses
    /// the token (used to filter words that already appear in a wordlist).
    pub fn each_token<F, S>(&self, skip_term: S, mut callback: F)
    where
        F: FnMut(Token),
        S: Fn(&str) -> bool,
    {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("spellr: cannot open {:?}: {}", self.path, e);
                return;
            }
        };
        let reader = BufReader::new(file);

        // Cumulative offsets from the start of the file (updated after each line).
        let mut line_char_offset: usize = 0;
        let mut line_byte_offset: usize = 0;
        // The disable state persists across lines (mirrors Ruby's reuse of a single
        // LineTokenizer instance whose @disabled ivar is never reset between lines).
        let mut disabled: bool = false;

        for (line_idx, line_result) in reader.lines().enumerate() {
            let line_str = match line_result {
                Ok(l) => l,
                Err(_) => continue,
            };
            // Restore the newline that BufRead strips, so byte/char counts stay accurate.
            // (We do length arithmetic on the raw line including the newline.)
            let line_with_nl = format!("{}\n", line_str);

            let line_number = line_idx + 1; // 1-based

            // Skip lines that carry a `spellr:disable-line` / `spellr:disable:line` marker.
            if SPELLR_LINE_DISABLE_RE.is_match(&line_with_nl) {
                line_char_offset += line_with_nl.chars().count();
                line_byte_offset += line_with_nl.len();
                continue;
            }

            // Tokenise the current line, carrying the disabled state across lines.
            let mut lt = LineTokenizer::new_with_disabled(
                &line_with_nl,
                /* skip_key = */ true,
                self.config.word_minimum_length,
                self.config.key_minimum_length,
                self.config.key_heuristic_weight,
                disabled,
            );

            for raw in lt.tokens() {
                let word = &raw.value;

                // Apply the caller-supplied skip predicate (e.g. wordlist lookup).
                let normalized = normalize_str(word);
                if skip_term(&normalized) {
                    continue;
                }

                let location = ColumnLocation {
                    char_offset: raw.char_offset,
                    byte_offset: raw.byte_offset,
                    line_number,
                    file: self.path.clone(),
                    line_char_offset,
                    line_byte_offset,
                };

                // Store the line content without the synthetic newline for display.
                let token = Token::new(word.clone(), location, line_str.clone());
                callback(token);
            }

            // Carry the disabled state forward to the next line.
            disabled = lt.is_disabled();

            // Advance cumulative offsets past this line (including its newline).
            line_char_offset += line_with_nl.chars().count();
            line_byte_offset += line_with_nl.len();
        }
    }

    /// Collect all unique normalised terms from the file (for wordlist generation).
    pub fn normalized_terms(&self) -> Vec<String> {
        let mut terms: Vec<String> = Vec::new();
        self.each_token(|_| false, |tok| {
            let n = tok.normalized();
            if !terms.contains(&n) {
                terms.push(n);
            }
        });
        terms.sort();
        terms
    }
}
