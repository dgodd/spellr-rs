#![allow(dead_code)]

use std::collections::HashSet;

use crate::reporter::{FileContext, Reporter, ReporterAction};
use crate::token::Token;

pub struct WordlistReporter {
    /// Unique normalised words encountered.
    words: HashSet<String>,
    /// 0 = clean, 1 = errors found.
    exit_code: i32,
    /// Number of files that finished being checked.
    checked_files: usize,
}

impl WordlistReporter {
    pub fn new() -> Self {
        Self {
            words: HashSet::new(),
            exit_code: 0,
            checked_files: 0,
        }
    }
}

impl Default for WordlistReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for WordlistReporter {
    fn call(&mut self, token: &Token) -> ReporterAction {
        // Mark that errors exist so the exit code is non-zero.
        self.exit_code = 1;
        // Collect the normalised form of the token.
        self.words.insert(token.normalized());
        ReporterAction::Continue
    }

    fn finish(&mut self) {
        if self.words.is_empty() {
            return;
        }
        let mut sorted: Vec<&String> = self.words.iter().collect();
        sorted.sort();
        for word in sorted {
            println!("{word}");
        }
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }

    fn checked_file(&mut self) {
        self.checked_files += 1;
    }

    fn set_file_context(&mut self, _ctx: FileContext) {}
}
