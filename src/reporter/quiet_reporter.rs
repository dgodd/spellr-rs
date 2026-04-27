#![allow(dead_code)]

use crate::reporter::{FileContext, Reporter, ReporterAction};
use crate::token::Token;

/// A reporter that produces no output but still sets the exit code correctly.
///
/// Mirrors `Spellr::QuietReporter` from the Ruby implementation.
pub struct QuietReporter {
    /// 0 = clean, 1 = errors found.
    exit_code: i32,
    /// Number of files that finished being checked.
    checked_files: usize,
}

impl QuietReporter {
    pub fn new() -> Self {
        Self {
            exit_code: 0,
            checked_files: 0,
        }
    }
}

impl Default for QuietReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for QuietReporter {
    fn call(&mut self, _token: &Token) -> ReporterAction {
        // Silently mark that at least one error was found.
        self.exit_code = 1;
        ReporterAction::Continue
    }

    fn finish(&mut self) {
        // Intentionally produces no output.
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }

    fn checked_file(&mut self) {
        self.checked_files += 1;
    }

    fn set_file_context(&mut self, _ctx: FileContext) {}
}
