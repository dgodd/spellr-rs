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
