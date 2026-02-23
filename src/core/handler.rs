/// Terminal handler: translates VT parser Actions into Grid operations.
/// This is the brain that interprets CSI/ESC/OSC sequences.

use crate::core::grid::{Grid, Cell, CellAttr, Color};
use crate::core::parser::Action;
use crate::core::utf8::{Utf8Decoder, char_width};

/// Standard 8 ANSI colors + bright variants
const ANSI_COLORS: [Color; 16] = [
    Color { r: 0,   g: 0,   b: 0   }, // 0 black
    Color { r: 205, g: 49,  b: 49  }, // 1 red
    Color { r: 13,  g: 188, b: 121 }, // 2 green
    Color { r: 229, g: 229, b: 16  }, // 3 yellow
    Color { r: 36,  g: 114, b: 200 }, // 4 blue
    Color { r: 188, g: 63,  b: 188 }, // 5 magenta
    Color { r: 17,  g: 168, b: 205 }, // 6 cyan
    Color { r: 204, g: 204, b: 204 }, // 7 white
    Color { r: 102, g: 102, b: 102 }, // 8 bright black
    Color { r: 241, g: 76,  b: 76  }, // 9 bright red
    Color { r: 35,  g: 209, b: 139 }, // 10 bright green
    Color { r: 245, g: 245, b: 67  }, // 11 bright yellow
    Color { r: 59,  g: 142, b: 234 }, // 12 bright blue
    Color { r: 214, g: 112, b: 214 }, // 13 bright magenta
    Color { r: 41,  g: 184, b: 219 }, // 14 bright cyan
    Color { r: 242, g: 242, b: 242 }, // 15 bright white
];

