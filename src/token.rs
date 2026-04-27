#![allow(dead_code)]

use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;

/// Location within a line (column)
#[derive(Debug, Clone)]
pub struct ColumnLocation {
    pub char_offset: usize,
    pub byte_offset: usize,
    pub line_number: usize,
    pub file: PathBuf,
    /// absolute char offset from start of file
    pub line_char_offset: usize,
    /// absolute byte offset from start of file
    pub line_byte_offset: usize,
}

impl ColumnLocation {
    pub fn absolute_char_offset(&self) -> usize {
        self.char_offset + self.line_char_offset
    }
    pub fn absolute_byte_offset(&self) -> usize {
        self.byte_offset + self.line_byte_offset
    }
}

impl std::fmt::Display for ColumnLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.file.display(),
            self.line_number,
            self.char_offset
        )
    }
}

/// A word token found in source code
#[derive(Debug, Clone)]
pub struct Token {
    pub value: String,
    pub location: ColumnLocation,
    pub line_content: String,
}

impl Token {
    pub fn new(value: String, location: ColumnLocation, line_content: String) -> Self {
        Self {
            value,
            location,
            line_content,
        }
    }

    /// Normalize: strip, lowercase, unicode-normalize, replace curly quotes with apostrophe
    pub fn normalized(&self) -> String {
        normalize_str(&self.value)
    }

    /// Determine the case style of this token's value
    pub fn case_method(&self) -> CaseMethod {
        let s = &self.value;
        if s.is_empty() {
            return CaseMethod::Lowercase;
        }
        let all_lower = s.chars().all(|c| !c.is_alphabetic() || c.is_lowercase());
        let all_upper = s.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
        let first_upper = s
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        let rest_lower = s.chars().skip(1).all(|c| !c.is_alphabetic() || c.is_lowercase());

        if all_lower {
            CaseMethod::Lowercase
        } else if all_upper {
            CaseMethod::Uppercase
        } else if first_upper && rest_lower {
            CaseMethod::Capitalize
        } else {
            CaseMethod::AsIs
        }
    }

    /// Apply the token's case style to a given word
    pub fn apply_case(&self, word: &str) -> String {
        self.case_method().apply(word)
    }

    /// Returns the line content with the token's value highlighted in red
    pub fn highlight_in_line(&self) -> String {
        let line = &self.line_content;
        let start = self.location.char_offset;
        let end = start + self.value.chars().count();
        let chars: Vec<char> = line.chars().collect();
        let before: String = chars[..start.min(chars.len())].iter().collect();
        let word: String =
            chars[start.min(chars.len())..end.min(chars.len())]
                .iter()
                .collect();
        let after: String = chars[end.min(chars.len())..].iter().collect();
        format!(
            "{}{}{}",
            before,
            crate::string_format::red(&word),
            after
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaseMethod {
    Lowercase,
    Uppercase,
    Capitalize,
    AsIs,
}

impl CaseMethod {
    pub fn apply(&self, word: &str) -> String {
        match self {
            CaseMethod::Lowercase => word.to_lowercase(),
            CaseMethod::Uppercase => word.to_uppercase(),
            CaseMethod::Capitalize => {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            }
            CaseMethod::AsIs => word.to_string(),
        }
    }
}

/// Normalize a string: trim, lowercase, NFC unicode normalize,
/// replace curly single quotes with ASCII apostrophe
pub fn normalize_str(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .nfc()
        .collect::<String>()
        .replace('\u{2019}', "'") // RIGHT SINGLE QUOTATION MARK → apostrophe
        .replace('\u{2018}', "'") // LEFT SINGLE QUOTATION MARK → apostrophe
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_location() -> ColumnLocation {
        ColumnLocation {
            char_offset: 0,
            byte_offset: 0,
            line_number: 1,
            file: PathBuf::from("test.rs"),
            line_char_offset: 0,
            line_byte_offset: 0,
        }
    }

    #[test]
    fn test_normalize_str() {
        assert_eq!(normalize_str("Hello"), "hello");
        assert_eq!(normalize_str("  World  "), "world");
        assert_eq!(normalize_str("it\u{2019}s"), "it's");
    }

    #[test]
    fn test_case_method_lowercase() {
        let tok = Token::new("hello".into(), make_location(), String::new());
        assert_eq!(tok.case_method(), CaseMethod::Lowercase);
    }

    #[test]
    fn test_case_method_uppercase() {
        let tok = Token::new("HELLO".into(), make_location(), String::new());
        assert_eq!(tok.case_method(), CaseMethod::Uppercase);
    }

    #[test]
    fn test_case_method_capitalize() {
        let tok = Token::new("Hello".into(), make_location(), String::new());
        assert_eq!(tok.case_method(), CaseMethod::Capitalize);
    }

    #[test]
    fn test_case_method_asis() {
        let tok = Token::new("hELLO".into(), make_location(), String::new());
        assert_eq!(tok.case_method(), CaseMethod::AsIs);
    }

    #[test]
    fn test_apply_case() {
        let tok = Token::new("Hello".into(), make_location(), String::new());
        assert_eq!(tok.apply_case("world"), "World");

        let tok2 = Token::new("HELLO".into(), make_location(), String::new());
        assert_eq!(tok2.apply_case("world"), "WORLD");
    }

    #[test]
    fn test_absolute_offsets() {
        let loc = ColumnLocation {
            char_offset: 5,
            byte_offset: 5,
            line_number: 3,
            file: PathBuf::from("foo.rs"),
            line_char_offset: 100,
            line_byte_offset: 100,
        };
        assert_eq!(loc.absolute_char_offset(), 105);
        assert_eq!(loc.absolute_byte_offset(), 105);
    }
}
