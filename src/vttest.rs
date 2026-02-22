/// VT compatibility test suite — automated vttest-style checks.
/// Tests terminal response to standard VT sequences without interactive UI.

#[cfg(test)]
mod tests {
    use crate::core::{Terminal, VtParser, CellAttr, Color};

    fn run(input: &[u8]) -> Terminal {
        let mut t = Terminal::new(80, 24);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, input);
        t
    }

    // === Cursor Movement ===

    #[test]
    fn vt_cup_home() {
        let t = run(b"\x1b[H");
        assert_eq!((t.grid.cursor_row, t.grid.cursor_col), (0, 0));
    }

    #[test]
    fn vt_cup_absolute() {
        let t = run(b"\x1b[10;20H");
        assert_eq!((t.grid.cursor_row, t.grid.cursor_col), (9, 19));
    }

    #[test]
    fn vt_cup_clamp() {
        let t = run(b"\x1b[999;999H");
        assert_eq!(t.grid.cursor_row, 23);
        assert_eq!(t.grid.cursor_col, 79);
    }

    #[test]
    fn vt_cuu_cud_cuf_cub() {
        let t = run(b"\x1b[12;40H\x1b[5A\x1b[3B\x1b[10C\x1b[2D");
        // Start (11,39), up 5 → (6,39), down 3 → (9,39), right 10 → (9,49), left 2 → (9,47)
        assert_eq!((t.grid.cursor_row, t.grid.cursor_col), (9, 47));
    }

    #[test]
    fn vt_cursor_save_restore() {
        let t = run(b"\x1b[5;10H\x1b7\x1b[1;1H\x1b8");
        assert_eq!((t.grid.cursor_row, t.grid.cursor_col), (4, 9));
    }

    // === Erase ===

    #[test]
    fn vt_ed_below() {
        let mut t = run(b"");
        let mut p = VtParser::new();
        // Fill screen
        for _ in 0..24 { t.feed_bytes(&mut p, b"XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"); }
        t.feed_bytes(&mut p, b"\x1b[12;1H\x1b[J");
        // Row 11 (0-indexed) should be cleared
        assert_eq!(t.grid.cell(11, 0).ch, ' ');
        // Row 10 should still have content
        assert_eq!(t.grid.cell(10, 0).ch, 'X');
    }

    #[test]
    fn vt_el_right() {
        let t = run(b"ABCDEFGHIJ\x1b[1;5H\x1b[K");
        assert_eq!(t.grid.cell(0, 3).ch, 'D');
        assert_eq!(t.grid.cell(0, 4).ch, ' ');
        assert_eq!(t.grid.cell(0, 9).ch, ' ');
    }

    #[test]
    fn vt_ech() {
        let t = run(b"ABCDEFGHIJ\x1b[1;3H\x1b[4X");
        assert_eq!(t.grid.cell(0, 1).ch, 'B');
        assert_eq!(t.grid.cell(0, 2).ch, ' ');
        assert_eq!(t.grid.cell(0, 5).ch, ' ');
        assert_eq!(t.grid.cell(0, 6).ch, 'G');
    }

    // === Scroll ===

    #[test]
    fn vt_scroll_region() {
        let t = run(b"\x1b[5;10r\x1b[5;1H\x1b[3S");
        // Scroll region set, scrolled up 3 lines within region
        assert_eq!(t.grid.cursor_row, 4);
    }

    #[test]
    fn vt_reverse_index_at_top() {
        let t = run(b"\x1b[1;1H\x1bM");
        // At top of screen, reverse index should scroll down
        assert_eq!(t.grid.cursor_row, 0);
    }

    // === SGR ===

    #[test]
    fn vt_sgr_bold_italic() {
        let t = run(b"\x1b[1;3mA\x1b[0mB");
        let a = t.grid.cell(0, 0);
        assert!(a.attr.contains(CellAttr::BOLD));
        assert!(a.attr.contains(CellAttr::ITALIC));
        let b = t.grid.cell(0, 1);
        assert!(!b.attr.contains(CellAttr::BOLD));
    }

    #[test]
    fn vt_sgr_256_color() {
        let t = run(b"\x1b[38;5;196mR");
        let cell = t.grid.cell(0, 0);
        assert_eq!(cell.fg, Color { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn vt_sgr_truecolor() {
        let t = run(b"\x1b[38;2;100;150;200mX");
        let cell = t.grid.cell(0, 0);
        assert_eq!(cell.fg, Color { r: 100, g: 150, b: 200 });
    }

    #[test]
    fn vt_sgr_inverse() {
        let t = run(b"\x1b[7mI");
        assert!(t.grid.cell(0, 0).attr.contains(CellAttr::INVERSE));
    }

    // === Alt Screen ===

    #[test]
    fn vt_alt_screen_1049() {
        let t = run(b"Main\x1b[?1049hAlt\x1b[?1049l");
        assert_eq!(t.grid.cell(0, 0).ch, 'M'); // back to main
    }

    // === DSR ===

    #[test]
    fn vt_dsr_cpr() {
        let t = run(b"\x1b[5;10H\x1b[6n");
        assert_eq!(t.write_back, b"\x1b[5;10R");
    }

    #[test]
    fn vt_da() {
        let t = run(b"\x1b[c");
        assert!(t.write_back.starts_with(b"\x1b[?"));
    }

    // === Tabs ===

    #[test]
    fn vt_default_tabs() {
        let t = run(b"\tX");
        assert_eq!(t.grid.cursor_col, 9); // tab to 8, then X at 9
    }

    #[test]
    fn vt_tab_set_clear() {
        let t = run(b"\x1b[1;5H\x1bH\x1b[3g\x1b[1;1H\t");
        // Set tab at col 4, then clear ALL tabs, tab goes to end of line
        assert_eq!(t.grid.cursor_col, 79);
    }

    // === Insert/Delete ===

    #[test]
    fn vt_insert_chars() {
        let t = run(b"ABCDE\x1b[1;3H\x1b[2@");
        assert_eq!(t.grid.cell(0, 0).ch, 'A');
        assert_eq!(t.grid.cell(0, 1).ch, 'B');
        assert_eq!(t.grid.cell(0, 2).ch, ' ');
        assert_eq!(t.grid.cell(0, 3).ch, ' ');
        assert_eq!(t.grid.cell(0, 4).ch, 'C');
    }

    #[test]
    fn vt_delete_chars() {
        let t = run(b"ABCDE\x1b[1;2H\x1b[2P");
        assert_eq!(t.grid.cell(0, 0).ch, 'A');
        assert_eq!(t.grid.cell(0, 1).ch, 'D');
        assert_eq!(t.grid.cell(0, 2).ch, 'E');
    }

    #[test]
    fn vt_insert_lines() {
        let t = run(b"AAA\r\nBBB\r\nCCC\x1b[2;1H\x1b[1L");
        assert_eq!(t.grid.cell(0, 0).ch, 'A');
        assert_eq!(t.grid.cell(1, 0).ch, ' '); // inserted blank
        assert_eq!(t.grid.cell(2, 0).ch, 'B');
    }

    #[test]
    fn vt_delete_lines() {
        let t = run(b"AAA\r\nBBB\r\nCCC\x1b[2;1H\x1b[1M");
        assert_eq!(t.grid.cell(0, 0).ch, 'A');
        assert_eq!(t.grid.cell(1, 0).ch, 'C');
    }

    // === Wrap ===

    #[test]
    fn vt_auto_wrap() {
        let mut t = Terminal::new(5, 3);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"ABCDE");
        assert_eq!(t.grid.cursor_col, 5); // pending wrap
        t.feed_bytes(&mut p, b"F");
        assert_eq!(t.grid.cursor_row, 1);
        assert_eq!(t.grid.cursor_col, 1);
        assert_eq!(t.grid.cell(1, 0).ch, 'F');
    }

    #[test]
    fn vt_no_wrap() {
        let mut t = Terminal::new(5, 3);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[?7l"); // disable wrap
        t.feed_bytes(&mut p, b"ABCDEFGH");
        assert_eq!(t.grid.cursor_row, 0);
        assert_eq!(t.grid.cell(0, 4).ch, 'H'); // last char overwrites
    }

    // === DEC Modes ===

    #[test]
    fn vt_origin_mode() {
        let mut t = Terminal::new(80, 24);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[5;20r");  // scroll region 5-20
        t.feed_bytes(&mut p, b"\x1b[?6h");    // origin mode
        t.feed_bytes(&mut p, b"\x1b[1;1H");   // home → row 4 (top of region)
        assert_eq!(t.grid.cursor_row, 4);
    }

    #[test]
    fn vt_decckm() {
        let t = run(b"\x1b[?1h");
        assert!(t.cursor_keys_app);
    }

    #[test]
    fn vt_bracketed_paste() {
        let t = run(b"\x1b[?2004h");
        assert!(t.bracketed_paste);
    }

    // === Full Reset ===

    #[test]
    fn vt_ris() {
        let t = run(b"\x1b[1;31m\x1b[5;10H\x1bc");
        assert_eq!(t.grid.cursor_row, 0);
        assert_eq!(t.grid.cursor_col, 0);
        assert!(!t.cursor_keys_app);
    }

    // === NEL / IND ===

    #[test]
    fn vt_nel() {
        let t = run(b"ABC\x1bE");
        assert_eq!(t.grid.cursor_row, 1);
        assert_eq!(t.grid.cursor_col, 0);
    }

    #[test]
    fn vt_ind() {
        let t = run(b"\x1b[1;1H\x1bD");
        assert_eq!(t.grid.cursor_row, 1);
    }

    // === DECALN ===

    #[test]
    fn vt_decaln() {
        let t = run(b"\x1b#8");
        for c in 0..80 {
            assert_eq!(t.grid.cell(0, c).ch, 'E');
        }
    }

    // === Repeat ===

    #[test]
    fn vt_rep() {
        let t = run(b"X\x1b[5b");
        for c in 0..6 {
            assert_eq!(t.grid.cell(0, c).ch, 'X');
        }
    }
}