pub struct Terminal {
    pub grid: Grid,
    utf8: Utf8Decoder,
    attr: CellAttr,
    fg: Color,
    bg: Color,
    saved_cursor: (usize, usize),
    saved_attr: CellAttr,
    saved_fg: Color,
    saved_bg: Color,
    /// Alternate screen buffer
    alt_grid: Option<Grid>,
    /// Tab stops (column indices)
    tab_stops: Vec<bool>,
    /// Origin mode (DECOM)
    origin_mode: bool,
    /// Auto-wrap mode (DECAWM)
    auto_wrap: bool,
    /// Scroll region (top, bottom) — inclusive
    scroll_top: usize,
    scroll_bottom: usize,
    /// Title set via OSC
    pub title: String,
    /// Write-back buffer for DSR responses
    pub write_back: Vec<u8>,
    /// Cursor key mode: true = application, false = normal
    pub cursor_keys_app: bool,
    /// Cursor visible
    pub cursor_visible: bool,
    /// Bracketed paste mode
    pub bracketed_paste: bool,
    /// Mouse reporting mode
    pub mouse_mode: MouseMode,
    /// Mouse encoding
    pub mouse_encoding: MouseEncoding,
    /// Keypad application mode
    pub keypad_app: bool,
    /// OSC 7 working directory (latest)
    pub osc7_cwd: Option<String>,
    /// OSC 133 shell integration data (latest)
    pub osc133_data: Option<String>,
    /// OSC 52 clipboard data (latest)
    pub osc52_data: Option<String>,
    /// Shell integration state
    pub shell: crate::shell_integration::ShellIntegration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMode {
    Off,
    X10,       // 9 — press only
    Normal,    // 1000 — press + release
    Button,    // 1002 — press + release + drag
    Any,       // 1003 — all motion
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEncoding {
    X10,   // default
    Sgr,   // 1006
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut tab_stops = vec![false; cols];
        for i in (0..cols).step_by(8) {
            tab_stops[i] = true;
        }
        Self {
            grid: Grid::new(cols, rows),
            utf8: Utf8Decoder::new(),
            attr: CellAttr::empty(),
            fg: Color::DEFAULT_FG,
            bg: Color::DEFAULT_BG,
            saved_cursor: (0, 0),
            saved_attr: CellAttr::empty(),
            saved_fg: Color::DEFAULT_FG,
            saved_bg: Color::DEFAULT_BG,
            alt_grid: None,
            tab_stops,
            origin_mode: false,
            auto_wrap: true,
            scroll_top: 0,
            scroll_bottom: rows - 1,
            title: String::new(),
            write_back: Vec::new(),
            cursor_keys_app: false,
            cursor_visible: true,
            bracketed_paste: false,
            mouse_mode: MouseMode::Off,
            mouse_encoding: MouseEncoding::X10,
            keypad_app: false,
            osc7_cwd: None,
            osc133_data: None,
            osc52_data: None,
            shell: crate::shell_integration::ShellIntegration::new(),
        }
    }

    /// Feed raw bytes from PTY. Decodes UTF-8 and processes VT actions.
    pub fn feed_bytes(&mut self, parser: &mut crate::core::parser::VtParser, data: &[u8]) {
        for &byte in data {
            // Let parser handle control chars and escape sequences directly
            if byte < 0x80 || self.utf8.is_pending() || byte >= 0x80 {
                let action = parser.advance(byte);
                match action {
                    Action::Print(ch) if ch == char::REPLACEMENT_CHARACTER && byte >= 0x80 => {
                        // Parser doesn't handle UTF-8; decode ourselves
                        if let Some(decoded) = self.utf8.feed(byte) {
                            self.print(decoded);
                        }
                    }
                    _ => self.handle_action(action),
                }
            }
        }
    }

    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::Print(ch) => self.print(ch),
            Action::Execute(byte) => self.execute(byte),
            Action::CsiDispatch { final_byte, params, intermediates } => {
                self.csi_dispatch(final_byte, &params, &intermediates);
            }
            Action::EscDispatch { final_byte, intermediates } => {
                self.esc_dispatch(final_byte, &intermediates);
            }
            Action::OscDispatch(data) => self.osc_dispatch(&data),
            Action::None => {}
        }
    }

    fn print(&mut self, ch: char) {
        let width = char_width(ch);
        if width == 0 {
            return;
        }

        let cols = self.grid.cols();
        // Auto-wrap
        if self.grid.cursor_col >= cols {
            if self.auto_wrap {
                self.grid.cursor_col = 0;
                self.index();
            } else {
                self.grid.cursor_col = cols - 1;
            }
        }

        // For wide chars, check if there's room
        if width == 2 && self.grid.cursor_col + 1 >= cols {
            if self.auto_wrap {
                self.grid.put_char(' ', self.attr, self.fg, self.bg);
                self.grid.cursor_col = 0;
                self.index();
            } else {
                // No room for wide char at end, overwrite last cell
                self.grid.cursor_col = cols - 2;
            }
        }

        self.grid.put_char(ch, self.attr, self.fg, self.bg);

        // Wide char occupies two cells
        if width == 2 && self.grid.cursor_col < cols {
            let cell = self.grid.cell_mut(self.grid.cursor_row, self.grid.cursor_col);
            cell.ch = '\0';
            cell.attr = self.attr;
            cell.fg = self.fg;
            cell.bg = self.bg;
            self.grid.cursor_col += 1;
        }

        // Clamp cursor when auto-wrap is off
        if !self.auto_wrap && self.grid.cursor_col >= cols {
            self.grid.cursor_col = cols - 1;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => {} // BEL — TODO: visual bell
            0x08 => {  // BS
                if self.grid.cursor_col > 0 {
                    self.grid.cursor_col -= 1;
                }
            }
            0x09 => {  // HT (tab)
                let col = self.grid.cursor_col;
                let cols = self.grid.cols();
                let next = self.tab_stops.iter()
                    .enumerate()
                    .skip(col + 1)
                    .find(|(_, &stop)| stop)
                    .map(|(i, _)| i)
                    .unwrap_or(cols - 1);
                self.grid.cursor_col = next;
            }
            0x0a | 0x0b | 0x0c => self.index(), // LF, VT, FF
            0x0d => self.grid.cursor_col = 0,     // CR
            _ => {}
        }
    }

    /// Move cursor down one line, scrolling if at bottom of scroll region.
    fn index(&mut self) {
        if self.grid.cursor_row == self.scroll_bottom {
            self.grid.scroll_region_up(self.scroll_top, self.scroll_bottom);
        } else if self.grid.cursor_row < self.grid.rows() - 1 {
            self.grid.cursor_row += 1;
        }
    }

    /// Move cursor up one line, scrolling down if at top of scroll region.
    fn reverse_index(&mut self) {
        if self.grid.cursor_row == self.scroll_top {
            self.grid.scroll_region_down(self.scroll_top, self.scroll_bottom);
        } else if self.grid.cursor_row > 0 {
            self.grid.cursor_row -= 1;
        }
    }

    fn csi_dispatch(&mut self, final_byte: u8, params: &[u16], intermediates: &[u8]) {
        let is_private = intermediates.first() == Some(&b'?');
        let is_space = intermediates.first() == Some(&b' ');

        match final_byte {
            // Cursor movement
            b'A' => { // CUU
                let n = param(params, 0, 1) as usize;
                let top = if self.origin_mode { self.scroll_top } else { 0 };
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n).max(top);
            }
            b'B' => { // CUD
                let n = param(params, 0, 1) as usize;
                let bottom = if self.origin_mode { self.scroll_bottom } else { self.grid.rows() - 1 };
                self.grid.cursor_row = (self.grid.cursor_row + n).min(bottom);
            }
            b'C' => { // CUF
                let n = param(params, 0, 1) as usize;
                self.grid.cursor_col = (self.grid.cursor_col + n).min(self.grid.cols() - 1);
            }
            b'D' => { // CUB
                let n = param(params, 0, 1) as usize;
                self.grid.cursor_col = self.grid.cursor_col.saturating_sub(n);
            }
            b'E' => { // CNL
                let n = param(params, 0, 1) as usize;
                let max = self.grid.rows() - 1;
                self.grid.cursor_row = (self.grid.cursor_row + n).min(max);
                self.grid.cursor_col = 0;
            }
            b'F' => { // CPL
                let n = param(params, 0, 1) as usize;
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n);
                self.grid.cursor_col = 0;
            }
            b'G' | b'`' => { // CHA / HPA
                let col = param(params, 0, 1) as usize;
                self.grid.cursor_col = (col - 1).min(self.grid.cols() - 1);
            }
            b'H' | b'f' => { // CUP / HVP
                let row = param(params, 0, 1) as usize;
                let col = param(params, 1, 1) as usize;
                let offset = if self.origin_mode { self.scroll_top } else { 0 };
                self.grid.cursor_row = (offset + row - 1).min(self.grid.rows() - 1);
                self.grid.cursor_col = (col - 1).min(self.grid.cols() - 1);
            }
            b'd' => { // VPA
                let row = param(params, 0, 1) as usize;
                self.grid.cursor_row = (row - 1).min(self.grid.rows() - 1);
            }
            b'I' => { // CHT — Cursor Forward Tabulation
                let n = param(params, 0, 1) as usize;
                for _ in 0..n {
                    let col = self.grid.cursor_col;
                    let next = self.tab_stops.iter()
                        .enumerate().skip(col + 1)
                        .find(|(_, &s)| s).map(|(i, _)| i)
                        .unwrap_or(self.grid.cols() - 1);
                    self.grid.cursor_col = next;
                }
            }
            b'Z' => { // CBT — Cursor Backward Tabulation
                let n = param(params, 0, 1) as usize;
                for _ in 0..n {
                    let col = self.grid.cursor_col;
                    let prev = self.tab_stops.iter()
                        .enumerate().rev().skip(self.tab_stops.len() - col)
                        .find(|(_, &s)| s).map(|(i, _)| i)
                        .unwrap_or(0);
                    self.grid.cursor_col = prev;
                }
            }

            // Erase
            b'J' => {
                let mode = param(params, 0, 0);
                match mode {
                    0 => self.grid.erase_below(),
                    1 => self.grid.erase_above(),
                    2 | 3 => self.grid.clear(),
                    _ => {}
                }
            }
            b'K' => {
                let mode = param(params, 0, 0);
                match mode {
                    0 => self.grid.erase_line_right(),
                    1 => self.grid.erase_line_left(),
                    2 => self.grid.erase_line(),
                    _ => {}
                }
            }
            b'X' => { // ECH — Erase Characters
                let n = param(params, 0, 1) as usize;
                let row = self.grid.cursor_row;
                let col = self.grid.cursor_col;
                for c in col..(col + n).min(self.grid.cols()) {
                    *self.grid.cell_mut(row, c) = Cell::default();
                }
            }

            // Insert/Delete
            b'L' => {
                let n = param(params, 0, 1) as usize;
                self.grid.insert_lines(self.grid.cursor_row, n, self.scroll_bottom);
            }
            b'M' => {
                let n = param(params, 0, 1) as usize;
                self.grid.delete_lines(self.grid.cursor_row, n, self.scroll_bottom);
            }
            b'P' => {
                let n = param(params, 0, 1) as usize;
                self.grid.delete_chars(n);
            }
            b'@' => {
                let n = param(params, 0, 1) as usize;
                self.grid.insert_chars(n);
            }

            // Scroll
            b'S' if !is_private => {
                let n = param(params, 0, 1) as usize;
                for _ in 0..n {
                    self.grid.scroll_region_up(self.scroll_top, self.scroll_bottom);
                }
            }
            b'T' => {
                let n = param(params, 0, 1) as usize;
                for _ in 0..n {
                    self.grid.scroll_region_down(self.scroll_top, self.scroll_bottom);
                }
            }

            // REP — Repeat preceding character
            b'b' => {
                let n = param(params, 0, 1) as usize;
                if self.grid.cursor_col > 0 {
                    let ch = self.grid.cell(self.grid.cursor_row, self.grid.cursor_col - 1).ch;
                    for _ in 0..n {
                        self.print(ch);
                    }
                }
            }

            // SGR
            b'm' => self.handle_sgr(params),

            // Scroll region
            b'r' if !is_private => {
                let top = param(params, 0, 1) as usize;
                let bottom = param(params, 1, self.grid.rows() as u16) as usize;
                self.scroll_top = (top - 1).min(self.grid.rows() - 1);
                self.scroll_bottom = (bottom - 1).min(self.grid.rows() - 1);
                self.grid.cursor_row = if self.origin_mode { self.scroll_top } else { 0 };
                self.grid.cursor_col = 0;
            }

            // DEC Private modes
            b'h' if is_private => self.set_dec_mode(params, true),
            b'l' if is_private => self.set_dec_mode(params, false),
            // SM — Set Mode (ANSI)
            b'h' if !is_private => self.set_ansi_mode(params, true),
            b'l' if !is_private => self.set_ansi_mode(params, false),

            // Save/restore cursor (ANSI)
            b's' if !is_private => {
                self.saved_cursor = (self.grid.cursor_row, self.grid.cursor_col);
                self.saved_attr = self.attr;
                self.saved_fg = self.fg;
                self.saved_bg = self.bg;
            }
            b'u' => {
                let (r, c) = self.saved_cursor;
                self.grid.cursor_row = r.min(self.grid.rows() - 1);
                self.grid.cursor_col = c.min(self.grid.cols() - 1);
                self.attr = self.saved_attr;
                self.fg = self.saved_fg;
                self.bg = self.saved_bg;
            }

            // DSR — Device Status Report
            b'n' if !is_private => {
                match param(params, 0, 0) {
                    5 => { // Status report — "OK"
                        self.write_back.extend_from_slice(b"\x1b[0n");
                    }
                    6 => { // CPR — Cursor Position Report
                        let r = self.grid.cursor_row + 1;
                        let c = self.grid.cursor_col + 1;
                        let resp = format!("\x1b[{};{}R", r, c);
                        self.write_back.extend_from_slice(resp.as_bytes());
                    }
                    _ => {}
                }
            }
            b'n' if is_private => {
                match param(params, 0, 0) {
                    6 => { // DECXCPR
                        let r = self.grid.cursor_row + 1;
                        let c = self.grid.cursor_col + 1;
                        let resp = format!("\x1b[?{};{}R", r, c);
                        self.write_back.extend_from_slice(resp.as_bytes());
                    }
                    _ => {}
                }
            }

            // DA — Device Attributes
            b'c' if !is_private => {
                if param(params, 0, 0) == 0 {
                    // Report as VT220
                    self.write_back.extend_from_slice(b"\x1b[?62;22c");
                }
            }

            // Tab clear
            b'g' => {
                match param(params, 0, 0) {
                    0 => { // Clear tab at cursor
                        let col = self.grid.cursor_col;
                        if col < self.tab_stops.len() {
                            self.tab_stops[col] = false;
                        }
                    }
                    3 => { // Clear all tabs
                        self.tab_stops.fill(false);
                    }
                    _ => {}
                }
            }

            // DECSCUSR — Set Cursor Style (CSI Ps SP q)
            b'q' if is_space => {
                // 0,1 = block blink, 2 = block steady, 3 = underline blink,
                // 4 = underline steady, 5 = bar blink, 6 = bar steady
                // TODO: pass to renderer
            }

            _ => {} // Unhandled CSI
        }
    }

    fn handle_sgr(&mut self, params: &[u16]) {
        if params.is_empty() {
            self.sgr_reset();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.sgr_reset(),
                1 => self.attr.insert(CellAttr::BOLD),
                2 => self.attr.insert(CellAttr::DIM),
                3 => self.attr.insert(CellAttr::ITALIC),
                4 => self.attr.insert(CellAttr::UNDERLINE),
                7 => self.attr.insert(CellAttr::INVERSE),
                8 => self.attr.insert(CellAttr::HIDDEN),
                9 => self.attr.insert(CellAttr::STRIKETHROUGH),
                22 => { self.attr.remove(CellAttr::BOLD); self.attr.remove(CellAttr::DIM); }
                23 => self.attr.remove(CellAttr::ITALIC),
                24 => self.attr.remove(CellAttr::UNDERLINE),
                27 => self.attr.remove(CellAttr::INVERSE),
                28 => self.attr.remove(CellAttr::HIDDEN),
                29 => self.attr.remove(CellAttr::STRIKETHROUGH),
                // Foreground colors
                30..=37 => self.fg = ANSI_COLORS[(params[i] - 30) as usize],
                38 => {
                    if let Some((color, skip)) = parse_extended_color(params, i + 1) {
                        self.fg = color;
                        i += skip;
                    }
                }
                39 => self.fg = Color::DEFAULT_FG,
                90..=97 => self.fg = ANSI_COLORS[(params[i] - 90 + 8) as usize],
                // Background colors
                40..=47 => self.bg = ANSI_COLORS[(params[i] - 40) as usize],
                48 => {
                    if let Some((color, skip)) = parse_extended_color(params, i + 1) {
                        self.bg = color;
                        i += skip;
                    }
                }
                49 => self.bg = Color::DEFAULT_BG,
                100..=107 => self.bg = ANSI_COLORS[(params[i] - 100 + 8) as usize],
                _ => {}
            }
            i += 1;
        }
    }

    fn sgr_reset(&mut self) {
        self.attr = CellAttr::empty();
        self.fg = Color::DEFAULT_FG;
        self.bg = Color::DEFAULT_BG;
    }

    fn set_dec_mode(&mut self, params: &[u16], enable: bool) {
        for &p in params {
            match p {
                1 => self.cursor_keys_app = enable,  // DECCKM
                6 => self.origin_mode = enable,       // DECOM
                7 => self.auto_wrap = enable,         // DECAWM
                12 => {}                              // Cursor blink (renderer)
                25 => self.cursor_visible = enable,   // DECTCEM
                9 => self.mouse_mode = if enable { MouseMode::X10 } else { MouseMode::Off },
                1000 => self.mouse_mode = if enable { MouseMode::Normal } else { MouseMode::Off },
                1002 => self.mouse_mode = if enable { MouseMode::Button } else { MouseMode::Off },
                1003 => self.mouse_mode = if enable { MouseMode::Any } else { MouseMode::Off },
                1006 => self.mouse_encoding = if enable { MouseEncoding::Sgr } else { MouseEncoding::X10 },
                47 => { // Alt screen (no save/restore cursor)
                    if enable {
                        let (cols, rows) = (self.grid.cols(), self.grid.rows());
                        let old = std::mem::replace(&mut self.grid, Grid::new(cols, rows));
                        self.alt_grid = Some(old);
                    } else if let Some(main) = self.alt_grid.take() {
                        self.grid = main;
                    }
                }
                1047 => { // Alt screen (clear on enter)
                    if enable {
                        let (cols, rows) = (self.grid.cols(), self.grid.rows());
                        let old = std::mem::replace(&mut self.grid, Grid::new(cols, rows));
                        self.alt_grid = Some(old);
                    } else if let Some(main) = self.alt_grid.take() {
                        self.grid = main;
                    }
                }
                1048 => { // Save/restore cursor
                    if enable {
                        self.saved_cursor = (self.grid.cursor_row, self.grid.cursor_col);
                        self.saved_attr = self.attr;
                        self.saved_fg = self.fg;
                        self.saved_bg = self.bg;
                    } else {
                        let (r, c) = self.saved_cursor;
                        self.grid.cursor_row = r.min(self.grid.rows() - 1);
                        self.grid.cursor_col = c.min(self.grid.cols() - 1);
                        self.attr = self.saved_attr;
                        self.fg = self.saved_fg;
                        self.bg = self.saved_bg;
                    }
                }
                1049 => { // Alt screen + save/restore cursor
                    if enable {
                        let (cols, rows) = (self.grid.cols(), self.grid.rows());
                        self.saved_cursor = (self.grid.cursor_row, self.grid.cursor_col);
                        self.saved_attr = self.attr;
                        self.saved_fg = self.fg;
                        self.saved_bg = self.bg;
                        let old = std::mem::replace(&mut self.grid, Grid::new(cols, rows));
                        self.alt_grid = Some(old);
                    } else if let Some(main) = self.alt_grid.take() {
                        self.grid = main;
                        let (r, c) = self.saved_cursor;
                        self.grid.cursor_row = r.min(self.grid.rows() - 1);
                        self.grid.cursor_col = c.min(self.grid.cols() - 1);
                        self.attr = self.saved_attr;
                        self.fg = self.saved_fg;
                        self.bg = self.saved_bg;
                    }
                }
                2004 => self.bracketed_paste = enable,
                _ => {}
            }
        }
    }

    fn set_ansi_mode(&mut self, params: &[u16], _enable: bool) {
        for &p in params {
            match p {
                4 => {} // IRM — Insert/Replace mode (TODO)
                20 => {} // LNM — Line feed/new line mode
                _ => {}
            }
        }
    }

    fn esc_dispatch(&mut self, final_byte: u8, intermediates: &[u8]) {
        // Check for ESC # sequences
        if intermediates.first() == Some(&b'#') {
            match final_byte {
                b'8' => { // DECALN — fill screen with 'E'
                    for r in 0..self.grid.rows() {
                        for c in 0..self.grid.cols() {
                            let cell = self.grid.cell_mut(r, c);
                            cell.ch = 'E';
                            cell.attr = CellAttr::empty();
                            cell.fg = Color::DEFAULT_FG;
                            cell.bg = Color::DEFAULT_BG;
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        match final_byte {
            b'7' => { // DECSC — Save Cursor + attrs
                self.saved_cursor = (self.grid.cursor_row, self.grid.cursor_col);
                self.saved_attr = self.attr;
                self.saved_fg = self.fg;
                self.saved_bg = self.bg;
            }
            b'8' => { // DECRC — Restore Cursor + attrs
                let (r, c) = self.saved_cursor;
                self.grid.cursor_row = r.min(self.grid.rows() - 1);
                self.grid.cursor_col = c.min(self.grid.cols() - 1);
                self.attr = self.saved_attr;
                self.fg = self.saved_fg;
                self.bg = self.saved_bg;
            }
            b'M' => self.reverse_index(),
            b'D' => self.index(),
            b'E' => {
                self.grid.cursor_col = 0;
                self.index();
            }
            b'H' => { // HTS — Horizontal Tab Set
                let col = self.grid.cursor_col;
                if col < self.tab_stops.len() {
                    self.tab_stops[col] = true;
                }
            }
            b'=' => self.keypad_app = true,   // DECKPAM
            b'>' => self.keypad_app = false,  // DECKPNM
            b'c' => { // RIS — Full Reset
                let cols = self.grid.cols();
                let rows = self.grid.rows();
                *self = Terminal::new(cols, rows);
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, data: &[u8]) {
        let s = String::from_utf8_lossy(data);
        if let Some(rest) = s.strip_prefix("0;").or_else(|| s.strip_prefix("2;")) {
            self.title = rest.to_string();
        }
        // OSC 7 — working directory
        if s.starts_with("7;") {
            if let Some(url) = s.strip_prefix("7;") {
                self.osc7_cwd = Some(url.to_string());
                self.shell.handle_osc7(url);
            }
        }
        // OSC 133 — shell integration (FinalTerm)
        if let Some(rest) = s.strip_prefix("133;") {
            self.osc133_data = Some(rest.to_string());
            self.shell.handle_osc133(rest, self.grid.cursor_row);
        }
        // OSC 52 — clipboard
        if s.starts_with("52;") {
            self.osc52_data = Some(s.to_string());
        }
    }

    pub fn set_default_colors(&mut self, fg: Color, bg: Color) {
        self.fg = fg;
        self.bg = bg;
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.grid.resize(cols, rows);
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;
        self.tab_stops = vec![false; cols];
        for i in (0..cols).step_by(8) {
            self.tab_stops[i] = true;
        }
    }
}

/// Get param at index with default value.
fn param(params: &[u16], idx: usize, default: u16) -> u16 {
    params.get(idx).copied().filter(|&v| v != 0).unwrap_or(default)
}

/// Parse 256-color (38;5;N) or truecolor (38;2;R;G;B) sequences.
/// Returns (Color, number of extra params consumed).
fn parse_extended_color(params: &[u16], start: usize) -> Option<(Color, usize)> {
    match params.get(start)? {
        5 => {
            // 256-color: index
            let idx = *params.get(start + 1)? as usize;
            Some((color_from_256(idx), 2))
        }
        2 => {
            // Truecolor: R;G;B
            let r = *params.get(start + 1)? as u8;
            let g = *params.get(start + 2)? as u8;
            let b = *params.get(start + 3)? as u8;
            Some((Color { r, g, b }, 4))
        }
        _ => None,
    }
}

/// Convert 256-color index to RGB.
fn color_from_256(idx: usize) -> Color {
    match idx {
        0..=15 => ANSI_COLORS[idx],
        16..=231 => {
            // 6x6x6 color cube
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_val = |v: usize| if v == 0 { 0u8 } else { (55 + 40 * v) as u8 };
            Color { r: to_val(r), g: to_val(g), b: to_val(b) }
        }
        232..=255 => {
            // Grayscale ramp
            let v = (8 + 10 * (idx - 232)) as u8;
            Color { r: v, g: v, b: v }
        }
        _ => Color::DEFAULT_FG,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::VtParser;

    fn make_term() -> (Terminal, VtParser) {
        (Terminal::new(80, 24), VtParser::new())
    }

    fn small_term() -> (Terminal, VtParser) {
        (Terminal::new(10, 5), VtParser::new())
    }

    fn grid_row(t: &Terminal, row: usize) -> String {
        (0..t.grid.cols())
            .map(|c| t.grid.cell(row, c).ch)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    // --- Basic printing ---

    #[test]
    fn test_print_ascii() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"Hello");
        assert_eq!(t.grid.cursor_col, 5);
        assert_eq!(t.grid.cell(0, 0).ch, 'H');
        assert_eq!(t.grid.cell(0, 4).ch, 'o');
    }

    #[test]
    fn test_cjk_wide_char() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, "中文".as_bytes());
        assert_eq!(t.grid.cursor_col, 4);
        assert_eq!(t.grid.cell(0, 0).ch, '中');
        assert_eq!(t.grid.cell(0, 1).ch, '\0');
        assert_eq!(t.grid.cell(0, 2).ch, '文');
        assert_eq!(t.grid.cell(0, 3).ch, '\0');
    }

    #[test]
    fn test_cjk_wrap_at_boundary() {
        // 9-col terminal: wide char at col 8 should wrap
        let (mut t, mut p) = (Terminal::new(9, 3), VtParser::new());
        t.feed_bytes(&mut p, b"AAAAAAAA"); // 8 chars, cursor at col 8
        t.feed_bytes(&mut p, "中".as_bytes()); // needs 2 cols, should wrap
        assert_eq!(t.grid.cursor_row, 1);
        assert_eq!(t.grid.cursor_col, 2);
        assert_eq!(t.grid.cell(1, 0).ch, '中');
    }

    #[test]
    fn test_emoji() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, "😀".as_bytes());
        assert_eq!(t.grid.cursor_col, 2);
        assert_eq!(t.grid.cell(0, 0).ch, '😀');
    }

    // --- Cursor movement ---

    #[test]
    fn test_cursor_movement() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[5;10H");
        assert_eq!(t.grid.cursor_row, 4);
        assert_eq!(t.grid.cursor_col, 9);
    }

    #[test]
    fn test_cursor_up_down() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[10;1H"); // row 10
        t.feed_bytes(&mut p, b"\x1b[3A");    // up 3
        assert_eq!(t.grid.cursor_row, 6);
        t.feed_bytes(&mut p, b"\x1b[5B");    // down 5
        assert_eq!(t.grid.cursor_row, 11);
    }

    #[test]
    fn test_cursor_forward_back() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[1;20H"); // col 20
        t.feed_bytes(&mut p, b"\x1b[5D");    // back 5
        assert_eq!(t.grid.cursor_col, 14);
        t.feed_bytes(&mut p, b"\x1b[10C");   // forward 10
        assert_eq!(t.grid.cursor_col, 24);
    }

    #[test]
    fn test_cursor_clamp_to_bounds() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"\x1b[100;100H"); // way out of bounds
        assert_eq!(t.grid.cursor_row, 4);  // clamped to rows-1
        assert_eq!(t.grid.cursor_col, 9);  // clamped to cols-1
    }

    #[test]
    fn test_cursor_up_clamp_zero() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[1;1H");  // top-left
        t.feed_bytes(&mut p, b"\x1b[999A");  // up 999
        assert_eq!(t.grid.cursor_row, 0);    // clamped
    }

    #[test]
    fn test_cnl_cpl() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[5;10H"); // row 5, col 10
        t.feed_bytes(&mut p, b"\x1b[2E");    // CNL: next line ×2
        assert_eq!(t.grid.cursor_row, 6);
        assert_eq!(t.grid.cursor_col, 0);
        t.feed_bytes(&mut p, b"\x1b[1F");    // CPL: prev line ×1
        assert_eq!(t.grid.cursor_row, 5);
        assert_eq!(t.grid.cursor_col, 0);
    }

    #[test]
    fn test_cha_vpa() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[15G");   // CHA: col 15
        assert_eq!(t.grid.cursor_col, 14);
        t.feed_bytes(&mut p, b"\x1b[8d");    // VPA: row 8
        assert_eq!(t.grid.cursor_row, 7);
    }

    // --- SGR ---

    #[test]
    fn test_sgr_colors() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[1;31m");
        assert!(t.attr.contains(CellAttr::BOLD));
        assert_eq!(t.fg, ANSI_COLORS[1]);
    }

    #[test]
    fn test_sgr_truecolor() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[38;2;100;150;200m");
        assert_eq!(t.fg, Color { r: 100, g: 150, b: 200 });
    }

    #[test]
    fn test_sgr_256_color() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[38;5;196m");
        assert_eq!(t.fg, Color { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn test_sgr_256_bg() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[48;5;21m"); // blue in cube
        // 21 - 16 = 5, r=0, g=0, b=5 → (0, 0, 255)
        assert_eq!(t.bg, Color { r: 0, g: 0, b: 255 });
    }

    #[test]
    fn test_sgr_bright_colors() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[91m"); // bright red fg
        assert_eq!(t.fg, ANSI_COLORS[9]);
        t.feed_bytes(&mut p, b"\x1b[104m"); // bright blue bg
        assert_eq!(t.bg, ANSI_COLORS[12]);
    }

    #[test]
    fn test_sgr_reset() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[1;3;4;31m"); // bold+italic+underline+red
        assert!(t.attr.contains(CellAttr::BOLD));
        assert!(t.attr.contains(CellAttr::ITALIC));
        t.feed_bytes(&mut p, b"\x1b[0m"); // reset
        assert_eq!(t.attr, CellAttr::empty());
        assert_eq!(t.fg, Color::DEFAULT_FG);
    }

    #[test]
    fn test_sgr_individual_remove() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[1;3m"); // bold + italic
        t.feed_bytes(&mut p, b"\x1b[22m");  // remove bold only
        assert!(!t.attr.contains(CellAttr::BOLD));
        assert!(t.attr.contains(CellAttr::ITALIC));
    }

    #[test]
    fn test_sgr_256_grayscale() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[38;5;232m"); // darkest gray
        assert_eq!(t.fg, Color { r: 8, g: 8, b: 8 });
        t.feed_bytes(&mut p, b"\x1b[38;5;255m"); // lightest gray
        assert_eq!(t.fg, Color { r: 238, g: 238, b: 238 });
    }

    // --- Erase ---

    #[test]
    fn test_erase_display() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"Hello");
        t.feed_bytes(&mut p, b"\x1b[2J");
        assert_eq!(t.grid.cell(0, 0).ch, ' ');
    }

    #[test]
    fn test_erase_line_right() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"ABCDEFGHIJ");
        t.feed_bytes(&mut p, b"\x1b[1;6H"); // col 6
        t.feed_bytes(&mut p, b"\x1b[0K");   // erase right
        assert_eq!(grid_row(&t, 0), "ABCDE");
    }

    #[test]
    fn test_erase_line_left() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"ABCDEFGHIJ");
        t.feed_bytes(&mut p, b"\x1b[1;4H"); // col 4
        t.feed_bytes(&mut p, b"\x1b[1K");   // erase left
        assert_eq!(t.grid.cell(0, 0).ch, ' ');
        assert_eq!(t.grid.cell(0, 3).ch, ' ');
        assert_eq!(t.grid.cell(0, 4).ch, 'E');
    }

    #[test]
    fn test_erase_entire_line() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"ABCDEFGHIJ");
        t.feed_bytes(&mut p, b"\x1b[1;5H");
        t.feed_bytes(&mut p, b"\x1b[2K");
        assert_eq!(grid_row(&t, 0), "");
    }

    // --- Scroll ---

    #[test]
    fn test_scroll_region() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[5;10r");
        assert_eq!(t.scroll_top, 4);
        assert_eq!(t.scroll_bottom, 9);
        assert_eq!(t.grid.cursor_row, 0); // cursor goes home
    }

    #[test]
    fn test_scroll_up_su() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"Line0\r\nLine1\r\nLine2\r\nLine3\r\nLine4");
        t.feed_bytes(&mut p, b"\x1b[1S"); // scroll up 1
        assert_eq!(grid_row(&t, 0), "Line1");
        assert_eq!(grid_row(&t, 3), "Line4");
        assert_eq!(grid_row(&t, 4), "");
    }

    #[test]
    fn test_scroll_down_sd() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"Line0\r\nLine1\r\nLine2\r\nLine3\r\nLine4");
        t.feed_bytes(&mut p, b"\x1b[1T"); // scroll down 1
        assert_eq!(grid_row(&t, 0), "");
        assert_eq!(grid_row(&t, 1), "Line0");
    }

    // --- Insert/Delete ---

    #[test]
    fn test_insert_lines() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"AAA\r\nBBB\r\nCCC\r\nDDD\r\nEEE");
        t.feed_bytes(&mut p, b"\x1b[2;1H"); // row 2
        t.feed_bytes(&mut p, b"\x1b[1L");   // insert 1 line
        assert_eq!(grid_row(&t, 0), "AAA");
        assert_eq!(grid_row(&t, 1), "");    // inserted
        assert_eq!(grid_row(&t, 2), "BBB");
    }

    #[test]
    fn test_delete_lines() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"AAA\r\nBBB\r\nCCC\r\nDDD\r\nEEE");
        t.feed_bytes(&mut p, b"\x1b[2;1H"); // row 2
        t.feed_bytes(&mut p, b"\x1b[1M");   // delete 1 line
        assert_eq!(grid_row(&t, 0), "AAA");
        assert_eq!(grid_row(&t, 1), "CCC");
        assert_eq!(grid_row(&t, 2), "DDD");
    }

    #[test]
    fn test_delete_chars() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"ABCDEFGHIJ");
        t.feed_bytes(&mut p, b"\x1b[1;4H"); // col 4
        t.feed_bytes(&mut p, b"\x1b[2P");   // delete 2 chars
        assert_eq!(grid_row(&t, 0), "ABCFGHIJ");
    }

    #[test]
    fn test_insert_chars() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"ABCDEFGHIJ");
        t.feed_bytes(&mut p, b"\x1b[1;4H"); // col 4
        t.feed_bytes(&mut p, b"\x1b[2@");   // insert 2 blanks
        assert_eq!(t.grid.cell(0, 3).ch, ' ');
        assert_eq!(t.grid.cell(0, 4).ch, ' ');
        assert_eq!(t.grid.cell(0, 5).ch, 'D');
    }

    // --- ESC sequences ---

    #[test]
    fn test_save_restore_cursor_esc() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[5;10H"); // row 5, col 10
        t.feed_bytes(&mut p, b"\x1b7");       // save
        t.feed_bytes(&mut p, b"\x1b[1;1H");   // home
        t.feed_bytes(&mut p, b"\x1b8");       // restore
        assert_eq!(t.grid.cursor_row, 4);
        assert_eq!(t.grid.cursor_col, 9);
    }

    #[test]
    fn test_save_restore_cursor_csi() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[3;7H");
        t.feed_bytes(&mut p, b"\x1b[s");     // save
        t.feed_bytes(&mut p, b"\x1b[1;1H");
        t.feed_bytes(&mut p, b"\x1b[u");     // restore
        assert_eq!(t.grid.cursor_row, 2);
        assert_eq!(t.grid.cursor_col, 6);
    }

    #[test]
    fn test_reverse_index() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"\x1b[1;1H"); // top
        t.feed_bytes(&mut p, b"\x1bM");     // reverse index at top → scroll down
        assert_eq!(t.grid.cursor_row, 0);
    }

    #[test]
    fn test_full_reset() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[1;31mHello");
        t.feed_bytes(&mut p, b"\x1bc"); // RIS
        assert_eq!(t.grid.cell(0, 0).ch, ' ');
        assert_eq!(t.attr, CellAttr::empty());
        assert_eq!(t.fg, Color::DEFAULT_FG);
        assert_eq!(t.grid.cursor_row, 0);
        assert_eq!(t.grid.cursor_col, 0);
    }

    // --- OSC ---

    #[test]
    fn test_osc_title() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b]0;My Terminal\x07");
        assert_eq!(t.title, "My Terminal");
    }

    #[test]
    fn test_osc_title_type2() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b]2;Window Title\x07");
        assert_eq!(t.title, "Window Title");
    }

    // --- Alt screen ---

    #[test]
    fn test_alt_screen() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"Main");
        assert_eq!(t.grid.cell(0, 0).ch, 'M');
        t.feed_bytes(&mut p, b"\x1b[?1049h");
        assert_eq!(t.grid.cell(0, 0).ch, ' ');
        t.feed_bytes(&mut p, b"Alt");
        t.feed_bytes(&mut p, b"\x1b[?1049l");
        assert_eq!(t.grid.cell(0, 0).ch, 'M');
    }

    #[test]
    fn test_alt_screen_preserves_cursor() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x1b[5;10H"); // row 5, col 10
        t.feed_bytes(&mut p, b"\x1b[?1049h"); // alt
        t.feed_bytes(&mut p, b"\x1b[1;1H");   // move in alt
        t.feed_bytes(&mut p, b"\x1b[?1049l"); // back to main
        assert_eq!(t.grid.cursor_row, 4);
        assert_eq!(t.grid.cursor_col, 9);
    }

    // --- Tab stops ---

    #[test]
    fn test_tab_stop() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"AB\t"); // tab from col 2
        assert_eq!(t.grid.cursor_col, 8); // next tab stop at 8
    }

    // --- Auto-wrap ---

    #[test]
    fn test_auto_wrap_off() {
        let (mut t, mut p) = small_term();
        t.feed_bytes(&mut p, b"\x1b[?7l"); // disable auto-wrap
        t.feed_bytes(&mut p, b"ABCDEFGHIJKLM"); // more than 10 cols
        assert_eq!(t.grid.cursor_row, 0); // no wrap
        assert_eq!(t.grid.cursor_col, 9); // stuck at last col
    }

    // --- C0 controls ---

    #[test]
    fn test_backspace() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"ABC\x08"); // BS
        assert_eq!(t.grid.cursor_col, 2);
    }

    #[test]
    fn test_backspace_at_zero() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"\x08"); // BS at col 0
        assert_eq!(t.grid.cursor_col, 0); // stays
    }

    #[test]
    fn test_cr_lf() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"Hello\r\nWorld");
        assert_eq!(t.grid.cell(0, 0).ch, 'H');
        assert_eq!(t.grid.cell(1, 0).ch, 'W');
    }

    // --- Resize ---

    #[test]
    fn test_terminal_resize() {
        let (mut t, mut p) = make_term();
        t.feed_bytes(&mut p, b"Hello");
        t.resize(40, 12);
        assert_eq!(t.grid.cols(), 40);
        assert_eq!(t.grid.rows(), 12);
        assert_eq!(t.scroll_bottom, 11);
        assert_eq!(t.grid.cell(0, 0).ch, 'H');
    }

    // --- Color helpers ---

    #[test]
    fn test_color_from_256_ansi() {
        assert_eq!(color_from_256(0), ANSI_COLORS[0]);
        assert_eq!(color_from_256(15), ANSI_COLORS[15]);
    }

    #[test]
    fn test_color_from_256_cube() {
        // Index 16 = (0,0,0) = black
        assert_eq!(color_from_256(16), Color { r: 0, g: 0, b: 0 });
        // Index 231 = (5,5,5) = white-ish
        assert_eq!(color_from_256(231), Color { r: 255, g: 255, b: 255 });
    }

    #[test]
    fn test_color_from_256_grayscale() {
        assert_eq!(color_from_256(232), Color { r: 8, g: 8, b: 8 });
        assert_eq!(color_from_256(255), Color { r: 238, g: 238, b: 238 });
    }

    // ---- Phase 3 tests ----

    #[test]
    fn test_dsr_status() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[5n");
        assert_eq!(t.write_back, b"\x1b[0n");
    }

    #[test]
    fn test_dsr_cursor_position() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[3;5H");
        t.write_back.clear();
        t.feed_bytes(&mut p, b"\x1b[6n");
        assert_eq!(t.write_back, b"\x1b[3;5R");
    }

    #[test]
    fn test_device_attributes() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[c");
        assert_eq!(t.write_back, b"\x1b[?62;22c");
    }

    #[test]
    fn test_cursor_keys_app_mode() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        assert!(!t.cursor_keys_app);
        t.feed_bytes(&mut p, b"\x1b[?1h");
        assert!(t.cursor_keys_app);
        t.feed_bytes(&mut p, b"\x1b[?1l");
        assert!(!t.cursor_keys_app);
    }

    #[test]
    fn test_cursor_visible() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        assert!(t.cursor_visible);
        t.feed_bytes(&mut p, b"\x1b[?25l");
        assert!(!t.cursor_visible);
        t.feed_bytes(&mut p, b"\x1b[?25h");
        assert!(t.cursor_visible);
    }

    #[test]
    fn test_bracketed_paste() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        assert!(!t.bracketed_paste);
        t.feed_bytes(&mut p, b"\x1b[?2004h");
        assert!(t.bracketed_paste);
        t.feed_bytes(&mut p, b"\x1b[?2004l");
        assert!(!t.bracketed_paste);
    }

    #[test]
    fn test_mouse_mode() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        assert_eq!(t.mouse_mode, MouseMode::Off);
        t.feed_bytes(&mut p, b"\x1b[?1000h");
        assert_eq!(t.mouse_mode, MouseMode::Normal);
        t.feed_bytes(&mut p, b"\x1b[?1003h");
        assert_eq!(t.mouse_mode, MouseMode::Any);
        t.feed_bytes(&mut p, b"\x1b[?1003l");
        assert_eq!(t.mouse_mode, MouseMode::Off);
    }

    #[test]
    fn test_sgr_mouse_encoding() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        assert_eq!(t.mouse_encoding, MouseEncoding::X10);
        t.feed_bytes(&mut p, b"\x1b[?1006h");
        assert_eq!(t.mouse_encoding, MouseEncoding::Sgr);
    }

    #[test]
    fn test_ech_erase_characters() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"ABCDEFGHIJ");
        t.feed_bytes(&mut p, b"\x1b[1;4H");
        t.feed_bytes(&mut p, b"\x1b[3X");
        assert_eq!(t.grid.cell(0, 3).ch, ' ');
        assert_eq!(t.grid.cell(0, 4).ch, ' ');
        assert_eq!(t.grid.cell(0, 5).ch, ' ');
        assert_eq!(t.grid.cell(0, 6).ch, 'G');
    }

    #[test]
    fn test_tab_set_and_clear() {
        let mut t = Terminal::new(20, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[1;6H");
        t.feed_bytes(&mut p, b"\x1bH");
        t.feed_bytes(&mut p, b"\x1b[1;1H");
        t.feed_bytes(&mut p, b"\t");
        assert_eq!(t.grid.cursor_col, 5);
        t.feed_bytes(&mut p, b"\x1b[0g");
        t.feed_bytes(&mut p, b"\x1b[1;1H");
        t.feed_bytes(&mut p, b"\t");
        assert_eq!(t.grid.cursor_col, 8);
    }

    #[test]
    fn test_keypad_mode() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        assert!(!t.keypad_app);
        t.feed_bytes(&mut p, b"\x1b=");
        assert!(t.keypad_app);
        t.feed_bytes(&mut p, b"\x1b>");
        assert!(!t.keypad_app);
    }

    #[test]
    fn test_decaln() {
        let mut t = Terminal::new(5, 3);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b#8");
        for r in 0..3 {
            for c in 0..5 {
                assert_eq!(t.grid.cell(r, c).ch, 'E');
            }
        }
    }

    #[test]
    fn test_dim_hidden_attr() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[2m");
        t.feed_bytes(&mut p, b"A");
        assert!(t.grid.cell(0, 0).attr.contains(CellAttr::DIM));
        t.feed_bytes(&mut p, b"\x1b[8m");
        t.feed_bytes(&mut p, b"B");
        assert!(t.grid.cell(0, 1).attr.contains(CellAttr::HIDDEN));
        t.feed_bytes(&mut p, b"\x1b[22m");
        t.feed_bytes(&mut p, b"C");
        assert!(!t.grid.cell(0, 2).attr.contains(CellAttr::DIM));
    }

    #[test]
    fn test_alt_screen_1047() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"Hello");
        t.feed_bytes(&mut p, b"\x1b[?1047h");
        assert_eq!(t.grid.cell(0, 0).ch, ' ');
        t.feed_bytes(&mut p, b"\x1b[?1047l");
        assert_eq!(t.grid.cell(0, 0).ch, 'H');
    }

    #[test]
    fn test_save_restore_cursor_with_attrs() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[1;31m");
        t.feed_bytes(&mut p, b"\x1b[s");
        t.feed_bytes(&mut p, b"\x1b[0m");
        t.feed_bytes(&mut p, b"\x1b[u");
        t.feed_bytes(&mut p, b"X");
        assert!(t.grid.cell(0, 0).attr.contains(CellAttr::BOLD));
        assert_eq!(t.grid.cell(0, 0).fg, ANSI_COLORS[1]);
    }

    #[test]
    fn test_rep_repeat_char() {
        let mut t = Terminal::new(10, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"A");
        t.feed_bytes(&mut p, b"\x1b[3b");
        assert_eq!(t.grid.cell(0, 0).ch, 'A');
        assert_eq!(t.grid.cell(0, 1).ch, 'A');
        assert_eq!(t.grid.cell(0, 2).ch, 'A');
        assert_eq!(t.grid.cell(0, 3).ch, 'A');
    }

    #[test]
    fn test_origin_mode_cup() {
        let mut t = Terminal::new(10, 10);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b[3;7r");
        t.feed_bytes(&mut p, b"\x1b[?6h");
        t.feed_bytes(&mut p, b"\x1b[1;1H");
        assert_eq!(t.grid.cursor_row, 2);
    }

    #[test]
    fn test_osc7_working_dir() {
        let mut t = Terminal::new(40, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b]7;file://hostname/home/user\x07");
        assert_eq!(t.osc7_cwd, Some("file://hostname/home/user".into()));
    }

    #[test]
    fn test_osc133_shell_integration() {
        let mut t = Terminal::new(40, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b]133;A\x07");
        assert_eq!(t.osc133_data, Some("A".into()));
    }

    #[test]
    fn test_osc52_clipboard() {
        let mut t = Terminal::new(40, 5);
        let mut p = VtParser::new();
        t.feed_bytes(&mut p, b"\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(t.osc52_data, Some("52;c;aGVsbG8=".into()));
    }
}
