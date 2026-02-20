# Terminal Emulator — Launch Roadmap

## Vision

A fast, GPU-accelerated, native terminal emulator that doesn't force users to choose between speed, features, and native experience.

## Target Market

- **Primary**: Power users, developers, DevOps/SRE engineers
- **Secondary**: Data scientists, sysadmins, students learning CLI
- **Platform**: macOS first, then Linux, then Windows

## Competitive Landscape

| Product | Strength | Weakness | Our Opportunity |
|---------|----------|----------|-----------------|
| iTerm2 | Feature-rich, macOS native | Slow, Electron-like feel, aging codebase | Speed + native feel |
| Alacritty | Fast, GPU | Minimal features, no tabs/splits natively | Features + speed |
| Ghostty | Fast, native, feature-rich | New, Zig ecosystem risk | Rust ecosystem stability |
| Kitty | Fast, extensible | Non-native UI, complex config | Native UI + simplicity |
| Warp | AI-integrated, modern UX | Closed source, requires account, privacy concerns | Open source, no account needed |
| WezTerm | Feature-rich, Lua config | Performance inconsistent, UI not native | Native UI + consistent perf |

**Our positioning**: Open source, Rust-based, native UI, zero-config-to-good-defaults, fast.

---

## Phase 0: Foundation (Month 1-2) ✅ CURRENT

### Engineering
- [x] Project structure and build system
- [x] VT parser (table-driven state machine)
- [x] Cell grid with scrollback
- [x] PTY management
- [ ] Basic CSI sequence handling (cursor movement, erase, SGR colors)
- [ ] UTF-8 decode (multi-byte characters, CJK width)
- [ ] Basic CPU text rendering (CoreText on macOS)

### Milestone Gate
`ls`, `vim`, `top`, `htop` render correctly. Chinese/Japanese/Korean text displays properly.

---

## Phase 1: GPU Rendering (Month 3-4)

### Engineering
- [ ] wgpu rendering pipeline (Metal on macOS)
- [ ] Glyph atlas with texture caching
- [ ] harfbuzz font shaping (ligatures, complex scripts)
- [ ] Font fallback chain (system fonts → bundled fallback)
- [ ] Emoji rendering (color emoji via CoreText)
- [ ] Cursor rendering (block, beam, underline + blink)
- [ ] Selection rendering (highlight)
- [ ] Smooth scrolling

### Performance Targets
- Input latency: < 5ms
- Throughput: > 500MB/s (`cat large_file`)
- Frame rate: 120fps during scroll
- Memory: < 50MB base, < 200MB with 100k scrollback

### Milestone Gate
Benchmark competitive with Alacritty/Ghostty. `vtebench` scores published.

---

## Phase 2: macOS Native Shell (Month 5-6)

### Engineering
- [ ] Swift AppKit window management
- [ ] Native tabs (NSTabView)
- [ ] Split panes (horizontal/vertical)
- [ ] Native menu bar with keyboard shortcuts
- [ ] Preferences window (SwiftUI)
- [ ] System dark/light mode support
- [ ] macOS services integration (Quick Look, Spotlight)
- [ ] Secure keyboard input (macOS secure input API)
- [ ] Window state restoration on restart
- [ ] Retina/HiDPI rendering
- [ ] Touch Bar support (if applicable)
- [ ] Notification integration

### Config System
- [ ] TOML config file (`~/.config/term/config.toml`)
- [ ] Sensible defaults (zero-config usable)
- [ ] Config hot-reload
- [ ] Theme system (bundled themes + custom)

### Milestone Gate
Daily-drivable for the development team. All team members switch to it.

---

## Phase 3: Protocol Completion (Month 7-8)

### Terminal Protocol
- [ ] Full SGR (256 color, truecolor, bold, italic, underline styles)
- [ ] Mouse reporting (all modes: X10, normal, SGR, button)
- [ ] Bracketed paste
- [ ] Focus events
- [ ] Alternate screen buffer
- [ ] Scroll regions
- [ ] Tab stops
- [ ] DEC private modes (DECCKM, DECOM, DECAWM, etc.)
- [ ] OSC sequences (title, clipboard, color query/set)
- [ ] Kitty keyboard protocol
- [ ] Kitty graphics protocol (image display)
- [ ] Sixel graphics
- [ ] Synchronized rendering (BSU/ESU)
- [ ] Unicode 15+ (grapheme clusters, zero-width joiners)

### Shell Integration
- [ ] Shell integration scripts (zsh, bash, fish)
- [ ] Current directory tracking
- [ ] Command status tracking
- [ ] Semantic zones (prompt, command, output)

### Compatibility Testing
- [ ] vim/neovim — full feature support
- [ ] tmux — no rendering glitches
- [ ] zellij — splits and tabs work
- [ ] SSH — no escape sequence issues
- [ ] Docker — interactive containers work
- [ ] Common TUI apps: lazygit, btop, ranger, fzf

### Milestone Gate
Pass `vttest` suite. All major TUI apps work without glitches.

---

## Phase 4: Alpha Release (Month 9-10)

### Quality
- [ ] Crash reporting (opt-in, privacy-respecting)
- [ ] Automated regression tests (screenshot comparison)
- [ ] Fuzzing the VT parser (cargo-fuzz)
- [ ] Memory leak testing (Instruments on macOS)
- [ ] Accessibility (VoiceOver support on macOS)

### Distribution
- [ ] Homebrew formula
- [ ] DMG installer with code signing
- [ ] Apple notarization
- [ ] Auto-update mechanism (Sparkle framework)
- [ ] Website (landing page + docs)

### Documentation
- [ ] Getting started guide
- [ ] Configuration reference
- [ ] Keybinding reference
- [ ] Theme creation guide
- [ ] FAQ

