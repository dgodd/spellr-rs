#![allow(dead_code)]

use std::collections::HashSet;

use crate::reporter::{FileContext, Reporter, ReporterAction};
use crate::string_format::{aqua, bold, green, pluralize};
use crate::token::Token;

pub struct DefaultReporter {
    /// 0 = clean, 1 = errors found.
    exit_code: i32,
    /// Total number of misspelled tokens reported.
    total_errors: usize,
    /// Number of files that finished being checked.
    checked_files: usize,
    /// Paths (as strings) of files that contained at least one error.
    error_files: HashSet<String>,
}

impl DefaultReporter {
    pub fn new() -> Self {
        Self {
            exit_code: 0,
            total_errors: 0,
            checked_files: 0,
            error_files: HashSet::new(),
        }
    }
}

impl Default for DefaultReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for DefaultReporter {
    fn call(&mut self, token: &Token) -> ReporterAction {
        // Mark that we found an error.
        self.exit_code = 1;
        self.total_errors += 1;
        self.error_files
            .insert(token.location.file.display().to_string());

        // Format: "<aqua location>  <highlighted line stripped of leading/trailing whitespace>"
        let location_str = format!(
            "{}:{}:{}",
            token.location.file.display(),
            token.location.line_number,
            token.location.char_offset,
        );
        let highlighted = token.highlight_in_line();
        let stripped = highlighted.trim_end().to_string();
        // Print to stderr to match Ruby's `warn` / `$stderr.puts` behaviour.
        eprintln!("{} {}", aqua(&location_str), stripped);

        ReporterAction::Continue
    }

    fn finish(&mut self) {
        eprintln!();

        let files_msg = pluralize("file", self.checked_files);
        eprintln!("{}", bold(&format!("{files_msg} checked")));

        if self.total_errors > 0 {
            let errors_msg = pluralize("error", self.total_errors);
            eprintln!("{}", bold(&format!("{errors_msg} found")));

            // Suggest the interactive mode so the user can fix things easily.
            eprintln!(
                "{}",
                green("Run `spellr --interactive` to fix errors interactively")
            );
        }
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }

    fn checked_file(&mut self) {
        self.checked_files += 1;
    }

    fn set_file_context(&mut self, _ctx: FileContext) {
        // Default reporter does not need per-file context.
    }
}
