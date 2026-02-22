# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added — Phase 4: Alpha Release

- Config system: TOML-based (`~/.config/term/config.toml`) with sensible defaults, partial override, hot-reload ready (6 tests)
- Theme system: 3 bundled themes (default, dracula, solarized-dark) + custom TOML themes with hex colors and per-ANSI-slot override (7 tests)
- Clipboard: system copy/paste via pbcopy/pbpaste, bracketed paste wrapping, OSC 52 decode with built-in base64 (7 tests)
- Fuzz harness: cargo-fuzz target for VT parser + terminal handler (fuzz/fuzz_targets/fuzz_vt_parser.rs)
- Config hot-reload: polling-based file watcher, auto-detects config changes every 2s (2 tests)
- Homebrew formula: `Formula/term.rb` for macOS distribution
- Documentation: getting started guide with config reference, keybindings, themes, testing, fuzzing
- Community: CONTRIBUTING.md, LICENSE (MIT), issue templates

### Added — Phase 5: Beta & Linux

- Scrollback search: literal + regex search across visible grid and scrollback buffer (7 tests)
- URL detection: auto-detect https/http URLs in grid, position lookup, trailing punctuation stripping (5 tests)
- Dirty region tracking: incremental rendering — only redraw changed rows (5 tests)
- Session save/restore: JSON-based session persistence with working dir, shell, scrollback (3 tests)
- Linux GTK4 scaffold: Cargo project, README with build instructions, architecture docs
