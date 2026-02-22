# Getting Started

## Install from source

```bash
git clone https://github.com/dma9527/terminal-emulator.git
cd terminal-emulator
```

### Build the Rust library

```bash
cargo build --release
```

### Build and run the macOS app

```bash
cd macos/TerminalApp
bash build.sh
DYLD_LIBRARY_PATH=../../target/release ./terminal
```

## Configuration

Config file: `~/.config/term/config.toml`

```toml
scrollback = 10000

[font]
family = "Menlo"
size = 14.0

[window]
width = 800
height = 600
opacity = 1.0
padding = 4

[colors]
foreground = "#cccccc"
background = "#000000"
theme = "default"  # default, dracula, solarized-dark

[shell]
program = "/bin/zsh"
args = ["--login"]
```

All fields are optional — unset values use sensible defaults.

Config changes are auto-detected every 2 seconds (hot-reload).

## Keybindings

| Key | Action |
|-----|--------|
| Cmd+T | New tab |
| Cmd+W | Close window |
| Cmd+Q | Quit |
| Cmd+, | Preferences |
| Cmd++ | Increase font size |
| Cmd+- | Decrease font size |
| Ctrl+C | Send SIGINT |

## Themes

### Bundled themes

- `default` — dark theme with standard ANSI colors
- `dracula` — popular dark theme
- `solarized-dark` — Ethan Schoonover's Solarized

### Custom themes

Create a TOML file:

```toml
name = "my-theme"
foreground = "#e0e0e0"
background = "#1a1a2e"
cursor = "#ffffff"
ansi = [
    "#000000", "#ff0000", "#00ff00", "#ffff00",
    "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
    "#808080", "#ff8080", "#80ff80", "#ffff80",
    "#8080ff", "#ff80ff", "#80ffff", "#ffffff"
]
```

## Running tests

```bash
cargo test
```

## Fuzzing

```bash
cargo install cargo-fuzz
cargo fuzz run fuzz_vt_parser
```
