use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CellAttr: u8 {
        const BOLD       = 0b0000_0001;
        const ITALIC     = 0b0000_0010;
        const UNDERLINE  = 0b0000_0100;
        const INVERSE    = 0b0000_1000;
        const STRIKETHROUGH = 0b0001_0000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const DEFAULT_FG: Self = Self { r: 204, g: 204, b: 204 };
    pub const DEFAULT_BG: Self = Self { r: 0, g: 0, b: 0 };
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub attr: CellAttr,
    pub fg: Color,
    pub bg: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            attr: CellAttr::empty(),
            fg: Color::DEFAULT_FG,
            bg: Color::DEFAULT_BG,
        }
    }
}

pub struct Grid {
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
    /// Scrollback buffer (ring buffer of rows)
    scrollback: Vec<Vec<Cell>>,
    scrollback_max: usize,
    /// Cursor position
    pub cursor_row: usize,
    pub cursor_col: usize,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            scrollback: Vec::new(),
            scrollback_max: 10_000,
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    pub fn cols(&self) -> usize { self.cols }
    pub fn rows(&self) -> usize { self.rows }

    pub fn cell(&self, row: usize, col: usize) -> &Cell {
        &self.cells[row * self.cols + col]
    }

    pub fn cell_mut(&mut self, row: usize, col: usize) -> &mut Cell {
        &mut self.cells[row * self.cols + col]
    }