### Community
- [ ] GitHub repository (public)
- [ ] Discord server
- [ ] Contributing guide
- [ ] Code of conduct
- [ ] Issue templates

### Alpha Program
- [ ] Invite 50-100 alpha testers (developer communities)
- [ ] Feedback collection system
- [ ] Weekly alpha builds
- [ ] Known issues list

### Milestone Gate
50+ alpha users daily-driving it. Crash rate < 1/week. No data loss bugs.

---

## Phase 5: Beta & Linux (Month 11-14)

### Linux Support
- [ ] GTK4 native shell
- [ ] Wayland support (primary)
- [ ] X11 support (fallback)
- [ ] Adwaita theming
- [ ] Flatpak package
- [ ] DEB/RPM packages
- [ ] AUR package

### Beta Quality
- [ ] Fix all alpha feedback issues
- [ ] Performance optimization pass
- [ ] Memory optimization pass
- [ ] Startup time < 100ms
- [ ] Config migration tool (from iTerm2, Alacritty, Kitty)

### Advanced Features
- [ ] Search in scrollback (regex support)
- [ ] Clickable URLs (auto-detect)
- [ ] Font ligature toggle
- [ ] Per-profile configurations
- [ ] Session save/restore
- [ ] Broadcast input to multiple panes
- [ ] Triggers (pattern → action)

### Milestone Gate
500+ beta users. NPS > 50. No P0 bugs for 2 weeks.

---

## Phase 6: 1.0 Launch (Month 15-16)

### Launch Checklist
- [ ] All P0/P1 bugs resolved
- [ ] Performance benchmarks published
- [ ] Security audit (fuzzing results, dependency audit)
- [ ] Accessibility audit passed
- [ ] Documentation complete
- [ ] Website polished
- [ ] Demo video / GIF on landing page
- [ ] Blog post: "Why we built another terminal emulator"

### Launch Strategy
1. **Soft launch**: Post on personal blog/Twitter, gather initial feedback
2. **Hacker News**: Submit with technical deep-dive blog post
3. **Reddit**: r/programming, r/rust, r/commandline, r/macOS
4. **Product Hunt**: Launch with demo video
5. **YouTube**: Partner with terminal/dev tool YouTubers for reviews
6. **Dev newsletters**: Console.dev, Changelog, This Week in Rust

### Success Metrics (First 30 Days)
- GitHub stars: > 2,000
- Downloads: > 5,000
- Daily active users: > 500
- Discord members: > 300
- Open issues: < 100 (with triage)

---

## Phase 7: Growth & Sustainability (Month 17+)

### Feature Expansion
- [ ] Windows support (DirectX 12 backend via wgpu)
- [ ] Plugin/extension system (Lua or WASM)
- [ ] AI integration (optional, local-first, no account required)
- [ ] Session sharing (pair programming)
- [ ] Built-in multiplexer (replace tmux for basic use cases)
- [ ] Inline image rendering
- [ ] Jupyter-style inline output

### Community Growth
- [ ] Regular release cadence (monthly)
- [ ] Contributor onboarding program
- [ ] Roadmap voting (GitHub Discussions)
- [ ] Annual user survey
- [ ] Conference talks (RustConf, FOSDEM)

### Sustainability Models
| Model | Pros | Cons | Viability |
|-------|------|------|-----------|
| Open source + donations (GitHub Sponsors) | Community goodwill | Unreliable income | Low |
| Open core (free + paid pro features) | Sustainable revenue | Community friction | Medium |
| Sponsorship (companies) | No user friction | Dependent on sponsors | Medium |
| Paid macOS app (free Linux) | Clear value exchange | Limits adoption | Medium-High |
| Support/consulting | Leverages expertise | Doesn't scale | Low |

**Recommended**: Open source core + optional paid features (team sync, cloud config, priority support). Keep the terminal itself fully free and open source.

---

## Risk Register

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Ghostty captures entire market | High | Medium | Differentiate on Rust ecosystem, Windows support, plugin system |
| GPU driver compatibility issues | Medium | High | Fallback CPU renderer, extensive testing matrix |
| macOS API changes break native shell | Medium | Medium | Abstract platform layer, track Apple betas |
| Burnout (solo/small team) | High | High | Scope aggressively, accept contributions early, sustainable pace |
| Security vulnerability in parser | High | Medium | Continuous fuzzing, security audit before 1.0 |
| Rust compile times slow development | Low | High | Incremental compilation, workspace splitting |

---

## Team Requirements

### To 1.0 (Minimum Viable Team)
- 1 core engineer (Rust + systems programming) — full time
- 1 macOS/Swift engineer — part time or contractor
- 1 designer — part time (website, icons, branding)

### To Scale
- +1 Linux/GTK engineer
- +1 Windows engineer
- +1 community manager
- +1 technical writer

---

## Key Differentiators to Communicate

1. **Rust = Safe + Fast** — Memory safety without GC, competitive with C/Zig
2. **Native, not Electron** — Looks and feels like a real macOS/Linux app
3. **Zero config to great defaults** — Works beautifully out of the box
4. **Open source, no account** — Unlike Warp, no login required, no telemetry by default
5. **Plugin system** — Extensible without forking (Phase 7)

---

## Timeline Summary

```
Month  1-2:  Foundation (VT parser, PTY, basic rendering)     ← YOU ARE HERE
Month  3-4:  GPU rendering pipeline
Month  5-6:  macOS native shell, daily-drivable
Month  7-8:  Protocol completion, compatibility
Month  9-10: Alpha release, community building
Month 11-14: Beta, Linux support, advanced features
Month 15-16: 1.0 launch
Month 17+:   Growth, Windows, plugins, sustainability
```

**Total to 1.0**: ~16 months (1 person full-time) or ~8-10 months (3 person team)
