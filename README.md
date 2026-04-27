# spellr-rs

A Rust reimplementation of [spellr](https://github.com/robotdana/spellr) — a spell checker for source code.

> _"Spell check your source code for fun and occasionally finding bugs"_

## What is spellr?

spellr is a spell checker designed specifically for source code. Unlike a prose spell checker, it understands the conventions of programming:

- It tokenizes **CamelCase**, **snake_case**, **kebab-case**, and **SCREAMING_SNAKE_CASE** and checks each component word independently, including acronym-prefixed forms like `IOString` → `IO` + `String`.
- It skips **URLs**.
- It heuristically skips strings that look like **base64 or hex API keys** rather than real words, using a Naive Bayes classifier. The sensitivity is tunable via `key_heuristic_weight`.
- It ships with **wordlists** for common programming languages and their standard library terms.
- It recognises **hashbangs** (`#!/usr/bin/env ruby`) to identify file types without extensions.
- It respects **.gitignore**.
- It is highly configurable via a `.spellr.yml` file.

## Why Rust?

The original [spellr gem](https://github.com/robotdana/spellr) is written in Ruby. This reimplementation aims to provide the same behaviour with easier distribution as a single executable with no runtime dependency.

## Installation

### From source

```sh
cargo install --path .
```

Or build without installing:

```sh
cargo build --release
# binary is at ./target/release/spellr
```

## Usage

```sh
spellr                        # spell check all files
spellr --interactive          # interactively fix each error
spellr --wordlist             # output unrecognised words in wordlist format
spellr --quiet                # suppress all output (exit code only)
spellr --autocorrect          # automatically apply the top suggestion
spellr --dry-run              # list files that would be checked, without checking them
```

You can check a specific file or set of files/globs:

```sh
spellr path/to/file.rs 'src/**/*.rs'
```

Additional flags:

```sh
spellr --config path/to/.spellr.yml   # use a custom config file
spellr --suppress-file-rules          # ignore configured include/exclude patterns
spellr --no-parallel                  # disable parallel file processing
```

### Interactive mode

In interactive mode you are shown each unrecognised word, its location, and a set of suggestions:

```
src/main.rs:12:4 receive
Did you mean: [1] receive, [2] relieve
[a]dd, [r]eplace, [s]kip, [h]elp, [^C] to exit:
```

| Key | Action |
|-----|--------|
| `1`…`n` | Replace with numbered suggestion |
| `a` | Add the word to a wordlist |
| `r` | Replace this occurrence with a custom correction |
| `R` | Replace this and all future occurrences |
| `s` | Skip this occurrence |
| `S` | Skip this and all future occurrences |
| `h` | Show help |
| `Ctrl-C` | Exit |

### Disabling the tokenizer inline

Add a comment containing `spellr:disable-line` to suppress checking on a single line:

```rust
let weird_ident = do_thing(); // spellr:disable-line
```

Surround a block with `spellr:disable` / `spellr:enable` to suppress an entire region:

```rust
// spellr:disable
let intentional_typo = "teh";
// spellr:enable
```

### First run on a large project

1. Check which files will be scanned:
   ```sh
   spellr --dry-run
   ```
2. Add generated or binary files to `excludes` in `.spellr.yml`.
3. Dump the current unknown words into a wordlist:
   ```sh
   spellr --wordlist > .spellr-wordlists/english.txt
   ```
4. Review the file and delete lines that are genuine typos.
5. Run interactively to fix anything remaining:
   ```sh
   spellr --interactive
   ```

## Configuration

Place a `.spellr.yml` file in your project root. It is merged with the built-in defaults.

```yaml
word_minimum_length: 3        # words shorter than this are ignored
key_minimum_length: 6         # strings shorter than this are never treated as API keys
key_heuristic_weight: 5       # higher → classifier leans more strongly toward word/key

excludes:
  - target/*
  - "*.lock"
  - generated/*

includes:
  - src/**/*
  - "*.md"

languages:
  english:
    locale:
      - US
  ruby:
    includes:
      - "*.rb"
      - Rakefile
    key: r                    # letter used in interactive mode to select this wordlist
    hashbangs:
      - ruby
  myproject:                  # project-specific terms
    includes:
      - src/**/*
```

### Language wordlists

Custom wordlists live in `.spellr-wordlists/` at the project root, one word per line in sorted order. The filename (without extension) must match the language name in `.spellr.yml`. The `--wordlist` flag outputs words in the correct format for pasting directly into these files.

## Compatibility

This implementation targets behavioural compatibility with the Ruby [spellr gem](https://github.com/robotdana/spellr). The tokenizer, skip heuristics, configuration format, wordlist format, and CLI flags all mirror the original.

## License

MIT — same as the original spellr gem.

Bundled wordlists are derived from [SCOWL](http://wordlist.aspell.net/) and [MDN](https://developer.mozilla.org/); see the `wordlists/` directory for their individual licences.

## To Create a release

To cut a release, just push a version tag:

```fish
set repo_version (cargo metadata --format-version=1 --no-deps | jq -r '.packages[0].version')
git tag v$repo_version
git push origin v$repo_version
```
