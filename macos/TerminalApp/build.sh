#!/bin/bash
# Build script for macOS native terminal app.
# 1. Build Rust libterm as cdylib
# 2. Build Swift macOS app linking against it

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "==> Building libterm (Rust)..."
cd "$ROOT_DIR"
cargo build --release

echo "==> Rust library built at: $ROOT_DIR/target/release/liblibterm.dylib"

echo "==> Building macOS app (Swift)..."
cd "$SCRIPT_DIR"

# Compile Swift with bridging header
swiftc \
    -O \
    -import-objc-header Sources/libterm.h \
    -L "$ROOT_DIR/target/release" \
    -llibterm \
    -framework Cocoa \
    -framework CoreText \
    -o terminal \
    Sources/main.swift \
    Sources/AppDelegate.swift \
    Sources/TerminalWindowController.swift \
    Sources/TerminalViewController.swift \
    Sources/TerminalMetalView.swift

echo "==> Built: $SCRIPT_DIR/terminal"
echo "==> Run with: DYLD_LIBRARY_PATH=$ROOT_DIR/target/release ./terminal"
