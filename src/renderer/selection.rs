/// Text selection: tracks selection range and generates highlight vertices.

use crate::core::Grid;
use crate::renderer::pipeline::CellVertex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Normal,
    Word,
    Line,
}

pub struct Selection {
    pub start: Option<SelectionPoint>,
    pub end: Option<SelectionPoint>,
    pub mode: SelectionMode,
    pub active: bool,
}

impl Selection {
    pub fn new() -> Self {
        Self { start: None, end: None, mode: SelectionMode::Normal, active: false }
    }

    pub fn begin(&mut self, row: usize, col: usize, mode: SelectionMode) {
        self.start = Some(SelectionPoint { row, col });
        self.end = Some(SelectionPoint { row, col });
        self.mode = mode;
        self.active = true;
    }

    pub fn update(&mut self, row: usize, col: usize) {
        if self.active {
            self.end = Some(SelectionPoint { row, col });
        }
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
        self.active = false;
    }

    /// Returns (start, end) normalized so start <= end.
    fn normalized(&self) -> Option<(SelectionPoint, SelectionPoint)> {
        let (s, e) = (self.start?, self.end?);
        if s.row < e.row || (s.row == e.row && s.col <= e.col) {
            Some((s, e))
        } else {
            Some((e, s))
        }
    }

    /// Check if a cell is within the selection.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        let Some((start, end)) = self.normalized() else { return false };
        match self.mode {
            SelectionMode::Line => row >= start.row && row <= end.row,
            SelectionMode::Normal | SelectionMode::Word => {
                if row < start.row || row > end.row { return false; }
                if start.row == end.row {
                    col >= start.col && col <= end.col
                } else if row == start.row {
                    col >= start.col
                } else if row == end.row {
                    col <= end.col
                } else {
                    true
                }
            }
        }
    }

    /// Extract selected text from grid.
    pub fn get_text(&self, grid: &Grid) -> String {
        let Some((start, end)) = self.normalized() else { return String::new() };
        let mut text = String::new();

        for row in start.row..=end.row.min(grid.rows() - 1) {
            let col_start = if row == start.row { start.col } else { 0 };
            let col_end = if row == end.row { end.col } else { grid.cols() - 1 };

            for col in col_start..=col_end.min(grid.cols() - 1) {
                let ch = grid.cell(row, col).ch;
                if ch != '\0' {
                    text.push(ch);
                }
            }
            if row < end.row {
                // Trim trailing spaces and add newline
                let trimmed = text.trim_end();
                text = trimmed.to_string();
                text.push('\n');
            }
        }
        text
    }

    /// Generate highlight overlay vertices for selected cells.
    pub fn build_vertices(
        &self,
        grid: &Grid,
        cell_width: f32,
        cell_height: f32,
        screen_width: f32,
        screen_height: f32,
    ) -> (Vec<CellVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        if !self.active { return (vertices, indices); }

        let highlight = [0.3, 0.5, 0.8]; // selection blue

        for row in 0..grid.rows() {
            for col in 0..grid.cols() {
                if !self.contains(row, col) { continue; }

                let x0 = col as f32 * cell_width;
                let y0 = row as f32 * cell_height;
                let nx0 = (x0 / screen_width) * 2.0 - 1.0;
                let ny0 = 1.0 - (y0 / screen_height) * 2.0;
                let nx1 = ((x0 + cell_width) / screen_width) * 2.0 - 1.0;
                let ny1 = 1.0 - ((y0 + cell_height) / screen_height) * 2.0;

                let base = vertices.len() as u32;
                let v = CellVertex {
                    position: [0.0; 2], uv: [0.0; 2],
                    fg_color: highlight, bg_color: highlight,
                };
                vertices.extend_from_slice(&[
                    CellVertex { position: [nx0, ny0], ..v },
                    CellVertex { position: [nx1, ny0], ..v },
                    CellVertex { position: [nx1, ny1], ..v },
                    CellVertex { position: [nx0, ny1], ..v },
                ]);
                indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
            }
        }

        (vertices, indices)
    }
}

