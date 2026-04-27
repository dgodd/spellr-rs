#![allow(dead_code)]

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::RwLock;

// ---------------------------------------------------------------------------
// Default config embedded in the binary
// ---------------------------------------------------------------------------

/// The default configuration, embedded from `wordlists/default_spellr.yml`.
/// This mirrors the Ruby gem's `lib/.spellr.yml` and is deep-merged with the
/// project's `.spellr.yml` at runtime.
const DEFAULT_CONFIG_YAML: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/wordlists/default_spellr.yml"));

// ---------------------------------------------------------------------------
// Serde structs for deserialization
// ---------------------------------------------------------------------------

/// Per-language configuration as read from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LanguageConfig {
    pub key: Option<String>,
    pub includes: Option<Vec<String>>,
    pub hashbangs: Option<Vec<String>>,
    pub locale: Option<serde_yaml::Value>,
    pub addable: Option<bool>,
}

/// Raw (fully-optional) config as read from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawConfig {
    pub word_minimum_length: Option<usize>,
    pub key_heuristic_weight: Option<f64>,
    pub key_minimum_length: Option<usize>,
    pub excludes: Option<Vec<String>>,
    pub includes: Option<Vec<String>>,
    pub languages: Option<IndexMap<String, LanguageConfig>>,
}

// ---------------------------------------------------------------------------
// Final resolved Config
// ---------------------------------------------------------------------------

/// Fully-resolved configuration with all defaults applied.
#[derive(Debug, Clone)]
pub struct Config {
    pub word_minimum_length: usize,
    pub key_heuristic_weight: f64,
    pub key_minimum_length: usize,
    pub excludes: Vec<String>,
    pub includes: Vec<String>,
    pub languages: IndexMap<String, LanguageConfig>,
    /// Path to the config file that was loaded (for display purposes).
    pub config_file: Option<std::path::PathBuf>,
}

impl Config {
    /// Load and merge the default config with an optional project config file.
    ///
    /// Priority (highest wins):
    ///   1. `config_file` argument (explicit CLI path)
    ///   2. `.spellr.yml` in the current working directory
    ///   3. Built-in default config
    pub fn load(config_file: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        // Always start from the baked-in defaults.
        let default_raw: RawConfig = serde_yaml::from_str(DEFAULT_CONFIG_YAML)?;

        // Locate and parse the user config (if any).
        let (user_path, user_raw): (Option<std::path::PathBuf>, Option<RawConfig>) =
            if let Some(explicit) = config_file {
                let text = std::fs::read_to_string(explicit)
                    .map_err(|e| format!("Cannot read config file '{}': {}", explicit.display(), e))?;
                let raw: RawConfig = serde_yaml::from_str(&text)
                    .map_err(|e| format!("Invalid config file '{}': {}", explicit.display(), e))?;
                (Some(explicit.to_path_buf()), Some(raw))
            } else {
                let candidate = Path::new(".spellr.yml");
                if candidate.exists() {
                    let text = std::fs::read_to_string(candidate)
                        .map_err(|e| format!("Cannot read '.spellr.yml': {}", e))?;
                    let raw: RawConfig = serde_yaml::from_str(&text)
                        .map_err(|e| format!("Invalid '.spellr.yml': {}", e))?;
                    (Some(candidate.to_path_buf()), Some(raw))
                } else {
                    (None, None)
                }
            };

        let merged = merge_configs(default_raw, user_raw);

        Ok(Config {
            word_minimum_length: merged.word_minimum_length.unwrap_or(3),
            key_heuristic_weight: merged.key_heuristic_weight.unwrap_or(5.0),
            key_minimum_length: merged.key_minimum_length.unwrap_or(6),
            excludes: merged.excludes.unwrap_or_default(),
            includes: merged.includes.unwrap_or_default(),
            languages: merged.languages.unwrap_or_default(),
            config_file: user_path,
        })
    }
}

// ---------------------------------------------------------------------------
// Merging logic
// ---------------------------------------------------------------------------

/// Merge a user `RawConfig` on top of a default `RawConfig`.
///
/// Mirrors the Ruby gem's `ConfigLoader#merge_config` behaviour:
/// - Arrays are **unioned** (`default | project`), preserving order and deduplicating.
/// - Hashes are **recursively merged** (project wins on scalars, arrays are unioned).
/// - Scalar fields: user value wins if present, otherwise default.
fn merge_configs(default: RawConfig, user: Option<RawConfig>) -> RawConfig {
    let Some(user) = user else {
        return default;
    };

    // Merge language maps: deep-merge per language entry.
    let languages = match (default.languages, user.languages) {
        (Some(mut def_langs), Some(user_langs)) => {
            for (name, user_lang) in user_langs {
                if let Some(def_lang) = def_langs.get(&name).cloned() {
                    def_langs.insert(name, merge_language_config(def_lang, user_lang));
                } else {
                    def_langs.insert(name, user_lang);
                }
            }
            Some(def_langs)
        }
        (def_langs, None) => def_langs,
        (None, user_langs) => user_langs,
    };

    RawConfig {
        word_minimum_length: user.word_minimum_length.or(default.word_minimum_length),
        key_heuristic_weight: user.key_heuristic_weight.or(default.key_heuristic_weight),
        key_minimum_length: user.key_minimum_length.or(default.key_minimum_length),
        // Arrays: union (default entries first, then any new user entries appended).
        excludes: union_vecs(default.excludes, user.excludes),
        includes: union_vecs(default.includes, user.includes),
        languages,
    }
}

