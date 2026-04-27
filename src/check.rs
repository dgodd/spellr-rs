#![allow(dead_code)]
#![allow(unused_imports)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::config::Config;
use crate::file_list::FileList;
use crate::language::{Language, languages_from_config, wordlists_for_file};
use crate::reporter::{create_reporter, FileContext, Reporter, ReporterAction, ReporterMode};
use crate::token::Token;
use crate::tokenizer::Tokenizer;
use crate::wordlist::Wordlist;

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run the spell-check over all files and return the process exit code.
pub fn run_check(files: FileList, config: &Config, mode: ReporterMode, parallel: bool) -> i32 {
    let wordlists_dir = get_wordlists_dir();
    let project_dir = std::env::current_dir().unwrap_or_default();
    let languages = languages_from_config(config, &wordlists_dir, &project_dir);
    let all_files: Vec<PathBuf> = files.iter().collect();

    if parallel {
        let reporter: Arc<Mutex<Box<dyn Reporter>>> =
            Arc::new(Mutex::new(create_reporter(mode)));
        run_parallel(&all_files, config, &languages, &reporter);
        let mut r = reporter.lock().unwrap();
        r.finish();
        r.exit_code()
    } else {
        let mut reporter = create_reporter(mode);
        run_serial(&all_files, config, &languages, &mut *reporter);
        reporter.finish();
        reporter.exit_code()
    }
}

// ── Serial execution ──────────────────────────────────────────────────────────

fn run_serial(
    files: &[PathBuf],
    config: &Config,
    languages: &[Language],
    reporter: &mut dyn Reporter,
) {
    for path in files {
        check_file(path, config, languages, reporter);
    }
}

// ── Parallel execution ────────────────────────────────────────────────────────

fn run_parallel(
    files: &[PathBuf],
    config: &Config,
    languages: &[Language],
    reporter: &Arc<Mutex<Box<dyn Reporter>>>,
) {
    files.par_iter().for_each(|path| {
        check_file_parallel(path, config, languages, reporter);
    });
}

/// Process a single file under a shared (mutex-protected) reporter.
///
/// Tokens are collected without holding the lock; the lock is acquired only
/// when the reporter is called, which keeps contention minimal.
fn check_file_parallel(
    path: &Path,
    config: &Config,
    languages: &[Language],
    reporter: &Arc<Mutex<Box<dyn Reporter>>>,
) {
    let first_line = read_first_line(path);

    // Provide per-file context and wordlists to the reporter (under lock).
    {
        let ctx = build_file_context(languages, path, first_line.as_deref());
        let wls = wordlists_for_file(languages, path, first_line.as_deref());
        let mut r = reporter.lock().unwrap();
        r.set_file_context(ctx);
        r.set_wordlists(wls);
    }

    let mut skip_all: HashSet<String> = HashSet::new();
    let mut needs_reload = true;
    let mut word_sets: Vec<HashSet<String>> = Vec::new();

    loop {
        if needs_reload {
            word_sets = build_word_sets(languages, path, first_line.as_deref());
            needs_reload = false;
        }

        let tokens = collect_tokens(path, config, &word_sets);
        let mut restart = false;

        'token_loop: for token in &tokens {
            let norm = token.normalized();
            if skip_all.contains(&norm) {
                continue;
            }

            let action = {
                let mut r = reporter.lock().unwrap();
                r.call(token)
            };

            match action {
                ReporterAction::Continue => {}
                ReporterAction::Replace(replacement) => {
                    replace_in_file(path, token, &replacement);
                    restart = true;
                    break 'token_loop;
                }
                ReporterAction::SkipAll(val) => {
                    skip_all.insert(val);
                }
                ReporterAction::AddToWordlist { word, language_key } => {
                    add_to_wordlist(languages, &word, language_key, path, first_line.as_deref());
                    {
                        let wls = wordlists_for_file(languages, path, first_line.as_deref());
                        let mut r = reporter.lock().unwrap();
                        r.set_wordlists(wls);
                    }
                    needs_reload = true;
                    restart = true;
                    break 'token_loop;
                }
            }
        }

        if !restart {
            break;
        }
    }

    reporter.lock().unwrap().checked_file();
}

// ── Serial file processing ────────────────────────────────────────────────────

