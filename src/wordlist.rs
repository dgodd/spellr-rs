#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::token::normalize_str;

/// A sorted list of words stored on disk and loaded lazily.
///
/// Words are kept in sorted order so binary search can be used for fast
/// membership testing.  This mirrors the Ruby `Spellr::Wordlist` class.
pub struct Wordlist {
    pub path: PathBuf,
    pub name: String,
    /// Lazily-loaded sorted list of words.
    words: Option<Vec<String>>,
    /// Cache: normalised term → whether it is in the list.
    cache: HashMap<String, bool>,
    /// Optional embedded wordlist content baked in at compile time.
    embedded: Option<&'static str>,
    /// Globally-cached `Arc<HashSet>` for fast O(1) bulk membership checks.
    static_set: Option<Arc<HashSet<String>>>,
}

impl Wordlist {
    /// Create a new `Wordlist` handle backed only by a disk file.
    ///
    /// No I/O is performed here; words are loaded on first access.
    pub fn new(path: PathBuf, name: String) -> Self {
        Self {
            path,
            name,
            words: None,
            cache: HashMap::new(),
            embedded: None,
            static_set: None,
        }
    }

    /// Create a new `Wordlist` handle that starts from embedded content and
    /// optionally merges any additional words found on disk.
    pub fn with_embedded(path: PathBuf, name: String, embedded: &'static str) -> Self {
        Self {
            path,
            name,
            words: None,
            cache: HashMap::new(),
            embedded: Some(embedded),
            static_set: None,
        }
    }

