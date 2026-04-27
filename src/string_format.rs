use std::path::{Path, PathBuf};

/// Return `path` relative to the current working directory, falling back to
/// the original path if the cwd cannot be determined or the path is not under it.
pub fn relative_path(path: &Path) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    path.strip_prefix(&cwd)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| path.to_path_buf())
}

pub fn pluralize(word: &str, count: usize) -> String {
    if count == 1 {
        format!("{} {}", count, word)
    } else {
        format!("{} {}s", count, word)
    }
}

pub fn normal(text: &str) -> String {
    format!("\x1b[0m{}", text)
}

pub fn aqua(text: &str) -> String {
    format!("\x1b[36m{}\x1b[0m", text)
}

pub fn bold(text: &str) -> String {
    format!("\x1b[1;39m{}\x1b[0m", text)
}

pub fn lighten(text: &str) -> String {
    format!("\x1b[2;39m{}\x1b[0m", text)
}

pub fn red(text: &str) -> String {
    format!("\x1b[1;31m{}\x1b[0m", text)
}

pub fn green(text: &str) -> String {
    format!("\x1b[1;32m{}\x1b[0m", text)
}

/// Formats a label as a keyboard key hint: "[b]old label"
pub fn key_label(label: &str) -> String {
    let mut chars = label.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let rest: String = chars.collect();
            format!("[{}]{}", bold(&first.to_string()), rest)
        }
    }
}
