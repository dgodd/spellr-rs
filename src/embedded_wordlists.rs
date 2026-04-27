//! Wordlist files compiled into the binary at build time via `include_str!`.

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
