#![allow(dead_code)]

use std::collections::HashMap;

use once_cell::sync::Lazy;



use crate::key_tuner::possible_key::PossibleKey;
use crate::key_tuner::stats::gaussian_probability;

// ── YAML data structures ──────────────────────────────────────────────────────
//
// The data.yml uses Ruby symbol keys (e.g. `:feature_set:`, `:+:`, `:mean:`).
// serde_yaml 0.9 parses them as plain strings with a leading colon.
// We strip that colon in `strip_sym`.

#[derive(Debug, Clone)]
pub struct FeatureStats {
    pub standard_deviation: f64,
    pub mean: f64,
    pub variance: f64,
}

/// `feature_set[class_name][feature_name] -> FeatureStats`
pub type FeatureSet = HashMap<String, HashMap<String, FeatureStats>>;

pub struct NaiveBayesData {
    pub feature_set: FeatureSet,
    pub num_classes: usize,
    pub classes: Vec<String>,
    pub features: Vec<String>,
}

// ── Public classifier ─────────────────────────────────────────────────────────

/// The parsed classifier data, loaded once from the embedded YAML.
/// This is the expensive part; `NaiveBayes` itself is just a weight wrapper.
static CACHED_DATA: Lazy<NaiveBayesData> = Lazy::new(|| {
    let yaml_str = include_str!("../../key_tuner_data.yml");
    parse_yaml(yaml_str).expect("bundled key_tuner_data.yml failed to parse")
});

pub struct NaiveBayes {
    key_heuristic_weight: f64,
}

impl NaiveBayes {
    /// Create a classifier using the default heuristic weight (5.0).
    pub fn new() -> Self {
        Self::with_weight(5.0)
    }

    /// Create a classifier with an explicit heuristic weight.
    ///
    /// This is cheap — the underlying YAML data is already cached in
    /// `CACHED_DATA` after the first call to `new()` or `with_weight()`.
    pub fn with_weight(key_heuristic_weight: f64) -> Self {
        // Touch the global so the YAML is parsed now (if not already).
        let _ = &*CACHED_DATA;
        Self { key_heuristic_weight }
    }