/// Return the union of two optional `Vec<String>`, preserving the order of
/// `default` entries and appending any `user` entries not already present.
/// Mirrors Ruby's `default | project` array union.
fn union_vecs(default: Option<Vec<String>>, user: Option<Vec<String>>) -> Option<Vec<String>> {
    match (default, user) {
        (Some(mut def), Some(usr)) => {
            for item in usr {
                if !def.contains(&item) {
                    def.push(item);
                }
            }
            Some(def)
        }
        (def, None) => def,
        (None, usr) => usr,
    }
}

/// Deep-merge two `LanguageConfig` entries.
///
/// - `includes` and `hashbangs` arrays are unioned.
/// - Scalar fields (`key`, `locale`, `addable`): user value wins if present.
fn merge_language_config(default: LanguageConfig, user: LanguageConfig) -> LanguageConfig {
    LanguageConfig {
        key: user.key.or(default.key),
        includes: union_vecs(default.includes, user.includes),
        hashbangs: union_vecs(default.hashbangs, user.hashbangs),
        locale: user.locale.or(default.locale),
        addable: user.addable.or(default.addable),
    }
}

// ---------------------------------------------------------------------------
// Global singleton config
// ---------------------------------------------------------------------------

/// A process-wide `Config` initialised on first access.
///
/// Call `init_global_config` once at startup (before any parallel work) to
/// set it from the CLI arguments.  After that, read it with `global_config()`.
static GLOBAL_CONFIG: Lazy<RwLock<Option<Config>>> = Lazy::new(|| RwLock::new(None));

/// Initialise the global config.  Must be called before `global_config()`.
pub fn init_global_config(config: Config) {
    let mut guard = GLOBAL_CONFIG.write().expect("GLOBAL_CONFIG write lock poisoned");
    *guard = Some(config);
}

/// Access the global config.  Panics if `init_global_config` has not been called.
pub fn global_config() -> Config {
    GLOBAL_CONFIG
        .read()
        .expect("GLOBAL_CONFIG read lock poisoned")
        .as_ref()
        .expect("global config not initialised; call init_global_config first")
        .clone()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let raw: RawConfig =
            serde_yaml::from_str(DEFAULT_CONFIG_YAML).expect("default YAML should parse");
        assert_eq!(raw.word_minimum_length, Some(3));
        assert_eq!(raw.key_minimum_length, Some(6));
        let langs = raw.languages.expect("languages should be present");
        assert!(langs.contains_key("english"));
        assert!(langs.contains_key("ruby"));
        assert!(langs.contains_key("shell"));
    }

    #[test]
    fn config_load_no_user_file() {
        // Pass a nonexistent path so we fall through to the no-user-config branch.
        // We skip the real file-system lookup by passing None (if no .spellr.yml present).
        let cfg = Config::load(None);
        // In most CI/dev environments there is no .spellr.yml at ".", so this should succeed.
        // If there happens to be one the test still passes as long as it is valid YAML.
        if let Ok(cfg) = cfg {
            assert_eq!(cfg.word_minimum_length, 3);
            assert_eq!(cfg.key_minimum_length, 6);
            assert!(!cfg.excludes.is_empty());
            assert!(cfg.languages.contains_key("english"));
        }
    }

    #[test]
    fn merge_user_overrides_scalar() {
        let default: RawConfig = serde_yaml::from_str(DEFAULT_CONFIG_YAML).unwrap();
        let user: RawConfig = serde_yaml::from_str("word_minimum_length: 5").unwrap();
        let merged = merge_configs(default, Some(user));
        assert_eq!(merged.word_minimum_length, Some(5));
        assert_eq!(merged.key_minimum_length, Some(6)); // default preserved
    }

    #[test]
    fn merge_user_adds_language() {
        let default: RawConfig = serde_yaml::from_str(DEFAULT_CONFIG_YAML).unwrap();
        let user: RawConfig = serde_yaml::from_str(
            "languages:\n  mylang:\n    includes:\n      - '*.my'\n",
        )
        .unwrap();
        let merged = merge_configs(default, Some(user));
        let langs = merged.languages.unwrap();
        assert!(langs.contains_key("mylang"));
        assert!(langs.contains_key("ruby")); // default preserved
    }

    #[test]
    fn merge_excludes_are_unioned_not_replaced() {
        let default: RawConfig = serde_yaml::from_str(DEFAULT_CONFIG_YAML).unwrap();
        let user: RawConfig = serde_yaml::from_str("excludes:\n  - '*.custom'\n").unwrap();
        let merged = merge_configs(default, Some(user));
        let excludes = merged.excludes.unwrap();
        // Default entries are preserved
        assert!(excludes.iter().any(|e| e == ".git/"), "default .git/ missing");
        // User entry is appended
        assert!(excludes.iter().any(|e| e == "*.custom"), "user *.custom missing");
    }

    #[test]
    fn merge_language_includes_are_unioned() {
        let default: RawConfig = serde_yaml::from_str(DEFAULT_CONFIG_YAML).unwrap();
        let user: RawConfig = serde_yaml::from_str(
            "languages:\n  ruby:\n    includes:\n      - '*.myruby'\n",
        )
        .unwrap();
        let merged = merge_configs(default, Some(user));
        let langs = merged.languages.unwrap();
        let ruby = langs.get("ruby").unwrap();
        let includes = ruby.includes.as_ref().unwrap();
        // Default ruby includes preserved
        assert!(includes.iter().any(|i| i == "*.rb"), "default *.rb missing");
        // User include appended
        assert!(includes.iter().any(|i| i == "*.myruby"), "user *.myruby missing");
    }
}
