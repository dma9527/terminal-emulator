/// Cursor rendering: block, beam, underline styles with blink support.

use crate::renderer::pipeline::CellVertex;
use crate::core::Color;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Beam,
    Underline,
}

pub struct Cursor {
    pub style: CursorStyle,
    pub visible: bool,
    pub blink: bool,
    blink_start: Instant,
    blink_interval_ms: u64,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            style: CursorStyle::Block,
            visible: true,
            blink: true,
            blink_start: Instant::now(),
            blink_interval_ms: 530,
        }
    }

    /// Returns true if cursor should be drawn this frame.
    pub fn is_visible_now(&self) -> bool {
        if !self.visible {
            return false;
        }
        if !self.blink {
            return true;
        }
        let elapsed = self.blink_start.elapsed().as_millis() as u64;
        (elapsed / self.blink_interval_ms) % 2 == 0
    }

    /// Reset blink timer (e.g., on keypress).
    pub fn reset_blink(&mut self) {
        self.blink_start = Instant::now();
    }

    /// Generate vertices for the cursor at the given grid position.
    pub fn build_vertices(
        &self,
        cursor_row: usize,
        cursor_col: usize,
        cell_width: f32,
        cell_height: f32,
        screen_width: f32,
        screen_height: f32,
        color: Color,
    ) -> Vec<CellVertex> {
        if !self.is_visible_now() {
            return Vec::new();
        }

        let x0 = cursor_col as f32 * cell_width;
        let y0 = cursor_row as f32 * cell_height;

        let (w, h) = match self.style {
            CursorStyle::Block => (cell_width, cell_height),
            CursorStyle::Beam => (2.0, cell_height),
            CursorStyle::Underline => (cell_width, 2.0),
        };

        let (x0, y0) = match self.style {
            CursorStyle::Underline => (x0, y0 + cell_height - 2.0),
            _ => (x0, y0),
        };

        let nx0 = (x0 / screen_width) * 2.0 - 1.0;
        let ny0 = 1.0 - (y0 / screen_height) * 2.0;
        let nx1 = ((x0 + w) / screen_width) * 2.0 - 1.0;
        let ny1 = 1.0 - ((y0 + h) / screen_height) * 2.0;

        let fg = [color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0];
        // Use UV (0,0) — solid fill, atlas pixel at (0,0) should be opaque for cursor
        let uv = [0.0, 0.0];

        vec![
            CellVertex { position: [nx0, ny0], uv, fg_color: fg, bg_color: fg },
            CellVertex { position: [nx1, ny0], uv, fg_color: fg, bg_color: fg },
            CellVertex { position: [nx1, ny1], uv, fg_color: fg, bg_color: fg },
            CellVertex { position: [nx0, ny1], uv, fg_color: fg, bg_color: fg },
        ]
    }
}

impl Default for Cursor {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_default() {
        let c = Cursor::new();
        assert_eq!(c.style, CursorStyle::Block);
        assert!(c.visible);
        assert!(c.blink);
    }

    #[test]
    fn test_cursor_visible_initially() {
        let c = Cursor::new();
        assert!(c.is_visible_now());
    }

    #[test]
    fn test_cursor_hidden() {
        let mut c = Cursor::new();
        c.visible = false;
        assert!(!c.is_visible_now());
    }

    #[test]
    fn test_cursor_no_blink_always_visible() {
        let mut c = Cursor::new();
        c.blink = false;
        assert!(c.is_visible_now());
    }

    #[test]
    fn test_block_cursor_vertices() {
        let c = Cursor::new();
        let verts = c.build_vertices(0, 0, 8.0, 16.0, 640.0, 480.0,
            Color { r: 255, g: 255, b: 255 });
        assert_eq!(verts.len(), 4);
    }

    #[test]
    fn test_beam_cursor_narrow() {
        let mut c = Cursor::new();
        c.style = CursorStyle::Beam;
        let verts = c.build_vertices(0, 5, 8.0, 16.0, 640.0, 480.0,
            Color { r: 255, g: 255, b: 255 });
        assert_eq!(verts.len(), 4);
        // Beam should be narrow: x1 - x0 ≈ 2px in NDC
        let width_ndc = verts[1].position[0] - verts[0].position[0];
        let cell_width_ndc = (8.0 / 640.0) * 2.0;
        assert!(width_ndc < cell_width_ndc); // beam is narrower than cell
    }

    #[test]
    fn test_underline_cursor_at_bottom() {
        let mut c = Cursor::new();
        c.style = CursorStyle::Underline;
        let verts = c.build_vertices(0, 0, 8.0, 16.0, 640.0, 480.0,
            Color { r: 255, g: 255, b: 255 });
        assert_eq!(verts.len(), 4);
        // Underline y should be near bottom of cell
        let top_y = verts[0].position[1];
        let bottom_y = verts[2].position[1];
        assert!(top_y > bottom_y); // NDC: top > bottom
    }

    #[test]
    fn test_reset_blink() {
        let mut c = Cursor::new();
        c.reset_blink();
        assert!(c.is_visible_now()); // just reset, should be visible
    }
}
