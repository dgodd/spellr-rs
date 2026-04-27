#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::io::Write;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::reporter::{FileContext, Reporter, ReporterAction};
use crate::string_format::{aqua, bold, green, red};
use crate::token::Token;
use crate::wordlist::Wordlist;

// ── InteractiveReporter ───────────────────────────────────────────────────────

pub struct InteractiveReporter {
    /// 0 = clean, 1 = errors found.
    exit_code: i32,
    /// Tokens successfully replaced (via r / R / digit suggestion).
    total_fixed: usize,
    /// Tokens skipped (via s / S / Esc).
    total_skipped: usize,
    /// Words added to a wordlist (via a).
    total_added: usize,
    /// Number of files that finished being checked.
    checked_files: usize,
    /// token.value → replacement; populated by "Replace all" (R).
    global_replacements: HashMap<String, String>,
    /// Normalised token values that are always skipped (populated by S).
    global_skips: HashSet<String>,
    /// Wordlists for the current file – used to fetch spelling suggestions.
    current_wordlists: Vec<Wordlist>,
    /// Per-file language context – used to show wordlist choices for "add".
    file_context: Option<FileContext>,
}

impl InteractiveReporter {
    pub fn new() -> Self {
        Self {
            exit_code: 0,
            total_fixed: 0,
            total_skipped: 0,
            total_added: 0,
            checked_files: 0,
            global_replacements: HashMap::new(),
            global_skips: HashSet::new(),
            current_wordlists: Vec::new(),
            file_context: None,
        }
    }

    /// Update the wordlists for the file that is about to be processed.
    ///
    /// `check.rs` calls this before emitting tokens for each new file.
    pub fn set_wordlists(&mut self, wordlists: Vec<Wordlist>) {
        self.current_wordlists = wordlists;
    }

    // ── Display helpers ───────────────────────────────────────────────────────

    fn print_header(&self, token: &Token) {
        let loc = &token.location;
        let location_str = format!(
            "{}:{}:{}",
            loc.file.display(),
            loc.line_number,
            loc.char_offset,
        );
        eprintln!(
            "{} {}",
            aqua(&location_str),
            token.highlight_in_line().trim_end()
        );
    }

    fn print_suggestions(suggestions: &[String]) {
        if suggestions.is_empty() {
            return;
        }
        let parts: Vec<String> = suggestions
            .iter()
            .enumerate()
            .map(|(i, word)| format!("[{}] {}", i + 1, bold(word)))
            .collect();
        eprintln!("Did you mean: {}", parts.join(", "));
    }

    fn print_prompt(suggestions: &[String]) {
        let hint = if !suggestions.is_empty() {
            " digit=suggestion,"
        } else {
            ""
        };
        eprint!(
            "[{}]dd, [{}]eplace, [{}]kip, [{}]kip-all,{} [{}]elp, [^C] exit: ",
            bold("a"),
            bold("r"),
            bold("s"),
            bold("S"),
            hint,
            bold("h"),
        );
        let _ = std::io::stderr().flush();
    }

    fn print_help() {
        eprintln!();
        eprintln!("{}", bold("Interactive spellr help:"));
        eprintln!("  {}      Add the word to a project wordlist", bold("a"));
        eprintln!("  {}      Replace with a custom string (this occurrence)", bold("r"));
        eprintln!("  {}      Replace with a custom string (ALL occurrences)", bold("R"));
        eprintln!("  {}      Skip this occurrence", bold("s"));
        eprintln!("  {}      Skip ALL future occurrences of this word", bold("S"));
        eprintln!("  {}   Use the Nth suggestion as the replacement", bold("1-9"));
        eprintln!("  {}   Quit spellr (exit code 1)", bold("q / Ctrl-C"));
        eprintln!();
    }

    // ── Input helpers ─────────────────────────────────────────────────────────

    /// Read the replacement string using rustyline with the token value
    /// pre-filled.  Raw mode must be **disabled** when this is called.
    fn read_replacement(token: &Token) -> Option<String> {
        eprintln!(); // blank line after the prompt
        match rustyline::DefaultEditor::new() {
            Ok(mut rl) => {
                match rl.readline_with_initial("Replace with: ", (&token.value, "")) {
                    Ok(line) => {
                        let trimmed = line.trim().to_string();
                        if trimmed.is_empty() { None } else { Some(trimmed) }
                    }
                    // Ctrl-C / Ctrl-D / EOF in the readline
                    Err(rustyline::error::ReadlineError::Interrupted)
                    | Err(rustyline::error::ReadlineError::Eof) => {
                        eprintln!();
                        std::process::exit(1);
                    }
                    Err(_) => None,
                }
            }
            Err(_) => {
                // Fall back to a plain stdin read.
                let mut buf = String::new();
                if std::io::stdin().read_line(&mut buf).is_ok() {
                    let trimmed = buf.trim().to_string();
                    if trimmed.is_empty() { None } else { Some(trimmed) }
                } else {
                    None
                }
            }
        }
    }

