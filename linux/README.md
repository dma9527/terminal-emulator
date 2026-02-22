# Linux GTK4 Terminal

## Prerequisites

```bash
# Fedora
sudo dnf install gtk4-devel harfbuzz-devel

# Ubuntu/Debian
sudo apt install libgtk-4-dev libharfbuzz-dev

# Arch
sudo pacman -S gtk4 harfbuzz
```

## Build

```bash
# Build libterm first
cd ../..
cargo build --release

# Build GTK app
cd linux
cargo build --release
```

## Run

```bash
LD_LIBRARY_PATH=../target/release ./target/release/terminal-gtk
```

## Architecture

Same C ABI bridge as macOS (`libterm.h`):
- `term_session_new/free` — lifecycle
- `term_session_spawn_shell` — PTY
- `term_session_read_pty/write_pty` — I/O
- `term_session_cell_*` — grid access
- `term_session_cursor_*` — cursor state

Rendering uses Cairo (via GTK4 DrawingArea) instead of CoreGraphics.