    /// Create a `Wordlist` backed by an embedded string **and** a pre-built
    /// globally-cached `Arc<HashSet<String>>`.
    ///
    /// `as_arc_hashset` will return the cached `Arc` directly (no allocation)
    /// when no on-disk override file exists, making per-file word-set
    /// construction essentially free for the common case.
    pub fn with_static_set(
        path: PathBuf,
        name: String,
        embedded: &'static str,
        static_set: Arc<HashSet<String>>,
    ) -> Self {
        Self {
            path,
            name,
            words: None,
            cache: HashMap::new(),
            embedded: Some(embedded),
            static_set: Some(static_set),
        }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Return `true` if the (normalised) `term` is present in the wordlist.
    ///
    /// Results are memoised in `self.cache`.
    pub fn contains(&mut self, term: &str) -> bool {
        let term = normalize_str(term);
        if let Some(&cached) = self.cache.get(&term) {
            return cached;
        }
        let found = self.load_words().binary_search(&term).is_ok();
        self.cache.insert(term, found);
        found
    }

    /// Add a normalised `term` to the wordlist, inserting it in sorted order
    /// and persisting the updated list to disk.
    pub fn push(&mut self, term: &str) -> std::io::Result<()> {
        let term = normalize_str(term);
        self.touch()?;
        self.cache.insert(term.clone(), true);
        {
            let words = self.load_words_mut();
            match words.binary_search(&term) {
                Ok(_) => {} // already present – nothing to do
                Err(pos) => words.insert(pos, term),
            }
        }
        let snapshot = self.words.as_ref().unwrap().clone();
        self.write(&snapshot)
    }

    /// Return `true` if embedded content is present **or** the backing file
    /// exists on disk.
    pub fn exists(&self) -> bool {
        self.embedded.is_some() || self.static_set.is_some() || self.path.exists()
    }

    /// Return a cheaply-cloned `Arc` to this wordlist's word set.
    ///
    /// * If the wordlist has a globally-cached set **and** no on-disk override
    ///   file exists, the shared `Arc` is returned directly (zero allocation).
    /// * If a disk file also exists, the cached set is cloned and the disk
    ///   words are merged in before wrapping in a new `Arc`.
    /// * Otherwise the words are loaded normally and wrapped in a fresh `Arc`.
    pub fn as_arc_hashset(&mut self) -> Arc<HashSet<String>> {
        if let Some(ref set) = self.static_set {
            if !self.path.exists() {
                // Pure embedded, no disk override — hand out the global copy.
                return Arc::clone(set);
            }
            // Disk override exists: merge embedded set with on-disk words.
            let mut merged: HashSet<String> = (**set).clone();
            for w in Self::read_words_from_disk(&self.path) {
                merged.insert(w);
            }
            return Arc::new(merged);
        }
        // Pure disk-backed wordlist.
        Arc::new(self.load_words().iter().cloned().collect())
    }

    /// Return a slice of all words (loading from disk on first call).
    pub fn words(&mut self) -> &[String] {
        self.load_words()
    }

    /// Return the number of words in the wordlist.
    pub fn len(&mut self) -> usize {
        self.load_words().len()
    }

    /// Return `true` if the wordlist is empty.
    pub fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    /// Discard the in-memory cache (forces re-evaluation on next `contains`).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Parse a string of newline-separated words into a sorted, deduplicated
    /// `Vec<String>`.
    fn parse_content(content: &str) -> Vec<String> {
        let mut words: Vec<String> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect();
        words.sort();
        words.dedup();
        words
    }

    /// Return an immutable reference to the loaded words, loading from disk
    /// (and/or embedded content) if they have not been loaded yet.
    fn load_words(&mut self) -> &[String] {
        if self.words.is_none() {
            // Start with embedded content (if any).
            let mut words = match self.embedded {
                Some(content) => Self::parse_content(content),
                None => Vec::new(),
            };
            // Merge in any additional words from disk.
            for w in Self::read_words_from_disk(&self.path) {
                match words.binary_search(&w) {
                    Ok(_) => {}          // already present
                    Err(pos) => words.insert(pos, w),
                }
            }
            self.words = Some(words);
        }
        self.words.as_deref().unwrap()
    }

    /// Return a mutable reference to the loaded words.
    fn load_words_mut(&mut self) -> &mut Vec<String> {
        if self.words.is_none() {
            // Start with embedded content (if any).
            let mut words = match self.embedded {
                Some(content) => Self::parse_content(content),
                None => Vec::new(),
            };
            // Merge in any additional words from disk.
            for w in Self::read_words_from_disk(&self.path) {
                match words.binary_search(&w) {
                    Ok(_) => {}
                    Err(pos) => words.insert(pos, w),
                }
            }
            self.words = Some(words);
        }
        self.words.as_mut().unwrap()
    }

    /// Read lines from the backing file.  Returns an empty `Vec` if the file
    /// does not exist or cannot be read.
    fn read_words_from_disk(path: &std::path::Path) -> Vec<String> {
        if !path.exists() {
            return Vec::new();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Write the given word list to disk as a newline-separated file.
    fn write(&self, words: &[String]) -> std::io::Result<()> {
        let content = if words.is_empty() {
            String::new()
        } else {
            format!("{}\n", words.join("\n"))
        };
        std::fs::write(&self.path, content)
    }

    /// Create the backing file (and any missing parent directories) if it does
    /// not yet exist.
    fn touch(&mut self) -> std::io::Result<()> {
        if !self.path.exists() {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            self.write(&[])?;
            // Ensure the in-memory list is initialised to an empty vec so that
            // subsequent `push` calls don't try to read a file that was just
            // created (and is still empty).
            if self.words.is_none() {
                self.words = Some(Vec::new());
            }
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_wordlist() -> (tempfile::TempDir, Wordlist) {
        let dir = tempfile::tempdir().expect("tmp dir");
        let path = dir.path().join("test.txt");
        let wl = Wordlist::new(path, "test".into());
        (dir, wl)
    }

    #[test]
    fn empty_wordlist_does_not_contain_words() {
        let (_dir, mut wl) = tmp_wordlist();
        assert!(!wl.contains("hello"));
    }

    #[test]
    fn push_and_contains() {
        let (_dir, mut wl) = tmp_wordlist();
        wl.push("hello").unwrap();
        assert!(wl.contains("hello"));
        assert!(!wl.contains("world"));
    }

    #[test]
    fn push_normalises_term() {
        let (_dir, mut wl) = tmp_wordlist();
        wl.push("Hello").unwrap(); // will be stored as "hello"
        assert!(wl.contains("hello"));
        assert!(wl.contains("Hello")); // look-up also normalises
    }

    #[test]
    fn words_are_sorted_after_multiple_pushes() {
        let (_dir, mut wl) = tmp_wordlist();
        wl.push("zebra").unwrap();
        wl.push("apple").unwrap();
        wl.push("mango").unwrap();
        let words = wl.words().to_vec();
        let mut sorted = words.clone();
        sorted.sort();
        assert_eq!(words, sorted);
    }

    #[test]
    fn push_duplicate_does_not_duplicate_word() {
        let (_dir, mut wl) = tmp_wordlist();
        wl.push("hello").unwrap();
        wl.push("hello").unwrap();
        assert_eq!(wl.len(), 1);
    }

    #[test]
    fn exists_returns_false_before_push() {
        let (_dir, wl) = tmp_wordlist();
        assert!(!wl.exists());
    }

    #[test]
    fn exists_returns_true_after_push() {
        let (_dir, mut wl) = tmp_wordlist();
        wl.push("word").unwrap();
        assert!(wl.exists());
    }

    #[test]
    fn reload_from_disk() {
        let (_dir, mut wl) = tmp_wordlist();
        wl.push("alpha").unwrap();
        wl.push("beta").unwrap();

        // Create a fresh handle to the same path (simulates a new process).
        let mut wl2 = Wordlist::new(wl.path.clone(), "test".into());
        assert!(wl2.contains("alpha"));
        assert!(wl2.contains("beta"));
        assert_eq!(wl2.len(), 2);
    }

    #[test]
    fn embedded_wordlist_exists_without_disk_file() {
        let dir = tempfile::tempdir().expect("tmp dir");
        let path = dir.path().join("nonexistent.txt");
        let wl = Wordlist::with_embedded(path, "test".into(), "apple\nbanana\ncherry\n");
        assert!(wl.exists());
    }

    #[test]
    fn embedded_wordlist_contains_words() {
        let dir = tempfile::tempdir().expect("tmp dir");
        let path = dir.path().join("nonexistent.txt");
        let mut wl = Wordlist::with_embedded(path, "test".into(), "apple\nbanana\ncherry\n");
        assert!(wl.contains("apple"));
        assert!(wl.contains("banana"));
        assert!(wl.contains("cherry"));
        assert!(!wl.contains("durian"));
    }

    #[test]
    fn embedded_words_are_sorted() {
        let dir = tempfile::tempdir().expect("tmp dir");
        let path = dir.path().join("nonexistent.txt");
        let mut wl = Wordlist::with_embedded(path, "test".into(), "zebra\napple\nmango\n");
        let words = wl.words().to_vec();
        let mut sorted = words.clone();
        sorted.sort();
        assert_eq!(words, sorted);
    }

    #[test]
    fn disk_words_merged_with_embedded() {
        let dir = tempfile::tempdir().expect("tmp dir");
        let path = dir.path().join("extra.txt");
        // Write some extra words to disk.
        std::fs::write(&path, "durian\nelderberry\n").unwrap();
        let mut wl = Wordlist::with_embedded(path, "test".into(), "apple\nbanana\ncherry\n");
        assert!(wl.contains("apple"));
        assert!(wl.contains("durian"));
        assert!(wl.contains("elderberry"));
        // Result should still be sorted.
        let words = wl.words().to_vec();
        let mut sorted = words.clone();
        sorted.sort();
        assert_eq!(words, sorted);
    }
}