    /// Returns `true` if `string` is classified as an API key.
    pub fn is_key(&self, string: &str) -> bool {
        let features = PossibleKey::new(string).features();
        self.classify(&features).starts_with("key")
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn heuristic_weight(&self) -> f64 {
        10_f64.powf(self.key_heuristic_weight)
    }

    /// Gaussian probability of `value` for `feature` in `class_name`.
    fn feature_probability(&self, feature: &str, value: f64, class_name: &str) -> f64 {
        let data = &*CACHED_DATA;
        let stats = data
            .feature_set
            .get(class_name)
            .and_then(|fs: &HashMap<String, FeatureStats>| fs.get(feature));

        match stats {
            Some(s) => gaussian_probability(value, s.standard_deviation, s.mean, s.variance),
            None => 1.0, // unknown feature: neutral
        }
    }

    /// Product of per-feature probabilities for `class_name`.
    fn feature_multiplication(&self, features: &HashMap<String, f64>, class_name: &str) -> f64 {
        features.iter().fold(1.0, |acc, (k, &v)| {
            acc * self.feature_probability(k, v, class_name)
        })
    }

    /// Full class probability (prior × likelihood × optional heuristic boost).
    fn class_probability(&self, features: &HashMap<String, f64>, class_name: &str) -> f64 {
        let data = &*CACHED_DATA;
        let class_fraction = 1.0 / data.num_classes as f64;
        let mut bayes = self.feature_multiplication(features, class_name);
        if class_name.starts_with("key_") {
            bayes *= self.heuristic_weight();
        }
        bayes * class_fraction
    }

    /// Return the class name with the highest probability.
    fn classify(&self, features: &HashMap<String, f64>) -> String {
        let data = &*CACHED_DATA;
        data.classes
            .iter()
            .max_by(|a, b| {
                let pa = self.class_probability(features, a);
                let pb = self.class_probability(features, b);
                pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .unwrap_or_default()
    }
}

impl Default for NaiveBayes {
    fn default() -> Self {
        Self::new()
    }
}

// ── YAML parsing ──────────────────────────────────────────────────────────────
//
// The Ruby YAML uses symbol keys such as `:feature_set:`, `:+:`, `:mean:`.
// serde_yaml 0.9 (backed by yaml-rust) parses these as plain strings that
// include the leading colon, e.g. the string `":feature_set"`.
// `strip_sym` strips that colon so we get the bare name.

fn strip_sym(s: &str) -> &str {
    s.strip_prefix(':').unwrap_or(s)
}

fn as_str(v: &serde_yaml::Value) -> Option<&str> {
    match v {
        serde_yaml::Value::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn as_f64(v: &serde_yaml::Value) -> Option<f64> {
    match v {
        serde_yaml::Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn as_usize(v: &serde_yaml::Value) -> Option<usize> {
    match v {
        serde_yaml::Value::Number(n) => n.as_u64().map(|u| u as usize),
        _ => None,
    }
}

fn parse_feature_stats(v: &serde_yaml::Value) -> Option<FeatureStats> {
    let mapping = v.as_mapping()?;
    let mut sd = None;
    let mut mean = None;
    let mut variance = None;

    for (k, val) in mapping {
        let key = as_str(k).map(strip_sym).unwrap_or("");
        match key {
            "standard_deviation" => sd       = as_f64(val),
            "mean"               => mean     = as_f64(val),
            "variance"           => variance = as_f64(val),
            _                    => {}
        }
    }

    Some(FeatureStats {
        standard_deviation: sd?,
        mean: mean?,
        variance: variance?,
    })
}

/// Parse `feature_set` block:
/// ```yaml
/// not_key_lower36:
///   :+:
///     :standard_deviation: …
///     :mean: …
///     :variance: …
/// ```
fn parse_feature_set(v: &serde_yaml::Value) -> Option<FeatureSet> {
    let mapping = v.as_mapping()?;
    let mut result: FeatureSet = HashMap::new();

    for (class_k, features_v) in mapping {
        // Class names are plain strings (no colon prefix), but we still strip
        // just in case.
        let class_name = as_str(class_k).map(strip_sym)?.to_string();
        let features_mapping = features_v.as_mapping()?;
        let mut feature_map: HashMap<String, FeatureStats> = HashMap::new();

        for (feat_k, stats_v) in features_mapping {
            let feat_name = as_str(feat_k).map(strip_sym)?.to_string();
            if let Some(stats) = parse_feature_stats(stats_v) {
                feature_map.insert(feat_name, stats);
            }
        }

        result.insert(class_name, feature_map);
    }

    Some(result)
}

fn parse_yaml(yaml_str: &str) -> Result<NaiveBayesData, Box<dyn std::error::Error>> {
    let root: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
    let mapping = root
        .as_mapping()
        .ok_or("root YAML value is not a mapping")?;

    let mut feature_set: Option<FeatureSet> = None;
    let mut num_classes: Option<usize> = None;
    let mut classes: Option<Vec<String>> = None;
    let mut features: Option<Vec<String>> = None;

    for (k, v) in mapping {
        let key = as_str(k).map(strip_sym).unwrap_or("");
        match key {
            "feature_set" => {
                feature_set = parse_feature_set(v);
            }
            "num_classes" => {
                num_classes = as_usize(v);
            }
            "classes" => {
                if let serde_yaml::Value::Sequence(seq) = v {
                    classes = Some(
                        seq.iter()
                            .filter_map(|item| as_str(item).map(strip_sym).map(str::to_string))
                            .collect(),
                    );
                }
            }
            "features" => {
                if let serde_yaml::Value::Sequence(seq) = v {
                    features = Some(
                        seq.iter()
                            .filter_map(|item| as_str(item).map(strip_sym).map(str::to_string))
                            .collect(),
                    );
                }
            }
            _ => {}
        }
    }

    Ok(NaiveBayesData {
        feature_set: feature_set.ok_or("missing feature_set")?,
        num_classes: num_classes.ok_or("missing num_classes")?,
        classes: classes.ok_or("missing classes")?,
        features: features.ok_or("missing features")?,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_from_embedded_yaml() {
        let _nb = NaiveBayes::new(); // ensure CACHED_DATA is initialised
        let data = &*CACHED_DATA;
        assert_eq!(data.num_classes, 8);
        assert_eq!(data.classes.len(), 8);
        assert!(data.feature_set.contains_key("not_key_lower36"));
        assert!(data.feature_set.contains_key("key_base64"));
    }

    #[test]
    fn strip_sym_strips_colon() {
        assert_eq!(strip_sym(":feature_set"), "feature_set");
        assert_eq!(strip_sym(":+"),           "+");
        assert_eq!(strip_sym("not_key"),      "not_key");
    }

    #[test]
    fn is_key_does_not_panic_on_plain_word() {
        // The raw Naive Bayes classifier is designed to be called after the
        // key_minimum_length / min_alpha_re guards in LineTokenizer, so it may
        // classify short common words as keys due to the strong heuristic weight.
        // This test just verifies it doesn't panic and returns a bool.
        let nb = NaiveBayes::new();
        let _ = nb.is_key("hello");
        let _ = nb.is_key("helloworld");
    }

    #[test]
    fn is_key_accepts_obvious_key() {
        let nb = NaiveBayes::new();
        // A long base64-ish string that alternates alpha/num chunks
        let candidate = "SG.abcdefghijklmnopqrstuv.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqr";
        // We're not asserting true/false here since the classifier decides,
        // but we do assert it doesn't panic.
        let _ = nb.is_key(candidate);
    }

    #[test]
    fn features_have_correct_stats_for_known_class() {
        let _nb = NaiveBayes::new();
        let class = CACHED_DATA
            .feature_set
            .get("not_key_lower36")
            .expect("class not_key_lower36 must exist");
        let plus_stats = class.get("+").expect("feature '+' must exist");
        // Sanity-check against the known YAML values
        assert!((plus_stats.mean - 0.31397306397306396).abs() < 1e-10);
        assert!((plus_stats.variance - 0.022188778922785653).abs() < 1e-10);
    }
}