fn check_file(
    path: &Path,
    config: &Config,
    languages: &[Language],
    reporter: &mut dyn Reporter,
) {
    let first_line = read_first_line(path);

    // Provide per-file context and initial wordlists to the reporter.
    let ctx = build_file_context(languages, path, first_line.as_deref());
    reporter.set_file_context(ctx);
    reporter.set_wordlists(wordlists_for_file(languages, path, first_line.as_deref()));

    let mut skip_all: HashSet<String> = HashSet::new();
    let mut needs_reload = true;
    let mut word_sets: Vec<HashSet<String>> = Vec::new();

    loop {
        if needs_reload {
            word_sets = build_word_sets(languages, path, first_line.as_deref());
            needs_reload = false;
        }

        let tokens = collect_tokens(path, config, &word_sets);
        let mut restart = false;

        'token_loop: for token in &tokens {
            let norm = token.normalized();
            if skip_all.contains(&norm) {
                continue;
            }

            match reporter.call(token) {
                ReporterAction::Continue => {}
                ReporterAction::Replace(replacement) => {
                    replace_in_file(path, token, &replacement);
                    restart = true;
                    break 'token_loop;
                }
                ReporterAction::SkipAll(val) => {
                    skip_all.insert(val);
                }
                ReporterAction::AddToWordlist { word, language_key } => {
                    add_to_wordlist(languages, &word, language_key, path, first_line.as_deref());
                    // Refresh the reporter's wordlists so it can find the new word.
                    reporter.set_wordlists(
                        wordlists_for_file(languages, path, first_line.as_deref()),
                    );
                    needs_reload = true;
                    restart = true;
                    break 'token_loop;
                }
            }
        }

        if !restart {
            break;
        }
    }

    reporter.checked_file();
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Collect tokens from `path` that are *not* found in any of the `word_sets`.
fn collect_tokens(
    path: &Path,
    config: &Config,
    word_sets: &[HashSet<String>],
) -> Vec<Token> {
    let tokenizer = Tokenizer::new(path.to_path_buf(), config.clone());
    let mut tokens: Vec<Token> = Vec::new();
    tokenizer.each_token(
        // skip_term: true  ↔  word is known → suppress token
        |term| word_sets.iter().any(|ws| ws.contains(term)),
        |token| tokens.push(token),
    );
    tokens
}

/// Load wordlists for `path` and flatten them into `HashSet<String>` for
/// O(1) membership testing.
fn build_word_sets(
    languages: &[Language],
    path: &Path,
    first_line: Option<&str>,
) -> Vec<HashSet<String>> {
    let mut wordlists = wordlists_for_file(languages, path, first_line);
    wordlists
        .iter_mut()
        .map(|w| w.words().iter().cloned().collect::<HashSet<String>>())
        .collect()
}

/// Build the [`FileContext`] for the current file (used by the interactive
/// reporter to present language-wordlist choices when the user presses `a`).
fn build_file_context(
    languages: &[Language],
    path: &Path,
    first_line: Option<&str>,
) -> FileContext {
    let addable_languages: Vec<(char, String)> = languages
        .iter()
        .filter(|l| l.addable && l.matches_file(path, first_line))
        .filter_map(|l| l.key.chars().next().map(|c| (c, l.name.clone())))
        .collect();
    FileContext { addable_languages }
}

/// Add `word` to the project wordlist of the first addable language whose key
/// matches `language_key` (or the first addable matching language if `None`).
fn add_to_wordlist(
    languages: &[Language],
    word: &str,
    language_key: Option<char>,
    path: &Path,
    first_line: Option<&str>,
) {
    for lang in languages {
        if !lang.addable {
            continue;
        }
        let key_ok = language_key
            .map(|k| lang.key.starts_with(k))
            .unwrap_or(true);
        if key_ok && lang.matches_file(path, first_line) {
            let mut wl = lang.project_wordlist();
            if let Err(e) = wl.push(word) {
                eprintln!("spellr: could not add '{}' to wordlist: {}", word, e);
            }
            return;
        }
    }
    eprintln!(
        "spellr: no addable wordlist found for key {:?}",
        language_key
    );
}

// ── File utilities ────────────────────────────────────────────────────────────

/// Locate the bundled wordlists directory.
///
/// Search order:
///   1. `<executable_dir>/wordlists/` — installed binary
///   2. `CARGO_MANIFEST_DIR/wordlists/` — `cargo run` / development
fn get_wordlists_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("wordlists");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("wordlists")
}

/// Read the first non-empty line from a file (used for shebang detection).
///
/// Returns `None` if the file cannot be opened or is empty.
fn read_first_line(path: &Path) -> Option<String> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let trimmed = line
        .trim_end_matches('\n')
        .trim_end_matches('\r')
        .to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Replace the text of `token` in the file at `path` with `replacement`.
///
/// All arithmetic is performed in Unicode scalar-value space so that multi-byte
/// characters are handled correctly.
fn replace_in_file(path: &Path, token: &Token, replacement: &str) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "spellr: cannot read {:?} for replacement: {}",
                path, e
            );
            return;
        }
    };

    let chars: Vec<char> = content.chars().collect();
    let start = token.location.absolute_char_offset();
    let end = start + token.value.chars().count();

    if end > chars.len() {
        eprintln!(
            "spellr: replacement offset out of bounds in {:?} \
             (start={}, end={}, file_len={})",
            path,
            start,
            end,
            chars.len()
        );
        return;
    }

    let capacity = chars.len() - (end - start) + replacement.chars().count();
    let mut new_chars: Vec<char> = Vec::with_capacity(capacity);
    new_chars.extend_from_slice(&chars[..start]);
    new_chars.extend(replacement.chars());
    new_chars.extend_from_slice(&chars[end..]);

    let new_content: String = new_chars.into_iter().collect();
    if let Err(e) = std::fs::write(path, &new_content) {
        eprintln!("spellr: cannot write {:?}: {}", path, e);
    }
}