    /// Prompt the user to choose a target language (by key character) when
    /// adding a word to a wordlist.  Returns `None` if the user pressed
    /// something unrecognised or if no addable languages are known.
    fn prompt_for_language_key(&self) -> Option<char> {
        let ctx = self.file_context.as_ref()?;
        if ctx.addable_languages.is_empty() {
            return None;
        }

        eprintln!();
        eprintln!("{}", bold("Add to which wordlist?"));
        for (key_char, name) in &ctx.addable_languages {
            eprintln!("  [{}] {}", bold(&key_char.to_string()), name);
        }
        eprint!("Choice: ");
        let _ = std::io::stderr().flush();

        // Collect the valid key characters so we can validate the input.
        let valid_keys: HashSet<char> = ctx.addable_languages.iter().map(|(k, _)| *k).collect();

        loop {
            match read_single_keypress() {
                Ok(ke) => {
                    if let KeyCode::Char(c) = ke.code {
                        // Handle Ctrl-C inside the sub-prompt too.
                        if ke.modifiers.contains(KeyModifiers::CONTROL)
                            && (c == 'c' || c == 'd')
                        {
                            eprintln!();
                            std::process::exit(1);
                        }
                        if valid_keys.contains(&c) {
                            eprintln!("{c}"); // echo choice
                            return Some(c);
                        }
                        // Invalid key – keep looping (raw mode is toggled by
                        // read_single_keypress so we just loop again).
                        eprint!("\r(invalid – press one of the listed keys) Choice: ");
                        let _ = std::io::stderr().flush();
                    }
                }
                Err(_) => return None,
            }
        }
    }

    // ── Core token-handling loop ──────────────────────────────────────────────

    fn handle_token(&mut self, token: &Token) -> ReporterAction {
        let norm = token.normalized();

        // ── 1. Consult global caches ──────────────────────────────────────────

        if let Some(replacement) = self.global_replacements.get(&token.value).cloned() {
            self.total_fixed += 1;
            eprintln!(
                "Automatically replaced {} → {}",
                red(&token.value),
                green(&replacement)
            );
            return ReporterAction::Replace(replacement);
        }

        if self.global_skips.contains(&norm) {
            eprintln!("Automatically skipped {}", red(&token.value));
            return ReporterAction::Continue;
        }

        // ── 2. Print location + highlighted line ──────────────────────────────

        self.exit_code = 1;
        self.print_header(token);

        // Clone wordlists snapshot so we can pass &mut to the suggester.
        let suggestions = crate::suggester::get_suggestions(token, &mut self.current_wordlists, 5);

        Self::print_suggestions(&suggestions);

        // ── 3. Prompt loop ────────────────────────────────────────────────────

        loop {
            Self::print_prompt(&suggestions);

            let key_event = match read_single_keypress() {
                Ok(ke) => ke,
                Err(_) => {
                    eprintln!();
                    return ReporterAction::Continue;
                }
            };

            // Global Ctrl-C / Ctrl-D → exit immediately.
            if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                match key_event.code {
                    KeyCode::Char('c') | KeyCode::Char('d') => {
                        eprintln!();
                        std::process::exit(1);
                    }
                    _ => {
                        eprintln!("\n(press [h] for help)");
                        continue;
                    }
                }
            }

            match key_event.code {
                // ── Quit ──────────────────────────────────────────────────────
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    eprintln!();
                    std::process::exit(1);
                }

                // ── Skip (this occurrence) ────────────────────────────────────
                KeyCode::Char('s') | KeyCode::Esc => {
                    eprintln!();
                    eprintln!("Skipped {}", red(&token.value));
                    self.total_skipped += 1;
                    return ReporterAction::Continue;
                }

                // ── Skip all occurrences ──────────────────────────────────────
                KeyCode::Char('S') => {
                    eprintln!();
                    eprintln!(
                        "Skipped {} (all future occurrences)",
                        red(&token.value)
                    );
                    self.total_skipped += 1;
                    self.global_skips.insert(norm.clone());
                    return ReporterAction::SkipAll(norm);
                }

                // ── Replace (this occurrence) ─────────────────────────────────
                KeyCode::Char('r') => {
                    if let Some(replacement) = Self::read_replacement(token) {
                        self.total_fixed += 1;
                        eprintln!(
                            "Replaced {} → {}",
                            red(&token.value),
                            green(&replacement)
                        );
                        return ReporterAction::Replace(replacement);
                    }
                    eprintln!("(no replacement entered – skipping)");
                    self.total_skipped += 1;
                    return ReporterAction::Continue;
                }

                // ── Replace all occurrences ───────────────────────────────────
                KeyCode::Char('R') => {
                    if let Some(replacement) = Self::read_replacement(token) {
                        self.total_fixed += 1;
                        self.global_replacements
                            .insert(token.value.clone(), replacement.clone());
                        eprintln!(
                            "Replaced {} → {} (all future occurrences)",
                            red(&token.value),
                            green(&replacement)
                        );
                        return ReporterAction::Replace(replacement);
                    }
                    eprintln!("(no replacement entered – skipping)");
                    self.total_skipped += 1;
                    return ReporterAction::Continue;
                }

                // ── Add to wordlist ───────────────────────────────────────────
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    eprintln!();
                    let language_key = self.prompt_for_language_key();
                    self.total_added += 1;
                    eprintln!("Added {} to wordlist", green(&norm));
                    return ReporterAction::AddToWordlist { word: norm, language_key };
                }

