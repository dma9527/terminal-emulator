# Terminal Emulator

A fast, GPU-accelerated, cross-platform terminal emulator built with Rust.

## Architecture

```
┌─────────────────────────────────────────────┐
│           Platform Shell (per OS)            │
│  macOS: Swift/AppKit   Linux: GTK4           │
│  (窗口、菜单、标签页、系统集成)                  │
├─────────────────────────────────────────────┤
│              Core Library (Rust)             │
│  C ABI export → libterm.dylib/.so            │
├──────────┬──────────┬───────────────────────┤
│ Terminal │ Renderer │     PTY Manager       │
│ Emulator │ (wgpu)   │  (async I/O)         │
├──────────┴──────────┴───────────────────────┤
│  VT Parser  │  Font Shaper  │  Input Handler│
│ (表驱动)     │ (harfbuzz)    │  (键盘/IME)   │
└─────────────┴──────────────┴────────────────┘
```

## Design Decisions

### Language: Rust

| Considered | Pros | Cons | Verdict |
|-----------|------|------|---------|
| Zig | Extreme perf, C ABI native, comptime | Unstable language, small ecosystem | Too risky for new project |
| Rust | Memory safe, rich ecosystem (wgpu, harfbuzz-rs), large community | Slower compile, C/Swift interop friction | **Selected** |
| C | Max control, zero interop friction | Manual memory mgmt, security risk | Too dangerous |
| Swift | Best macOS native feel | Poor cross-platform, weaker systems perf | Single-platform only |

### Rendering: CPU Shaping + wgpu

- **Font shaping**: harfbuzz (CPU) — ensures correct text rendering for all scripts
- **Rasterization**: GPU via wgpu — auto-selects Metal (macOS) / Vulkan (Linux) / OpenGL (fallback)
- **Glyph atlas**: Cache rasterized glyphs in GPU texture, batch draw per frame
- **Dirty tracking**: Only re-render changed cells, not full screen

### UI: Hybrid Native

- Window management, menus, dialogs → platform native (Swift on macOS, GTK4 on Linux)
- Terminal rendering area → self-drawn via wgpu
- Core exposes C ABI (`libterm`) for platform shells to consume

### Terminal Parser: Table-Driven State Machine

- Based on Paul Williams' [VT parser state diagram](https://vt100.net/emu/dec_ansi_parser)
- Lookup table for state transitions — O(1) per byte
- Same approach used by Alacritty and Ghostty

### Memory: Ring Buffer Scrollback

- Fixed-size ring buffer for scrollback history
- Each cell: 8-16 bytes (character + attributes + color)
- Configurable max scrollback (default: 10,000 lines)

## Module Breakdown

### `src/core/` — Terminal Emulator Core

| Component | Difficulty | Description |
|-----------|-----------|-------------|
| VT Parser | ⭐⭐⭐⭐⭐ | Table-driven state machine, handles hundreds of escape sequences |
| Grid/Screen | ⭐⭐⭐ | Cell grid with scrollback ring buffer |
| Input Handler | ⭐⭐⭐ | Keyboard mapping, modifier keys, IME |
| Selection | ⭐⭐⭐ | Text selection, clipboard, word/line select modes |

### `src/renderer/` — GPU Rendering Pipeline

| Component | Difficulty | Description |
|-----------|-----------|-------------|
| Glyph Atlas | ⭐⭐⭐⭐ | Font rasterization cache in GPU texture |
| Text Pipeline | ⭐⭐⭐⭐ | wgpu shaders for batched cell rendering |
| Cursor | ⭐⭐ | Block/beam/underline cursor with blink |

### `src/pty/` — Pseudo-Terminal Management

| Component | Difficulty | Description |
|-----------|-----------|-------------|
| PTY spawn | ⭐⭐ | `forkpty()`/`openpty()`, shell process management |
| Async I/O | ⭐⭐ | Event loop with `mio` or `polling` (not tokio) |

### `src/platform/` — Native Platform Shells

| Component | Difficulty | Description |
|-----------|-----------|-------------|
| macOS (Swift) | ⭐⭐⭐ | AppKit/SwiftUI window, tabs, splits, system integration |
| Linux (GTK4) | ⭐⭐⭐ | GTK4 window, Adwaita styling |
| C ABI bridge | ⭐⭐ | `#[no_mangle] extern "C"` exports from Rust |

## Development Roadmap

| Phase | Duration | Goal | Key Deliverables |
|-------|----------|------|-----------------|
| 1 | 1-2 months | Foundation | PTY management, basic VT parser, CPU text rendering. `ls`, `vim`, `top` work. |
| 2 | 1-2 months | GPU Rendering | wgpu pipeline, glyph atlas, harfbuzz shaping. Smooth scrolling. |
| 3 | 1 month | macOS Native Shell | Swift AppKit window, tabs, splits, Cmd shortcuts. |
| 4 | 1-2 months | Protocol Completion | 256/truecolor, mouse events, selection/clipboard, OSC sequences. |
| 5 | Ongoing | Polish & Expand | Performance optimization, Linux GTK4 shell, Kitty graphics protocol. |

**Estimated timeline**: ~5-8 months for a high-quality single-platform (macOS) release.

## Key Dependencies

```toml
# Core
wgpu = "24"           # GPU rendering (Metal/Vulkan/OpenGL)
harfbuzz_rs = "0.4"   # Font shaping
fontdue = "0.9"       # Font rasterization
mio = "1"             # Lightweight async I/O
nix = "0.29"          # Unix/PTY APIs

# Platform
objc2 = "0.6"         # macOS Objective-C bridge (for Swift interop)
```

## Reference Projects

- [Ghostty](https://github.com/ghostty-org/ghostty) — Best architecture (libghostty pattern)
- [Alacritty](https://github.com/alacritty/alacritty) — Smallest codebase, good learning reference
- [wezterm](https://github.com/wez/wezterm) — Most feature-rich Rust terminal

## Getting Started

```bash
# Prerequisites
rustup install stable
# macOS: Xcode Command Line Tools
# Linux: gtk4-devel, harfbuzz-devel

# Build
cargo build

# Run
cargo run
```
