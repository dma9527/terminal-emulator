#![no_main]
use libfuzzer_sys::fuzz_target;
use libterm::core::{Terminal, VtParser};

fuzz_target!(|data: &[u8]| {
    let mut terminal = Terminal::new(80, 24);
    let mut parser = VtParser::new();
    terminal.feed_bytes(&mut parser, data);
});
