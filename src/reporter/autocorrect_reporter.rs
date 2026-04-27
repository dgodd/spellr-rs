#![allow(dead_code)]

use crate::reporter::{FileContext, Reporter, ReporterAction};
use crate::string_format::{bold, green, red};
use crate::token::Token;
use crate::wordlist::Wordlist;

/// A reporter that automatically replaces each misspelled token with the
/// best suggestion returned by the suggester.
///
/// Mirrors the Ruby `Spellr::AutocorrectReporter`.
pub struct AutocorrectReporter {
    /// 0 = clean, 1 = errors found.
    exit_code: i32,
    /// Tokens that were successfully autocorrected.
    total_fixed: usize,
    /// Tokens for which no suitable suggestion could be found.
    total_unfixed: usize,
    /// Number of files that finished being checked.
    checked_files: usize,
    /// Wordlists for the file currently being processed.
    /// Updated each time `set_wordlists` is called.
    current_wordlists: Vec<Wordlist>,
}

impl AutocorrectReporter {
    pub fn new() -> Self {
        Self {
            exit_code: 0,
            total_fixed: 0,
            total_unfixed: 0,
            checked_files: 0,
            current_wordlists: Vec::new(),
        }
    }

    /// Provide the wordlists for the file that is about to be processed.
    ///
    /// `check.rs` calls this before emitting tokens for each new file so that
    /// the reporter can look up spelling suggestions.
    pub fn set_wordlists(&mut self, wordlists: Vec<Wordlist>) {
        self.current_wordlists = wordlists;
    }
}

impl Default for AutocorrectReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for AutocorrectReporter {
    fn call(&mut self, token: &Token) -> ReporterAction {
        // We found a misspelled token → exit code must be non-zero.
        self.exit_code = 1;

        // Ask the suggester for the single best replacement.
        let suggestions =
            crate::suggester::get_suggestions(token, &mut self.current_wordlists, 1);

        if let Some(replacement) = suggestions.into_iter().next() {
            self.total_fixed += 1;
            eprintln!(
                "Replaced {} with {}",
                red(&token.value),
                green(&replacement),
            );
            ReporterAction::Replace(replacement)
        } else {
            // No suggestion available – leave the token as-is and report it
            // the same way the default reporter would, so the user knows.
            self.total_unfixed += 1;
            let location_str = format!(
                "{}:{}:{}",
                token.location.file.display(),
                token.location.line_number,
                token.location.char_offset,
            );
            let highlighted = token.highlight_in_line();
            eprintln!(
                "{} {}  {}",
                crate::string_format::aqua(&location_str),
                highlighted.trim_end(),
                bold("(no suggestion)"),
            );
            ReporterAction::Continue
        }
    }

    fn finish(&mut self) {
        eprintln!();
        if self.total_fixed > 0 {
            let msg = crate::string_format::pluralize("error", self.total_fixed);
            eprintln!("{}", green(&format!("{msg} autocorrected")));
        }
        if self.total_unfixed > 0 {
            let msg = crate::string_format::pluralize("error", self.total_unfixed);
            eprintln!("{}", bold(&format!("{msg} could not be autocorrected")));
        }
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }

    fn checked_file(&mut self) {
        self.checked_files += 1;
    }

    fn set_file_context(&mut self, _ctx: FileContext) {
        // Autocorrect reporter does not need language context – wordlists are
        // supplied separately via `set_wordlists`.
    }
}
