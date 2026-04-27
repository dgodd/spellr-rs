#![allow(dead_code)]

use fancy_regex::Regex as FancyRegex;
use once_cell::sync::Lazy;
use regex::Regex;

// ── Term recognition ──────────────────────────────────────────────────────────
//
// Note: [\u{2018}\u{2019}'] covers the straight apostrophe plus both curly
// single-quote characters (left U+2018, right U+2019).
//
// These patterns use look-behinds, so they require fancy-regex.

// [Word], [Word]Word, [Word]'s, [Wordn't]
const TITLE_CASE_PAT: &str =
    r"\p{Lu}\p{Ll}+(?:[\u{2018}\u{2019}']\p{Ll}+(?<![\u{2018}\u{2019}']s))*";

// [WORD], [WORD]Word, [WORDN'T], [WORD]'S, [WORD]'s, [WORD]s
const UPPER_CASE_PAT: &str =
    r"\p{Lu}+(?:[\u{2018}\u{2019}']\p{Lu}+(?<![\u{2018}\u{2019}'][Ss]))*(?:(?!\p{Ll})|(?=s(?!\p{Ll})))";

// [word], [word]'s, [wordn't]
const LOWER_CASE_PAT: &str =
    r"\p{Ll}+(?:[\u{2018}\u{2019}']\p{Ll}+(?<![\u{2018}\u{2019}']s))*";

// Characters in \p{L} that are neither \p{Ll} nor \p{Lu} (e.g. Arabic, Devanagari)
const OTHER_CASE_PAT: &str = r"(?:\p{L}(?<![\p{Ll}\p{Lu}]))+";

/// Recognises a single word-token in title, upper, lower, or other case.
/// Requires fancy-regex because of the variable-length look-behinds.
pub static TERM_RE: Lazy<FancyRegex> = Lazy::new(|| {
    let pat = format!(
        "(?:{title}|{upper}|{lower}|{other})",
        title = TITLE_CASE_PAT,
        upper = UPPER_CASE_PAT,
        lower = LOWER_CASE_PAT,
        other = OTHER_CASE_PAT,
    );
    FancyRegex::new(&pat).expect("TERM_RE pattern is invalid")
});

// ── Non-word skip patterns ────────────────────────────────────────────────────

// Anything that is NOT a Unicode letter, /, %, #, 0-9, or backslash
const NOT_EVEN_NON_WORDS_PAT: &str = r"[^\p{L}/%#0-9\\]+";

// ANSI/terminal colour escapes: \e[31m or \033[1;32m
const SHELL_COLOR_ESCAPE_PAT: &str = r"\\(?:e|0?33)\[\d+(?:;\d+)*m";

// Single backslash + one ASCII letter: \n, \t, \r, ...
const BACKSLASH_ESCAPE_PAT: &str = r"\\[a-zA-Z]";

// Percent-encoded bytes: %0A, %2F ... (0-8 + uppercase A-F, matching the Ruby original)
const URL_ENCODED_ENTITIES_PAT: &str = r"%[0-8A-F]{2}";

// CSS colour literals and 0x... numeric literals; look-ahead guards alpha continuation
const HEX_PAT: &str =
    r"(?:#(?:[0-9a-fA-F]{6}|[0-9a-fA-F]{3})|0x[0-9a-fA-F]+)(?![[:alpha:]])";

// ── URL sub-patterns ──────────────────────────────────────────────────────────
// Ruby's \h (hex digit) -> [0-9a-fA-F] in Rust

const URL_SCHEME_PAT: &str = r"(?://|https?://|s?ftp://|mailto:)";
const URL_USERINFO_PAT: &str = r"[[:alnum:]]+(?::[[:alnum:]]+)?@";
const URL_HOSTNAME_PAT: &str =
    r"(?:[[:alnum:]\-\\]+(?:\.[[:alnum:]\-\\]+)+|localhost|\d{1,3}(?:\.\d{1,3}){3})";
