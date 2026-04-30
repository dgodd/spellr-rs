#![allow(dead_code)]

use crate::config::Config;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct FileList {
    pub patterns: Vec<String>,
    pub config: Config,
    pub suppress_file_rules: bool,
}

impl FileList {
    pub fn new(patterns: Vec<String>, config: Config, suppress_file_rules: bool) -> Self {
        Self {
            patterns,
            config,
            suppress_file_rules,
        }
    }

    /// Return an iterator over all matching file paths.
    ///
    /// Walk roots are determined from CLI patterns (existing paths are walked
    /// directly; non-existing patterns are treated as glob filters).  If no
    /// patterns are given the current working directory is walked.
    pub fn iter(&self) -> impl Iterator<Item = PathBuf> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (walk_roots, glob_filter) = self.build_roots_and_filter(&cwd);

        let mut builder = WalkBuilder::new(&walk_roots[0]);
        for extra in walk_roots.iter().skip(1) {
            builder.add(extra);
        }

        if self.suppress_file_rules {
            builder.git_ignore(false);
            builder.git_global(false);
            builder.git_exclude(false);
            builder.ignore(false);
        } else if !self.config.excludes.is_empty() {
            // Build a GlobSet from the exclude patterns and use filter_entry so
            // the walker never recurses into excluded directories.
            let mut gs_builder = GlobSetBuilder::new();
            for exclude in &self.config.excludes {
                let pat = exclude.trim_end_matches('/');
                if pat.is_empty() {
                    continue;
                }
                if let Ok(g) = Glob::new(pat) {
                    gs_builder.add(g);
                }
                if exclude.ends_with('/') {
                    if let Ok(g) = Glob::new(&format!("{pat}/**")) {
                        gs_builder.add(g);
                    }
                }
            }
            if let Ok(exclude_set) = gs_builder.build() {
                builder.filter_entry(move |entry| {
                    let rel = entry.path().strip_prefix(&cwd).unwrap_or(entry.path());
                    !exclude_set.is_match(rel)
                });
            }
        }

        builder
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|entry| entry.into_path())
            .filter(move |path| match &glob_filter {
                Some(gs) => gs.is_match(path),
                None => true,
            })
    }

    // ----------------------------------------------------------------
    // Helpers
    // ----------------------------------------------------------------

    /// Split CLI patterns into concrete walk roots (existing paths) and an
    /// optional `GlobSet` for filtering the walk output.
    fn build_roots_and_filter(&self, cwd: &Path) -> (Vec<PathBuf>, Option<GlobSet>) {
        if self.patterns.is_empty() {
            return (vec![cwd.to_path_buf()], None);
        }

        let mut roots: Vec<PathBuf> = Vec::new();
        let mut glob_builder = GlobSetBuilder::new();
        let mut has_globs = false;

        for pattern in &self.patterns {
            let path = Path::new(pattern);
            // Resolve the path relative to cwd if it is not absolute.
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            };

            if resolved.exists() {
                // Concrete existing path – use as a walk root directly.
                roots.push(resolved);
            } else {
                // Non-existent path – treat as a glob expression.
                match Glob::new(pattern) {
                    Ok(glob) => {
                        glob_builder.add(glob);
                        has_globs = true;
                    }
                    Err(_) => {
                        // Silently skip malformed patterns.
                    }
                }
            }
        }

        let filter = if has_globs {
            glob_builder.build().ok()
        } else {
            None
        };

        if roots.is_empty() {
            // All patterns were glob expressions – walk from cwd and filter.
            roots.push(cwd.to_path_buf());
        }

        (roots, filter)
    }
}
