/// Glyph atlas: rasterizes glyphs and packs them into a GPU texture.
/// Uses fontdue for rasterization and maintains a cache of glyph positions.

use fontdue::{Font, FontSettings};
use std::collections::HashMap;

/// Position of a glyph within the atlas texture.
#[derive(Debug, Clone, Copy)]
pub struct GlyphEntry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub advance_x: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub struct GlyphAtlas {
    font: Font,
    font_size: f32,
    /// Atlas pixel data (single channel, alpha)
    pub pixels: Vec<u8>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    /// Current packing cursor
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    /// Cached glyph positions
    cache: HashMap<char, GlyphEntry>,
    /// Whether atlas texture needs re-upload to GPU
    pub dirty: bool,
    /// Cell dimensions derived from font metrics
    pub cell_width: f32,
    pub cell_height: f32,
}

impl GlyphAtlas {
    pub fn new(font_data: &[u8], font_size: f32) -> Self {
        let font = Font::from_bytes(font_data, FontSettings::default())
            .expect("Failed to load font");

        // Calculate cell dimensions from font metrics
        let metrics = font.metrics('M', font_size);
        let line_metrics = font.horizontal_line_metrics(font_size);
        let cell_width = metrics.advance_width;
        let cell_height = line_metrics
            .map(|lm| lm.ascent - lm.descent + lm.line_gap)
            .unwrap_or(font_size * 1.2);

        let atlas_width = 1024;
        let atlas_height = 1024;

        Self {
            font,
            font_size,
            pixels: vec![0; (atlas_width * atlas_height) as usize],
            atlas_width,
            atlas_height,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            cache: HashMap::new(),
            dirty: true,
            cell_width,
            cell_height,
        }
    }

    /// Get or rasterize a glyph, returning its atlas entry.
    pub fn get_glyph(&mut self, ch: char) -> GlyphEntry {
        if let Some(&entry) = self.cache.get(&ch) {
            return entry;
        }
        self.rasterize(ch)
    }

    fn rasterize(&mut self, ch: char) -> GlyphEntry {
        let (metrics, bitmap) = self.font.rasterize(ch, self.font_size);

        let w = metrics.width as u32;
        let h = metrics.height as u32;

        // Simple row-based packing
        if self.cursor_x + w + 1 > self.atlas_width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height + 1;
            self.row_height = 0;
        }

        if self.cursor_y + h > self.atlas_height {
            // Atlas full — in production, would resize or use multiple atlases
            log::warn!("Glyph atlas full, cannot rasterize '{}'", ch);
            let entry = GlyphEntry {
                x: 0, y: 0, width: 0, height: 0,
                advance_x: metrics.advance_width,
                offset_x: 0.0, offset_y: 0.0,
            };
            self.cache.insert(ch, entry);
            return entry;
        }

        // Copy bitmap into atlas
        for row in 0..h {
            for col in 0..w {
                let src = bitmap[(row * w + col) as usize];
                let dst_x = self.cursor_x + col;
                let dst_y = self.cursor_y + row;
                self.pixels[(dst_y * self.atlas_width + dst_x) as usize] = src;
            }
        }

        let entry = GlyphEntry {
            x: self.cursor_x,
            y: self.cursor_y,
            width: w,
            height: h,
            advance_x: metrics.advance_width,
            offset_x: metrics.xmin as f32,
            offset_y: metrics.ymin as f32,
        };

        self.cursor_x += w + 1;
        self.row_height = self.row_height.max(h);
        self.dirty = true;
        self.cache.insert(ch, entry);
        entry
    }

    pub fn glyph_count(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use a minimal embedded font for testing
    fn test_font() -> Vec<u8> {
        // fontdue can parse TTF/OTF. We'll use the system font path for tests.
        // For CI, we'd bundle a test font. For now, test the API structure.
        include_bytes!("/System/Library/Fonts/Menlo.ttc").to_vec()
    }

    #[test]
    fn test_atlas_creation() {
        let font_data = test_font();
        let atlas = GlyphAtlas::new(&font_data, 14.0);
        assert!(atlas.cell_width > 0.0);
        assert!(atlas.cell_height > 0.0);
        assert_eq!(atlas.atlas_width, 1024);
        assert_eq!(atlas.glyph_count(), 0);
    }

    #[test]
    fn test_glyph_rasterization() {
        let font_data = test_font();
        let mut atlas = GlyphAtlas::new(&font_data, 14.0);
        let entry = atlas.get_glyph('A');
        assert!(entry.width > 0);
        assert!(entry.height > 0);
        assert!(entry.advance_x > 0.0);
        assert_eq!(atlas.glyph_count(), 1);
    }

    #[test]
    fn test_glyph_caching() {
        let font_data = test_font();
        let mut atlas = GlyphAtlas::new(&font_data, 14.0);
        let e1 = atlas.get_glyph('B');
        let e2 = atlas.get_glyph('B');
        assert_eq!(e1.x, e2.x);
        assert_eq!(e1.y, e2.y);
        assert_eq!(atlas.glyph_count(), 1); // cached, not re-rasterized
    }

    #[test]
    fn test_multiple_glyphs_packed() {
        let font_data = test_font();
        let mut atlas = GlyphAtlas::new(&font_data, 14.0);
        for ch in 'A'..='Z' {
            atlas.get_glyph(ch);
        }
        assert_eq!(atlas.glyph_count(), 26);
        // All glyphs should have unique positions
        let positions: Vec<(u32, u32)> = ('A'..='Z')
            .map(|ch| { let e = atlas.get_glyph(ch); (e.x, e.y) })
            .collect();
        for i in 0..positions.len() {
            for j in (i+1)..positions.len() {
                assert_ne!(positions[i], positions[j], "Glyphs overlap");
            }
        }
    }

    #[test]
    fn test_cjk_glyph() {
        let font_data = test_font();
        let mut atlas = GlyphAtlas::new(&font_data, 14.0);
        let entry = atlas.get_glyph('中');
        // CJK glyphs should be wider
        assert!(entry.width > 0);
    }
}