const URL_PORT_PAT: &str = r":\d+";
const URL_PATH_PAT: &str = r"/(?:[[:alnum:]=@!$&~\-/._\\]|%[0-9a-fA-F]{2})*";
const URL_QUERY_PART_PAT: &str = r"(?:[[:alnum:]=!$\-/._\\]|%[0-9a-fA-F]{2})+";
const URL_FRAGMENT_PAT: &str = r"#(?:[[:alnum:]=!$&\-/.\\]|%[0-9a-fA-F]{2})+";

fn url_query_pat() -> String {
    format!(r"\?{qp}(?:&{qp})*", qp = URL_QUERY_PART_PAT)
}

fn url_rest_pat() -> String {
    format!(
        "(?:{q})?(?:{f})?",
        q = url_query_pat(),
        f = URL_FRAGMENT_PAT
    )
}

/// Builds the full URL pattern as the union of three variants (matching the Ruby original):
///  1. Scheme + optional userinfo + hostname + optional port/path/query/fragment
///  2. Userinfo + hostname + optional port/path/query/fragment
///  3. Hostname + mandatory path + optional port/query/fragment
fn url_re_pat() -> String {
    let rest = url_rest_pat();
    format!(
        "(?:{scheme}(?:{userinfo})?{host}(?:{port})?(?:{path})?{rest}|{userinfo}{host}(?:{port})?(?:{path})?{rest}|{host}(?:{port})?{path}{rest})",
        scheme   = URL_SCHEME_PAT,
        userinfo = URL_USERINFO_PAT,
        host     = URL_HOSTNAME_PAT,
        port     = URL_PORT_PAT,
        path     = URL_PATH_PAT,
        rest     = rest,
    )
}

// ── API-key literal patterns ──────────────────────────────────────────────────
// Ruby's \w -> [A-Za-z0-9_], \h -> [0-9a-fA-F]

const KEY_SENDGRID_PAT: &str = r"SG\.[A-Za-z0-9_\-]{22}\.[A-Za-z0-9_\-]{43}";
const KEY_HYPERWALLET_PAT: &str =
    r"prg-[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";
const KEY_GTM_PAT: &str = r"GTM-[A-Z0-9]{7}";
const KEY_SHA1_PAT: &str = r"sha1-[A-Za-z0-9=+/]{28}";
const KEY_SHA512_PAT: &str = r"sha512-[A-Za-z0-9=;+/]{88}";
// data: URI with base64 payload - look-ahead prevents eating trailing alnum
const KEY_DATA_URL_PAT: &str =
    r"data:[a-z/;0-9\-]+;base64,[A-Za-z0-9+/]+=*(?![[:alnum:]])";

fn key_patterns_pat() -> String {
    format!(
        "(?:{sg}|{hw}|{gtm}|{sha1}|{sha512}|{data})",
        sg     = KEY_SENDGRID_PAT,
        hw     = KEY_HYPERWALLET_PAT,
        gtm    = KEY_GTM_PAT,
        sha1   = KEY_SHA1_PAT,
        sha512 = KEY_SHA512_PAT,
        data   = KEY_DATA_URL_PAT,
    )
}

/// Master skip pattern: consume sequences that contain no spell-checkable words.
/// Requires fancy-regex because HEX_PAT and KEY_DATA_URL_PAT contain look-aheads.
pub static SKIPS_RE: Lazy<FancyRegex> = Lazy::new(|| {
    let pat = format!(
        "(?:{non_word}|{shell}|{backslash}|{url_enc}|{hex}|{url}|{key})",
        non_word  = NOT_EVEN_NON_WORDS_PAT,
        shell     = SHELL_COLOR_ESCAPE_PAT,
        backslash = BACKSLASH_ESCAPE_PAT,
        url_enc   = URL_ENCODED_ENTITIES_PAT,
        hex       = HEX_PAT,
        url       = url_re_pat(),
        key       = key_patterns_pat(),
    );
    FancyRegex::new(&pat).expect("SKIPS_RE pattern is invalid")
});

// ── After-key skip patterns ───────────────────────────────────────────────────

// Leftover gateway characters and runs of digits
const LEFTOVER_NON_WORD_BITS_PAT: &str = r"[/%#\\]|\d+";

