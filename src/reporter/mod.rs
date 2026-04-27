#![allow(dead_code)]
#![allow(unused_imports)]

pub mod autocorrect_reporter;
pub mod default_reporter;
pub mod interactive;
pub mod quiet_reporter;
pub mod wordlist_reporter;

use crate::token::Token;

// ── Reporter mode ─────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone)]
pub enum ReporterMode {
    Default,
    Wordlist,
    Quiet,
    Autocorrect,
    Interactive,
}

// ── Action returned by a reporter call ───────────────────────────────────────

/// What the checker should do after a reporter handles a token.
#[derive(Debug)]
pub enum ReporterAction {
    /// Continue processing normally.
    Continue,
    /// Replace the token's text in the file with this string, then re-check.
    Replace(String),
    /// Skip every future occurrence of this normalised term.
    SkipAll(String),
    /// Add this word to a project wordlist.
    ///
    /// `language_key` is the single-character key of the target language
    /// (e.g. `'r'` for ruby, `'e'` for english).  `None` means "pick the
    /// most appropriate one" (check.rs will decide).
    AddToWordlist {
        word: String,
        language_key: Option<char>,
    },
}

// ── Context provided per-file ─────────────────────────────────────────────────

/// Metadata about the file currently being checked.
///
/// Passed to reporters via [`Reporter::set_file_context`] so that interactive
/// reporters can display language choices when adding words.
#[derive(Debug, Clone)]
pub struct FileContext {
    /// (key_char, language_name) pairs for addable languages that matched the
    /// current file.
    pub addable_languages: Vec<(char, String)>,
}

// ── Reporter trait ────────────────────────────────────────────────────────────

pub trait Reporter: Send {
    /// Called for every misspelled token found in a file.
    ///
    /// Returns a [`ReporterAction`] that tells the checker what to do next.
    fn call(&mut self, token: &Token) -> ReporterAction;

    /// Called once after all files have been processed.
    fn finish(&mut self);

    /// The process exit code (0 = no errors, 1 = errors found).
    fn exit_code(&self) -> i32;

    /// Called once each time a file finishes being checked (regardless of
    /// whether it contained errors).
    fn checked_file(&mut self);

    /// Provide per-file context before tokens for that file are processed.
    ///
    /// Default implementation is a no-op; override in reporters that need it
    /// (e.g. the interactive reporter to show language choices).
    fn set_file_context(&mut self, _ctx: FileContext) {}

    /// Supply the wordlists for the file that is about to be processed.
    ///
    /// Called by `check.rs` before any tokens for that file are emitted so
    /// that reporters that need suggestions (autocorrect, interactive) can
    /// look up candidate replacements.
    ///
    /// Default implementation is a no-op; override where needed.
    fn set_wordlists(&mut self, _wordlists: Vec<crate::wordlist::Wordlist>) {}
}

// ── Factory ───────────────────────────────────────────────────────────────────

pub fn create_reporter(mode: ReporterMode) -> Box<dyn Reporter> {
    match mode {
        ReporterMode::Default => {
            Box::new(default_reporter::DefaultReporter::new())
        }
        ReporterMode::Wordlist => {
            Box::new(wordlist_reporter::WordlistReporter::new())
        }
        ReporterMode::Quiet => {
            Box::new(quiet_reporter::QuietReporter::new())
        }
        ReporterMode::Autocorrect => {
            Box::new(autocorrect_reporter::AutocorrectReporter::new())
        }
        ReporterMode::Interactive => {
            Box::new(interactive::InteractiveReporter::new())
        }
    }
}
