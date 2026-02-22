/// GTK4 terminal application scaffold.
/// Build on Linux with: cargo build --release
///
/// Requires: gtk4 development libraries
///   Fedora: sudo dnf install gtk4-devel
///   Ubuntu: sudo apt install libgtk-4-dev
///   Arch:   sudo pacman -S gtk4

// This file is a scaffold — full implementation requires a Linux build env.
// The architecture mirrors the macOS app:
//   1. GTK Application + Window
//   2. Custom DrawingArea widget for terminal rendering (Cairo)
//   3. libterm FFI bridge (same C ABI as macOS)
//   4. GLib IO channel for PTY monitoring
//   5. Adwaita theming via libadwaita

fn main() {
    eprintln!("GTK4 terminal app — build on Linux with gtk4 dev libraries installed.");
    eprintln!("See linux/README.md for setup instructions.");
}