// Repeated single letters like "xxxxxxxx" - backreference requires fancy-regex
const REPEATED_SINGLE_LETTERS_PAT: &str = r"(?:(\p{L})\1+)(?!\p{L})";

// Sequential alphabet runs: a, ab, abc, ... abcdefghijklmnopqrstuvwxyz (lowercase only).
// Full expansion of the Ruby /a(?:b(?:c...yz?)?...)?(?![[:alpha:]])/i pattern.
// We drop the /i flag here; matching only lowercase is acceptable because TERM_RE already
// handles uppercase runs, and sequential uppercase is rarely a false-negative in practice.
// Structure: 23 optional nested groups (b..x), then (?:yz?) as the innermost, giving
// exactly 24 (?:  groups + 1 (?!  group = 25 opens, matched by 25 closes.
const SEQUENTIAL_LETTERS_PAT: &str =
    r"(?i:a(?:b(?:c(?:d(?:e(?:f(?:g(?:h(?:i(?:j(?:k(?:l(?:m(?:n(?:o(?:p(?:q(?:r(?:s(?:t(?:u(?:v(?:w(?:x(?:yz?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?)?(?!\p{L}))";

/// Skip pattern applied after the key-heuristic pass.
/// Requires fancy-regex for the backreference in REPEATED_SINGLE_LETTERS
/// and the look-ahead in SEQUENTIAL_LETTERS.
pub static AFTER_KEY_SKIPS_RE: Lazy<FancyRegex> = Lazy::new(|| {
    let pat = format!(
        "(?:{leftover}|{repeated}|{sequential})",
        leftover   = LEFTOVER_NON_WORD_BITS_PAT,
        repeated   = REPEATED_SINGLE_LETTERS_PAT,
        sequential = SEQUENTIAL_LETTERS_PAT,
    );
    FancyRegex::new(&pat).expect("AFTER_KEY_SKIPS_RE pattern is invalid")
});

// ── Possible API-key heuristic ────────────────────────────────────────────────

// "Three-chunk" rule: the key must have alternating alpha/digit segments (>= 3 chunks).
// Anchored implicitly by checking m.start() == 0 in the scanner.
const ALPHA_SEP_PAT: &str = r"[A-Za-z][A-Za-z\-_/+]*";
const NUM_SEP_PAT: &str = r"\d[\d\-_/+]*";

fn three_chunk_pat() -> String {
    format!(
        "(?:{a}{n}{a}|{n}{a}{n})",
        a = ALPHA_SEP_PAT,
        n = NUM_SEP_PAT,
    )
}

/// Heuristic pattern that spots potential API keys before the Naive Bayes classifier.
/// Requires fancy-regex because of the look-ahead.
pub static POSSIBLE_KEY_RE: Lazy<FancyRegex> = Lazy::new(|| {
    let pat = format!(
        r"{chunk}[A-Za-z0-9+/\-_]*=*(?![[:alnum:]])",
        chunk = three_chunk_pat(),
    );
    FancyRegex::new(&pat).expect("POSSIBLE_KEY_RE pattern is invalid")
});

// ── spellr control comments ───────────────────────────────────────────────────

/// Matches `spellr:disable` - disables spell-checking until re-enabled.
pub static SPELLR_DISABLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"spellr:disable").expect("SPELLR_DISABLE_RE invalid"));

/// Matches `spellr:enable` - re-enables spell-checking.
pub static SPELLR_ENABLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"spellr:enable").expect("SPELLR_ENABLE_RE invalid"));

/// Matches `spellr:disable-line` or `spellr:disable:line` - disables an entire line.
pub static SPELLR_LINE_DISABLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"spellr:disable[-:]line").expect("SPELLR_LINE_DISABLE_RE invalid")
});

// ── Helper ────────────────────────────────────────────────────────────────────

