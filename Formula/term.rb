class Term < Formula
  desc "Fast, GPU-accelerated terminal emulator built with Rust"
  homepage "https://github.com/dma9527/terminal-emulator"
  url "https://github.com/dma9527/terminal-emulator/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "PLACEHOLDER"
  license "MIT"

  depends_on "rust" => :build
  depends_on "harfbuzz"

  def install
    system "cargo", "build", "--release"
    lib.install "target/release/liblibterm.dylib"
    include.install "macos/TerminalApp/Sources/libterm.h"
  end

  test do
    system "cargo", "test"
  end
end
