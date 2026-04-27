#![allow(dead_code)]
#![allow(unused_imports)]

mod check;
mod cli;
mod config;
mod file_list;
pub mod key_tuner;
mod language;
mod line_tokenizer;
mod reporter;
mod string_format;
mod suggester;
mod token;
mod token_regexps;
mod tokenizer;
mod wordlist;

use clap::Parser;
use cli::Cli;
use config::Config;
use reporter::ReporterMode;
use std::process;

fn main() {
    let cli = Cli::parse();

    let config_path = cli.config.as_deref();
    let config = match Config::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[1;31m{}\x1b[0m", e);
            process::exit(1);
        }
    };

    // Initialise the global config singleton so other modules can access it.
    config::init_global_config(config.clone());

    // ------------------------------------------------------------------
    // Determine reporter mode (mutually exclusive flags enforced by clap)
    // ------------------------------------------------------------------
    let reporter_mode = if cli.interactive {
        ReporterMode::Interactive
    } else if cli.wordlist {
        ReporterMode::Wordlist
    } else if cli.quiet {
        ReporterMode::Quiet
    } else if cli.autocorrect {
        ReporterMode::Autocorrect
    } else {
        ReporterMode::Default
    };

    // ------------------------------------------------------------------
    // Dry-run: just list the files that would be checked
    // ------------------------------------------------------------------
    if cli.dry_run {
        let file_list = file_list::FileList::new(
            cli.files.clone(),
            config.clone(),
            cli.suppress_file_rules,
        );

        let cwd = std::env::current_dir().unwrap_or_default();
        for path in file_list.iter() {
            let display = path
                .strip_prefix(&cwd)
                .unwrap_or(&path);
            println!("{}", display.display());
        }

        process::exit(0);
    }

    // ------------------------------------------------------------------
    // Parallel is disabled for interactive mode (needs a real TTY/stdin).
    // ------------------------------------------------------------------
    let parallel =
        cli.parallel_enabled() && reporter_mode != ReporterMode::Interactive;

    // ------------------------------------------------------------------
    // Run the spell-check
    // ------------------------------------------------------------------
    let file_list = file_list::FileList::new(
        cli.files.clone(),
        config.clone(),
        cli.suppress_file_rules,
    );

    let exit_code = check::run_check(file_list, &config, reporter_mode, parallel);

    process::exit(exit_code);
}
