#![allow(dead_code)]

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "spellr",
    about = "Spell check your source code",
    version
)]
pub struct Cli {
    #[arg(
        short = 'w',
        long,
        help = "Outputs errors in wordlist format",
        conflicts_with_all = &["quiet", "interactive", "autocorrect"]
    )]
    pub wordlist: bool,

    #[arg(
        short = 'q',
        long,
        help = "Silences output",
        conflicts_with_all = &["wordlist", "interactive", "autocorrect"]
    )]
    pub quiet: bool,

    #[arg(
        short = 'i',
        long,
        help = "Runs the spell check interactively",
        conflicts_with_all = &["wordlist", "quiet", "autocorrect"]
    )]
    pub interactive: bool,

    #[arg(
        short = 'a',
        long,
        help = "Autocorrect errors",
        conflicts_with_all = &["wordlist", "quiet", "interactive"]
    )]
    pub autocorrect: bool,

    #[arg(
        long,
        help = "Run in parallel (default: true)",
        default_value_t = true,
        conflicts_with = "no_parallel"
    )]
    pub parallel: bool,

    #[arg(long = "no-parallel", hide = true)]
    pub no_parallel: bool,

    #[arg(
        short = 'd',
        long,
        help = "List files to be checked without actually checking them"
    )]
    pub dry_run: bool,

    #[arg(
        short = 'f',
        long,
        help = "Suppress all configured file rules (include/exclude patterns)"
    )]
    pub suppress_file_rules: bool,

    #[arg(
        short = 'c',
        long,
        value_name = "FILENAME",
        help = "Path to the config file (default: .spellr.yml)"
    )]
    pub config: Option<PathBuf>,

    #[arg(help = "Files or patterns to check (defaults to all tracked files)")]
    pub files: Vec<String>,
}

impl Cli {
    /// Returns true if parallel execution is enabled.
    ///
    /// `--no-parallel` always wins over `--parallel`.
    pub fn parallel_enabled(&self) -> bool {
        self.parallel && !self.no_parallel
    }

    /// Returns true if exactly one reporter mode flag was supplied.
    /// (Clap's `conflicts_with_all` already enforces mutual exclusivity, but
    /// this helper is handy for callers that need to inspect the mode.)
    pub fn has_explicit_reporter(&self) -> bool {
        self.wordlist || self.quiet || self.interactive || self.autocorrect
    }
}