impl Default for Selection {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Grid, CellAttr, Color};

    #[test]
    fn test_selection_empty() {
        let s = Selection::new();
        assert!(!s.active);
        assert!(!s.contains(0, 0));
    }

    #[test]
    fn test_selection_single_line() {
        let mut s = Selection::new();
        s.begin(0, 2, SelectionMode::Normal);
        s.update(0, 7);
        assert!(s.contains(0, 2));
        assert!(s.contains(0, 5));
        assert!(s.contains(0, 7));
        assert!(!s.contains(0, 1));
        assert!(!s.contains(0, 8));
        assert!(!s.contains(1, 5));
    }

    #[test]
    fn test_selection_multi_line() {
        let mut s = Selection::new();
        s.begin(1, 5, SelectionMode::Normal);
        s.update(3, 3);
        // Row 1: col 5+
        assert!(!s.contains(1, 4));
        assert!(s.contains(1, 5));
        assert!(s.contains(1, 79));
        // Row 2: all
        assert!(s.contains(2, 0));
        assert!(s.contains(2, 50));
        // Row 3: up to col 3
        assert!(s.contains(3, 0));
        assert!(s.contains(3, 3));
        assert!(!s.contains(3, 4));
    }

    #[test]
    fn test_selection_reversed() {
        let mut s = Selection::new();
        s.begin(3, 5, SelectionMode::Normal);
        s.update(1, 2); // drag upward
        assert!(s.contains(1, 2));
        assert!(s.contains(2, 0));
        assert!(s.contains(3, 5));
        assert!(!s.contains(3, 6));
    }

    #[test]
    fn test_selection_line_mode() {
        let mut s = Selection::new();
        s.begin(2, 5, SelectionMode::Line);
        s.update(4, 0);
        assert!(!s.contains(1, 0));
        assert!(s.contains(2, 0)); // entire line
        assert!(s.contains(3, 50));
        assert!(s.contains(4, 79));
        assert!(!s.contains(5, 0));
    }

    #[test]
    fn test_selection_clear() {
        let mut s = Selection::new();
        s.begin(0, 0, SelectionMode::Normal);
        s.update(5, 5);
        s.clear();
        assert!(!s.active);
        assert!(!s.contains(0, 0));
    }

    #[test]
    fn test_get_text() {
        let mut grid = Grid::new(10, 3);
        for (i, ch) in "Hello".chars().enumerate() {
            grid.cursor_col = i;
            grid.cursor_row = 0;
            grid.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        for (i, ch) in "World".chars().enumerate() {
            grid.cursor_col = i;
            grid.cursor_row = 1;
            grid.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }

        let mut s = Selection::new();
        s.begin(0, 0, SelectionMode::Normal);
        s.update(0, 4);
        assert_eq!(s.get_text(&grid), "Hello");
    }

    #[test]
    fn test_get_text_multiline() {
        let mut grid = Grid::new(10, 3);
        for (i, ch) in "Hello".chars().enumerate() {
            grid.cursor_col = i;
            grid.cursor_row = 0;
            grid.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }
        for (i, ch) in "World".chars().enumerate() {
            grid.cursor_col = i;
            grid.cursor_row = 1;
            grid.put_char(ch, CellAttr::empty(), Color::DEFAULT_FG, Color::DEFAULT_BG);
        }

        let mut s = Selection::new();
        s.begin(0, 0, SelectionMode::Normal);
        s.update(1, 4);
        assert_eq!(s.get_text(&grid), "Hello\nWorld");
    }

    #[test]
    fn test_build_vertices_inactive() {
        let s = Selection::new();
        let grid = Grid::new(10, 5);
        let (v, i) = s.build_vertices(&grid, 8.0, 16.0, 640.0, 480.0);
        assert!(v.is_empty());
        assert!(i.is_empty());
    }

    #[test]
    fn test_build_vertices_active() {
        let mut s = Selection::new();
        s.begin(0, 0, SelectionMode::Normal);
        s.update(0, 2);
        let grid = Grid::new(10, 5);
        let (v, i) = s.build_vertices(&grid, 8.0, 16.0, 640.0, 480.0);
        assert_eq!(v.len(), 12); // 3 cells × 4 vertices
        assert_eq!(i.len(), 18); // 3 cells × 6 indices
    }
}
