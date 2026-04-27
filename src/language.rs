#![allow(dead_code)]

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::{Config, LanguageConfig};
use crate::wordlist::Wordlist;

// ── Language ──────────────────────────────────────────────────────────────────

/// A single spellr language (e.g. "ruby", "english", "shell").
///
/// Mirrors the Ruby `Spellr::Language` class.
pub struct Language {
    pub name: String,
    /// Single-character key used in interactive mode to pick a wordlist.
    pub key: String,
    /// Whether the project wordlist for this language can be written to.
    pub addable: bool,
    /// Compiled glob patterns for `includes` (may be empty → match all files).
    includes_globs: GlobSet,
    /// Whether `includes_globs` was built from at least one pattern.
    has_includes: bool,
    /// Shebang strings that trigger this language (e.g. `["ruby"]`).
    hashbangs: Vec<String>,
    /// Locale suffixes for per-locale wordlists (e.g. `["US"]`).
    locales: Vec<String>,
    /// Path to the directory that holds the bundled (gem) wordlists.
    wordlists_dir: PathBuf,
    /// Current working directory (project root for project wordlists).
    project_dir: PathBuf,
}

impl Language {
    // ── Construction ──────────────────────────────────────────────────────────

    /// Build a `Language` from a parsed `LanguageConfig` entry.
    pub fn from_config(
        name: &str,
        config: &LanguageConfig,
        wordlists_dir: &Path,
        project_dir: &Path,
    ) -> Self {
        // Key: explicit value, or the first character of the language name.
        let key = config
            .key
            .clone()
            .unwrap_or_else(|| name.chars().next().unwrap_or('?').to_string());

        let addable = config.addable.unwrap_or(true);
        let hashbangs = config.hashbangs.clone().unwrap_or_default();

        // Decode locale(s) – may be a single string or a YAML sequence.
        let locales = match &config.locale {
            Some(serde_yaml::Value::String(s)) => vec![s.clone()],
            Some(serde_yaml::Value::Sequence(seq)) => seq
                .iter()
                .filter_map(|v| {
                    if let serde_yaml::Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => vec![],
        };

        // Compile glob patterns from `includes`.
        //
        // Rules (matching Ruby fast_ignore behaviour):
        //   • Pattern without a `/` → match the filename anywhere in the tree
        //     by prepending `**/`.
        //   • Pattern with a `/` → keep as-is (anchored or relative).
        let mut builder = GlobSetBuilder::new();
        let mut has_includes = false;

        if let Some(includes) = &config.includes {
            for pattern in includes {
                has_includes = true;
                let glob_pattern = if pattern.contains('/') {
                    // Already path-qualified – try as-is, and also with `**/`
                    // prefix so it matches regardless of where the project root
                    // sits.
                    pattern.clone()
                } else {
                    // Simple glob like `*.rb` → match anywhere in the tree.
                    format!("**/{pattern}")
                };

                if let Ok(g) = Glob::new(&glob_pattern) {
                    builder.add(g);
                }
                // Also add the bare pattern so it matches relative paths that
                // happen to start from the project root.
                if glob_pattern != *pattern {
                    if let Ok(g) = Glob::new(pattern) {
                        builder.add(g);
                    }
                }
            }
        }

        let includes_globs = builder
            .build()
            .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap());

        Self {
            name: name.to_string(),
            key,
            addable,
            includes_globs,
            has_includes,
            hashbangs,
            locales,
            wordlists_dir: wordlists_dir.to_path_buf(),
            project_dir: project_dir.to_path_buf(),
        }
    }

    // ── File matching ─────────────────────────────────────────────────────────

    /// Return `true` if this language applies to the given file.
    ///
    /// Matching rules (in priority order):
    ///  1. If `includes` is empty **and** no hashbangs are configured → match
    ///     every file (e.g. the "english" language).
    ///  2. Check the file path against the compiled `includes_globs`.
    ///  3. If `first_line` starts with `#!` and contains one of the configured
    ///     hashbang strings → match.
    pub fn matches_file(&self, path: &Path, first_line: Option<&str>) -> bool {
        let has_hashbangs = !self.hashbangs.is_empty();

        // Rule 1: no restrictions at all → catch-all language.
        if !self.has_includes && !has_hashbangs {
            return true;
        }

        // Normalise the path to something relative for glob matching.
        let rel_path: PathBuf = if path.is_absolute() {
            path.strip_prefix(&self.project_dir)
                .unwrap_or(path)
                .to_path_buf()
        } else {
            path.to_path_buf()
        };

        // Rule 2: glob match on the (relative) path.
        if self.has_includes && self.includes_globs.is_match(&rel_path) {
            return true;
        }

        // Fallback: try matching just the filename component so that absolute
        // paths that couldn't be stripped still work.
        if self.has_includes {
            if let Some(file_name) = path.file_name() {
                let file_name_path = Path::new(file_name);
                if self.includes_globs.is_match(file_name_path) {
                    return true;
                }
            }
        }

        // Rule 3: shebang check.
        if has_hashbangs {
            if let Some(line) = first_line {
                if line.starts_with("#!") {
                    for hb in &self.hashbangs {
                        if line.contains(hb.as_str()) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    // ── Wordlist access ───────────────────────────────────────────────────────

    /// Return all wordlists that **exist on disk** for this language.
    ///
    /// The order matches the Ruby implementation:
    ///   1. Bundled (gem) wordlist
    ///   2. Per-locale wordlists
    ///   3. Project wordlist (`<project>/.spellr_wordlists/<name>.txt`)
    pub fn wordlists(&self) -> Vec<Wordlist> {
        self.default_wordlists()
            .into_iter()
            .filter(|w| w.exists())
            .collect()
    }

    /// Return the project-local wordlist for this language.
    ///
    /// The file is located at `<project_dir>/.spellr_wordlists/<name>.txt`.
    /// It may not exist yet (callers should use [`Wordlist::exists`] to check).
    pub fn project_wordlist(&self) -> Wordlist {
        let path = self
            .project_dir
            .join(".spellr_wordlists")
            .join(format!("{}.txt", self.name));
        Wordlist::new(path, self.name.clone())
    }

    /// Return the bundled (gem / binary-local) wordlist for this language.
    fn gem_wordlist(&self) -> Wordlist {
        let path = self
            .wordlists_dir
            .join(format!("{}.txt", self.name));
        Wordlist::new(path.clone(), path.display().to_string())
    }

    /// Return per-locale wordlists (e.g. `wordlists/english/US.txt`).
    fn locale_wordlists(&self) -> Vec<Wordlist> {
        self.locales
            .iter()
            .map(|locale| {
                let path = self
                    .wordlists_dir
                    .join(&self.name)
                    .join(format!("{locale}.txt"));
                Wordlist::new(path.clone(), path.display().to_string())
            })
            .collect()
    }

    /// Return all candidate wordlists in priority order (existence not checked).
    fn default_wordlists(&self) -> Vec<Wordlist> {
        let mut wls = vec![self.gem_wordlist()];
        wls.extend(self.locale_wordlists());
        wls.push(self.project_wordlist());
        wls
    }
}

// ── Module-level helpers ──────────────────────────────────────────────────────

/// Build a `Vec<Language>` from the resolved `Config`.
pub fn languages_from_config(
    config: &Config,
    wordlists_dir: &Path,
    project_dir: &Path,
) -> Vec<Language> {
    config
        .languages
        .iter()
        .map(|(name, lang_cfg)| {
            Language::from_config(name, lang_cfg, wordlists_dir, project_dir)
        })
        .collect()
}

/// Return all wordlists (that exist on disk) for languages that match `path`.
pub fn wordlists_for_file(
    languages: &[Language],
    path: &Path,
    first_line: Option<&str>,
) -> Vec<Wordlist> {
    let mut wordlists: Vec<Wordlist> = Vec::new();
    // Use a set to deduplicate paths so we don't check the same file twice.
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for lang in languages {
        if lang.matches_file(path, first_line) {
            for wl in lang.wordlists() {
                if seen.insert(wl.path.clone()) {
                    wordlists.push(wl);
                }
            }
        }
    }
    wordlists
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LanguageConfig;

    fn make_config(includes: Option<Vec<&str>>, hashbangs: Option<Vec<&str>>) -> LanguageConfig {
        LanguageConfig {
            key: None,
            includes: includes.map(|v| v.into_iter().map(|s| s.to_string()).collect()),
            hashbangs: hashbangs.map(|v| v.into_iter().map(|s| s.to_string()).collect()),
            locale: None,
            addable: None,
        }
    }

    fn lang(name: &str, cfg: LanguageConfig) -> Language {
        Language::from_config(
            name,
            &cfg,
            Path::new("/wordlists"),
            Path::new("/project"),
        )
    }

    #[test]
    fn catch_all_language_matches_everything() {
        let l = lang("english", make_config(None, None));
        assert!(l.matches_file(Path::new("anything.txt"), None));
        assert!(l.matches_file(Path::new("foo/bar.rs"), None));
    }

    #[test]
    fn glob_match_by_extension() {
        let l = lang("ruby", make_config(Some(vec!["*.rb"]), None));
        assert!(l.matches_file(Path::new("app/models/user.rb"), None));
        assert!(!l.matches_file(Path::new("script.py"), None));
    }

    #[test]
    fn glob_match_exact_filename() {
        let l = lang("ruby", make_config(Some(vec!["Gemfile"]), None));
        assert!(l.matches_file(Path::new("/project/Gemfile"), None));
        assert!(!l.matches_file(Path::new("/project/Gemfile.lock"), None));
    }

    #[test]
    fn hashbang_match() {
        let l = lang("ruby", make_config(None, Some(vec!["ruby"])));
        let shebang = "#!/usr/bin/env ruby";
        assert!(l.matches_file(Path::new("script"), Some(shebang)));
        assert!(!l.matches_file(Path::new("script"), Some("#!/usr/bin/env python")));
    }

    #[test]
    fn hashbang_only_when_line_starts_with_shebang() {
        let l = lang("shell", make_config(None, Some(vec!["bash"])));
        // First line does not start with #! so should not match.
        assert!(!l.matches_file(Path::new("notes.txt"), Some("bash is great")));
    }

    #[test]
    fn key_defaults_to_first_char_of_name() {
        let l = lang("ruby", make_config(None, None));
        assert_eq!(l.key, "r");
    }

    #[test]
    fn explicit_key_overrides_default() {
        let mut cfg = make_config(None, None);
        cfg.key = Some("X".to_string());
        let l = lang("ruby", cfg);
        assert_eq!(l.key, "X");
    }
}