/// Returns a plain regex that matches strings containing at least one alphabetic
/// run of exactly `min_len` characters in any standard case style.
///
/// This is used to pre-filter candidates before the Naive Bayes classifier.
pub fn min_alpha_re(min_len: usize) -> Regex {
    // Title-case prefix: one uppercase + (min_len-1) lowercase
    // Lower-case run:    min_len lowercase letters
    // Upper-case run:    min_len uppercase letters
    let pat = format!(
        "[A-Z][a-z]{{{lower}}}|[a-z]{{{all}}}|[A-Z]{{{all}}}",
        lower = min_len.saturating_sub(1),
        all   = min_len,
    );
    Regex::new(&pat).expect("min_alpha_re pattern is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn term_re_matches_lowercase() {
        let re = &*TERM_RE;
        let m = re.find("hello world").unwrap().unwrap();
        assert_eq!(m.as_str(), "hello");
    }

    #[test]
    fn term_re_matches_uppercase() {
        let re = &*TERM_RE;
        let m = re.find("HELLO").unwrap().unwrap();
        assert_eq!(m.as_str(), "HELLO");
    }

    #[test]
    fn term_re_matches_title_case() {
        let re = &*TERM_RE;
        let m = re.find("Hello world").unwrap().unwrap();
        assert_eq!(m.as_str(), "Hello");
    }

    #[test]
    fn term_re_possessive() {
        // "it's" -> the 's is excluded because of the look-behind
        let re = &*TERM_RE;
        let m = re.find("it's").unwrap().unwrap();
        assert_eq!(m.as_str(), "it");
    }

    #[test]
    fn term_re_contraction() {
        // "don't" -> kept whole (look-behind only rejects trailing 's)
        let re = &*TERM_RE;
        let m = re.find("don't").unwrap().unwrap();
        assert_eq!(m.as_str(), "don't");
    }

    #[test]
    fn skips_re_matches_whitespace() {
        let re = &*SKIPS_RE;
        assert!(re.is_match("   ").unwrap());
    }

    #[test]
    fn skips_re_matches_hex_literal() {
        let re = &*SKIPS_RE;
        let text = "#ff0000 ";
        let m = re.find(text).unwrap().unwrap();
        assert_eq!(m.start(), 0);
    }

    #[test]
    fn spellr_control_patterns() {
        assert!(SPELLR_DISABLE_RE.is_match("# spellr:disable"));
        assert!(SPELLR_ENABLE_RE.is_match("# spellr:enable"));
        assert!(SPELLR_LINE_DISABLE_RE.is_match("# spellr:disable-line"));
        assert!(SPELLR_LINE_DISABLE_RE.is_match("# spellr:disable:line"));
    }

    #[test]
    fn min_alpha_re_works() {
        let re = min_alpha_re(3);
        assert!(re.is_match("abc"));
        assert!(re.is_match("ABC"));
        assert!(re.is_match("Abc"));
        assert!(!re.is_match("ab"));
        assert!(!re.is_match("AB"));
    }

    #[test]
    fn possible_key_re_matches_key_like_strings() {
        let re = &*POSSIBLE_KEY_RE;
        // alpha-num-alpha three-chunk
        let m = re.find("abc123def ").unwrap();
        assert!(m.is_some());
    }

    #[test]
    fn after_key_skips_repeated_letters() {
        let re = &*AFTER_KEY_SKIPS_RE;
        // "xxxxxx" not followed by alpha -> should match
        let m = re.find("xxxxxx ").unwrap();
        assert!(m.is_some());
        assert_eq!(m.unwrap().start(), 0);
    }

    #[test]
    fn after_key_skips_sequential() {
        let re = &*AFTER_KEY_SKIPS_RE;
        // lowercase sequential run "abcde" not followed by alpha -> should match at pos 0
        let m = re.find("abcde ").unwrap();
        assert!(m.is_some());
        assert_eq!(m.unwrap().start(), 0);
    }

    #[test]
    fn after_key_skips_sequential_uppercase() {
        let re = &*AFTER_KEY_SKIPS_RE;
        // uppercase sequential run "ABCD" not followed by alpha -> should match at pos 0
        // (Ruby's SEQUENTIAL_LETTERS_RE uses the /i flag)
        let m = re.find("ABCD ").unwrap();
        assert!(m.is_some());
        assert_eq!(m.unwrap().start(), 0);
    }
}
