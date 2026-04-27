//! Wordlist files compiled into the binary at build time via `include_str!`.

use std::collections::HashSet;
use std::sync::Arc;

use once_cell::sync::Lazy;

macro_rules! w {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/wordlists/", $path))
    };
}

// Top-level language wordlists
pub const CSS:        &str = w!("css.txt");
pub const DOCKERFILE: &str = w!("dockerfile.txt");
pub const ENGLISH:    &str = w!("english.txt");
pub const HTML:       &str = w!("html.txt");
pub const JAVASCRIPT: &str = w!("javascript.txt");
pub const RUBY:       &str = w!("ruby.txt");
pub const SHELL:      &str = w!("shell.txt");
pub const SPELLR:     &str = w!("spellr.txt");

// English locale wordlists
pub const ENGLISH_AU:  &str = w!("english/AU.txt");
pub const ENGLISH_CA:  &str = w!("english/CA.txt");
pub const ENGLISH_GB:  &str = w!("english/GB.txt");
pub const ENGLISH_GBS: &str = w!("english/GBs.txt");
pub const ENGLISH_GBZ: &str = w!("english/GBz.txt");
pub const ENGLISH_US:  &str = w!("english/US.txt");

// ── Cached Arc<HashSet<String>> for each wordlist ─────────────────────────────

fn parse_to_arc_set(content: &'static str) -> Arc<HashSet<String>> {
    Arc::new(
        content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect(),
    )
}

static CSS_SET:        Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(CSS));
static DOCKERFILE_SET: Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(DOCKERFILE));
static ENGLISH_SET:    Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(ENGLISH));
static HTML_SET:       Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(HTML));
static JAVASCRIPT_SET: Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(JAVASCRIPT));
static RUBY_SET:       Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(RUBY));
static SHELL_SET:      Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(SHELL));
static SPELLR_SET:     Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(SPELLR));

static ENGLISH_AU_SET:  Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(ENGLISH_AU));
static ENGLISH_CA_SET:  Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(ENGLISH_CA));
static ENGLISH_GB_SET:  Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(ENGLISH_GB));
static ENGLISH_GBS_SET: Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(ENGLISH_GBS));
static ENGLISH_GBZ_SET: Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(ENGLISH_GBZ));
static ENGLISH_US_SET:  Lazy<Arc<HashSet<String>>> = Lazy::new(|| parse_to_arc_set(ENGLISH_US));

// ── Lookup functions ──────────────────────────────────────────────────────────

/// Return the embedded content for a top-level language wordlist.
pub fn get(language: &str) -> Option<&'static str> {
    match language {
        "css"        => Some(CSS),
        "dockerfile" => Some(DOCKERFILE),
        "english"    => Some(ENGLISH),
        "html"       => Some(HTML),
        "javascript" => Some(JAVASCRIPT),
        "ruby"       => Some(RUBY),
        "shell"      => Some(SHELL),
        "spellr"     => Some(SPELLR),
        _            => None,
    }
}

/// Return the embedded content for a locale-specific wordlist.
pub fn get_locale(language: &str, locale: &str) -> Option<&'static str> {
    match (language, locale) {
        ("english", "AU")  => Some(ENGLISH_AU),
        ("english", "CA")  => Some(ENGLISH_CA),
        ("english", "GB")  => Some(ENGLISH_GB),
        ("english", "GBs") => Some(ENGLISH_GBS),
        ("english", "GBz") => Some(ENGLISH_GBZ),
        ("english", "US")  => Some(ENGLISH_US),
        _                  => None,
    }
}

/// Return the pre-built, globally-cached word-set for a top-level language
/// wordlist.  The `Arc` is cheap to clone (reference-count bump only).
pub fn get_set(language: &str) -> Option<Arc<HashSet<String>>> {
    match language {
        "css"        => Some(Arc::clone(&CSS_SET)),
        "dockerfile" => Some(Arc::clone(&DOCKERFILE_SET)),
        "english"    => Some(Arc::clone(&ENGLISH_SET)),
        "html"       => Some(Arc::clone(&HTML_SET)),
        "javascript" => Some(Arc::clone(&JAVASCRIPT_SET)),
        "ruby"       => Some(Arc::clone(&RUBY_SET)),
        "shell"      => Some(Arc::clone(&SHELL_SET)),
        "spellr"     => Some(Arc::clone(&SPELLR_SET)),
        _            => None,
    }
}

/// Return the pre-built, globally-cached word-set for a locale-specific
/// wordlist.
pub fn get_locale_set(language: &str, locale: &str) -> Option<Arc<HashSet<String>>> {
    match (language, locale) {
        ("english", "AU")  => Some(Arc::clone(&ENGLISH_AU_SET)),
        ("english", "CA")  => Some(Arc::clone(&ENGLISH_CA_SET)),
        ("english", "GB")  => Some(Arc::clone(&ENGLISH_GB_SET)),
        ("english", "GBs") => Some(Arc::clone(&ENGLISH_GBS_SET)),
        ("english", "GBz") => Some(Arc::clone(&ENGLISH_GBZ_SET)),
        ("english", "US")  => Some(Arc::clone(&ENGLISH_US_SET)),
        _                  => None,
    }
}