    /// Write a character at cursor, advance cursor.
    pub fn put_char(&mut self, ch: char, attr: CellAttr, fg: Color, bg: Color) {
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.newline();
        }
        let cell = self.cell_mut(self.cursor_row, self.cursor_col);
        cell.ch = ch;
        cell.attr = attr;
        cell.fg = fg;
        cell.bg = bg;
        self.cursor_col += 1;
    }

    /// Move to next line, scroll if at bottom.
    pub fn newline(&mut self) {
        if self.cursor_row + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_row += 1;
        }
        self.cursor_col = 0;
    }

    /// Scroll the grid up by one line.
    fn scroll_up(&mut self) {
        // Save top row to scrollback
        let top_row: Vec<Cell> = (0..self.cols)
            .map(|c| *self.cell(0, c))
            .collect();
        self.scrollback.push(top_row);
        if self.scrollback.len() > self.scrollback_max {
            self.scrollback.remove(0);
        }

        // Shift rows up
        for row in 1..self.rows {
            for col in 0..self.cols {
                let src = self.cells[row * self.cols + col];
                self.cells[(row - 1) * self.cols + col] = src;
            }
        }

        // Clear bottom row
        let last = self.rows - 1;
        for col in 0..self.cols {
            self.cells[last * self.cols + col] = Cell::default();
        }
    }

    /// Resize the grid (reflow not implemented yet).
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let mut new_cells = vec![Cell::default(); cols * rows];
        let copy_rows = self.rows.min(rows);
        let copy_cols = self.cols.min(cols);
        for r in 0..copy_rows {
            for c in 0..copy_cols {
                new_cells[r * cols + c] = self.cells[r * self.cols + c];
            }
        }
        self.cells = new_cells;
        self.cols = cols;
        self.rows = rows;
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
    }

    /// Clear entire screen.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    /// Erase from cursor to end of screen.
    pub fn erase_below(&mut self) {
        self.erase_line_right();
        for row in (self.cursor_row + 1)..self.rows {
            for col in 0..self.cols {
                self.cells[row * self.cols + col] = Cell::default();
            }
        }
    }

    /// Erase from start of screen to cursor.
    pub fn erase_above(&mut self) {
        for row in 0..self.cursor_row {
            for col in 0..self.cols {
                self.cells[row * self.cols + col] = Cell::default();
            }
        }
        self.erase_line_left();
    }

    /// Erase from cursor to end of line.
    pub fn erase_line_right(&mut self) {
        let row = self.cursor_row;
        for col in self.cursor_col..self.cols {
            self.cells[row * self.cols + col] = Cell::default();
        }
    }

    /// Erase from start of line to cursor.
    pub fn erase_line_left(&mut self) {
        let row = self.cursor_row;
        for col in 0..=self.cursor_col.min(self.cols - 1) {
            self.cells[row * self.cols + col] = Cell::default();
        }
    }

    /// Erase entire current line.
    pub fn erase_line(&mut self) {
        let row = self.cursor_row;
        for col in 0..self.cols {
            self.cells[row * self.cols + col] = Cell::default();
        }
    }

    /// Scroll a region up by one line.
    pub fn scroll_region_up(&mut self, top: usize, bottom: usize) {
        if top == 0 {
            // Save to scrollback
            let top_row: Vec<Cell> = (0..self.cols).map(|c| self.cells[c]).collect();
            self.scrollback.push(top_row);
            if self.scrollback.len() > self.scrollback_max {
                self.scrollback.remove(0);
            }
        }
        for row in top..bottom {
            for col in 0..self.cols {
                self.cells[row * self.cols + col] = self.cells[(row + 1) * self.cols + col];
            }
        }
        for col in 0..self.cols {
            self.cells[bottom * self.cols + col] = Cell::default();
        }
    }

    /// Scroll a region down by one line.
    pub fn scroll_region_down(&mut self, top: usize, bottom: usize) {
        for row in (top + 1..=bottom).rev() {
            for col in 0..self.cols {
                self.cells[row * self.cols + col] = self.cells[(row - 1) * self.cols + col];
            }
        }
        for col in 0..self.cols {
            self.cells[top * self.cols + col] = Cell::default();
        }
    }

    /// Insert n blank lines at cursor row, pushing lines down.
    pub fn insert_lines(&mut self, at: usize, n: usize, bottom: usize) {
        for _ in 0..n {
            if at <= bottom {
                // Shift rows down
                for row in (at + 1..=bottom).rev() {
                    for col in 0..self.cols {
                        self.cells[row * self.cols + col] = self.cells[(row - 1) * self.cols + col];
                    }
                }
                for col in 0..self.cols {
                    self.cells[at * self.cols + col] = Cell::default();
                }
            }
        }
    }

    /// Delete n lines at cursor row, pulling lines up.
    pub fn delete_lines(&mut self, at: usize, n: usize, bottom: usize) {
        for _ in 0..n {
            if at <= bottom {
                for row in at..bottom {
                    for col in 0..self.cols {
                        self.cells[row * self.cols + col] = self.cells[(row + 1) * self.cols + col];
                    }
                }
                for col in 0..self.cols {
                    self.cells[bottom * self.cols + col] = Cell::default();
                }
            }
        }
    }

    /// Delete n characters at cursor, shifting remaining left.
    pub fn delete_chars(&mut self, n: usize) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        for c in col..self.cols {
            let src = if c + n < self.cols {
                self.cells[row * self.cols + c + n]
            } else {
                Cell::default()
            };
            self.cells[row * self.cols + c] = src;
        }
    }

    /// Insert n blank characters at cursor, shifting existing right.
    pub fn insert_chars(&mut self, n: usize) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        for c in (col..self.cols).rev() {
            if c >= col + n {
                self.cells[row * self.cols + c] = self.cells[row * self.cols + c - n];
            } else {
                self.cells[row * self.cols + c] = Cell::default();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_row_chars(g: &Grid, row: usize) -> String {
        (0..g.cols()).map(|c| g.cell(row, c).ch).collect::<String>().trim_end().to_string()
    }

    #[test]
    fn test_put_char_and_wrap() {
        let mut g = Grid::new(5, 3);
        for ch in "ABCDE".chars() {
            g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        assert_eq!(g.cursor_col, 5);
        assert_eq!(g.cursor_row, 0);
        // Next char should wrap
        g.put_char('F', CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        assert_eq!(g.cursor_row, 1);
        assert_eq!(g.cursor_col, 1);
        assert_eq!(g.cell(1, 0).ch, 'F');
    }

    #[test]
    fn test_newline_scrolls() {
        let mut g = Grid::new(5, 3);
        g.put_char('A', CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        g.newline();
        g.put_char('B', CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        g.newline();
        g.put_char('C', CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        g.newline(); // should scroll
        assert_eq!(g.cell(0, 0).ch, 'B');
        assert_eq!(g.cell(1, 0).ch, 'C');
        assert_eq!(g.cell(2, 0).ch, ' ');
    }

    #[test]
    fn test_erase_line_right() {
        let mut g = Grid::new(10, 1);
        for ch in "ABCDEFGHIJ".chars() {
            g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        g.cursor_col = 5;
        g.erase_line_right();
        assert_eq!(grid_row_chars(&g, 0), "ABCDE");
    }

    #[test]
    fn test_erase_line_left() {
        let mut g = Grid::new(10, 1);
        for ch in "ABCDEFGHIJ".chars() {
            g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        g.cursor_col = 4;
        g.erase_line_left();
        assert_eq!(g.cell(0, 0).ch, ' ');
        assert_eq!(g.cell(0, 4).ch, ' ');
        assert_eq!(g.cell(0, 5).ch, 'F');
    }

    #[test]
    fn test_erase_below() {
        let mut g = Grid::new(5, 3);
        for r in 0..3 {
            for ch in "XXXXX".chars() {
                g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
            }
            if r < 2 { g.newline(); }
        }
        g.cursor_row = 1;
        g.cursor_col = 2;
        g.erase_below();
        assert_eq!(grid_row_chars(&g, 0), "XXXXX");
        assert_eq!(g.cell(1, 0).ch, 'X');
        assert_eq!(g.cell(1, 1).ch, 'X');
        assert_eq!(g.cell(1, 2).ch, ' '); // erased from cursor
        assert_eq!(g.cell(2, 0).ch, ' '); // row below fully erased
    }

    #[test]
    fn test_scroll_region_up() {
        let mut g = Grid::new(3, 5);
        for (r, s) in ["AAA", "BBB", "CCC", "DDD", "EEE"].iter().enumerate() {
            g.cursor_row = r;
            g.cursor_col = 0;
            for ch in s.chars() {
                g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
            }
        }
        g.scroll_region_up(1, 3); // scroll rows 1-3
        assert_eq!(grid_row_chars(&g, 0), "AAA");
        assert_eq!(grid_row_chars(&g, 1), "CCC");
        assert_eq!(grid_row_chars(&g, 2), "DDD");
        assert_eq!(grid_row_chars(&g, 3), "");    // cleared
        assert_eq!(grid_row_chars(&g, 4), "EEE");
    }

    #[test]
    fn test_scroll_region_down() {
        let mut g = Grid::new(3, 5);
        for (r, s) in ["AAA", "BBB", "CCC", "DDD", "EEE"].iter().enumerate() {
            g.cursor_row = r;
            g.cursor_col = 0;
            for ch in s.chars() {
                g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
            }
        }
        g.scroll_region_down(1, 3);
        assert_eq!(grid_row_chars(&g, 0), "AAA");
        assert_eq!(grid_row_chars(&g, 1), "");    // cleared (new blank line)
        assert_eq!(grid_row_chars(&g, 2), "BBB");
        assert_eq!(grid_row_chars(&g, 3), "CCC");
        assert_eq!(grid_row_chars(&g, 4), "EEE");
    }

    #[test]
    fn test_insert_lines() {
        let mut g = Grid::new(3, 4);
        for (r, s) in ["AAA", "BBB", "CCC", "DDD"].iter().enumerate() {
            g.cursor_row = r;
            g.cursor_col = 0;
            for ch in s.chars() {
                g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
            }
        }
        g.insert_lines(1, 1, 3);
        assert_eq!(grid_row_chars(&g, 0), "AAA");
        assert_eq!(grid_row_chars(&g, 1), "");    // inserted blank
        assert_eq!(grid_row_chars(&g, 2), "BBB");
        assert_eq!(grid_row_chars(&g, 3), "CCC"); // DDD pushed off
    }

    #[test]
    fn test_delete_lines() {
        let mut g = Grid::new(3, 4);
        for (r, s) in ["AAA", "BBB", "CCC", "DDD"].iter().enumerate() {
            g.cursor_row = r;
            g.cursor_col = 0;
            for ch in s.chars() {
                g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
            }
        }
        g.delete_lines(1, 1, 3);
        assert_eq!(grid_row_chars(&g, 0), "AAA");
        assert_eq!(grid_row_chars(&g, 1), "CCC");
        assert_eq!(grid_row_chars(&g, 2), "DDD");
        assert_eq!(grid_row_chars(&g, 3), "");    // blank at bottom
    }

    #[test]
    fn test_delete_chars() {
        let mut g = Grid::new(10, 1);
        for ch in "ABCDEFGHIJ".chars() {
            g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        g.cursor_col = 3;
        g.delete_chars(2); // delete D, E
        assert_eq!(grid_row_chars(&g, 0), "ABCFGHIJ");
    }

    #[test]
    fn test_insert_chars() {
        let mut g = Grid::new(10, 1);
        for ch in "ABCDEFGHIJ".chars() {
            g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        g.cursor_col = 3;
        g.insert_chars(2); // insert 2 blanks at D
        assert_eq!(g.cell(0, 3).ch, ' ');
        assert_eq!(g.cell(0, 4).ch, ' ');
        assert_eq!(g.cell(0, 5).ch, 'D');
    }

    #[test]
    fn test_resize_shrink() {
        let mut g = Grid::new(10, 5);
        for ch in "Hello".chars() {
            g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        g.cursor_row = 3;
        g.cursor_col = 8;
        g.resize(5, 3);
        assert_eq!(g.cols(), 5);
        assert_eq!(g.rows(), 3);
        assert_eq!(g.cursor_row, 2); // clamped
        assert_eq!(g.cursor_col, 4); // clamped
        assert_eq!(grid_row_chars(&g, 0), "Hello");
    }

    #[test]
    fn test_resize_grow() {
        let mut g = Grid::new(5, 3);
        for ch in "Hi".chars() {
            g.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        g.resize(10, 5);
        assert_eq!(g.cols(), 10);
        assert_eq!(g.rows(), 5);
        assert_eq!(g.cell(0, 0).ch, 'H');
        assert_eq!(g.cell(0, 1).ch, 'i');
    }

    #[test]
    fn test_scrollback_saved() {
        let mut g = Grid::new(3, 2);
        g.put_char('A', CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        g.newline();
        g.put_char('B', CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        g.newline(); // scrolls, A goes to scrollback
        assert_eq!(g.scrollback.len(), 1);
        assert_eq!(g.scrollback[0][0].ch, 'A');
    }
}