                // ── Help ──────────────────────────────────────────────────────
                KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') => {
                    eprintln!();
                    Self::print_help();
                    // Reprint the header so the user doesn't lose context.
                    self.print_header(token);
                    Self::print_suggestions(&suggestions);
                    continue;
                }

                // ── Numbered suggestion ───────────────────────────────────────
                KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                    let idx = (c as usize).saturating_sub('1' as usize);
                    if idx < suggestions.len() {
                        let replacement = suggestions[idx].clone();
                        eprintln!();
                        eprintln!(
                            "Replaced {} → {}",
                            red(&token.value),
                            green(&replacement)
                        );
                        self.total_fixed += 1;
                        return ReporterAction::Replace(replacement);
                    }
                    eprintln!("\n(invalid suggestion number – press [h] for help)");
                    continue;
                }

                // ── Unknown key ───────────────────────────────────────────────
                _ => {
                    eprintln!("\n(unknown key – press [h] for help)");
                    continue;
                }
            }
        }
    }
}

impl Default for InteractiveReporter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Reporter impl ─────────────────────────────────────────────────────────────

impl Reporter for InteractiveReporter {
    fn call(&mut self, token: &Token) -> ReporterAction {
        self.handle_token(token)
    }

    fn finish(&mut self) {
        // Make sure we leave the terminal in a usable state.
        let _ = terminal::disable_raw_mode();
        eprintln!();
        if self.total_fixed > 0 {
            let msg = crate::string_format::pluralize("error", self.total_fixed);
            eprintln!("{}", green(&format!("{msg} fixed")));
        }
        if self.total_skipped > 0 {
            let msg = crate::string_format::pluralize("word", self.total_skipped);
            eprintln!("{}", bold(&format!("{msg} skipped")));
        }
        if self.total_added > 0 {
            let msg = crate::string_format::pluralize("word", self.total_added);
            eprintln!("{}", bold(&format!("{msg} added to wordlists")));
        }
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }

    fn checked_file(&mut self) {
        self.checked_files += 1;
    }

    fn set_file_context(&mut self, ctx: FileContext) {
        self.file_context = Some(ctx);
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Enable raw mode, read exactly one `KeyPress` event, then disable raw mode.
///
/// Mouse and resize events are silently discarded.  `KeyEventKind::Release` /
/// `KeyEventKind::Repeat` events (crossterm ≥ 0.25) are filtered out so that
/// the caller always receives a key-press.
fn read_single_keypress() -> std::io::Result<KeyEvent> {
    terminal::enable_raw_mode()?;
    let result = loop {
        match crossterm::event::read()? {
            Event::Key(ke) => {
                // Only act on physical key-press events (not release / repeat).
                if ke.kind == KeyEventKind::Press {
                    break Ok(ke);
                }
            }
            // Discard mouse / resize / focus events.
            _ => {}
        }
    };
    let _ = terminal::disable_raw_mode();
    result
}
