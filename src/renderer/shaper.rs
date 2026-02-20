/// Font shaping via harfbuzz: handles ligatures and complex text layout.
/// Shapes a run of text and returns positioned glyph IDs.

use harfbuzz_rs::{Face, Font as HbFont, UnicodeBuffer, shape, Owned};

pub struct FontShaper {
    face: Owned<Face<'static>>,
    font: Owned<HbFont<'static>>,
    font_size: f32,
}

#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub codepoint: u32,
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

impl FontShaper {
    pub fn new(font_data: &'static [u8], font_size: f32) -> Self {
        let face = Face::from_bytes(font_data, 0);
        let mut font = HbFont::new(Face::from_bytes(font_data, 0));
        font.set_scale(
            (font_size * 64.0) as i32,
            (font_size * 64.0) as i32,
        );
        Self { face, font, font_size }
    }

    /// Shape a string and return positioned glyphs.
    pub fn shape_text(&self, text: &str) -> Vec<ShapedGlyph> {
        if text.is_empty() {
            return Vec::new();
        }
        let buffer = UnicodeBuffer::new().add_str(text);
        let output = shape(&self.font, buffer, &[]);

        let positions = output.get_glyph_positions();
        let infos = output.get_glyph_infos();

        infos.iter().zip(positions.iter()).map(|(info, pos)| {
            ShapedGlyph {
                codepoint: info.codepoint,
                cluster: info.cluster,
                x_advance: pos.x_advance,
                y_advance: pos.y_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
            }
        }).collect()
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FONT_DATA: &[u8] = include_bytes!("/System/Library/Fonts/Menlo.ttc");

    #[test]
    fn test_shaper_creation() {
        let shaper = FontShaper::new(FONT_DATA, 14.0);
        assert_eq!(shaper.font_size(), 14.0);
    }

    #[test]
    fn test_shape_ascii() {
        let shaper = FontShaper::new(FONT_DATA, 14.0);
        let glyphs = shaper.shape_text("Hello");
        assert_eq!(glyphs.len(), 5);
        // Each glyph should have positive advance
        for g in &glyphs {
            assert!(g.x_advance > 0, "glyph advance should be positive");
        }
    }

    #[test]
    fn test_shape_empty() {
        let shaper = FontShaper::new(FONT_DATA, 14.0);
        let glyphs = shaper.shape_text("");
        assert!(glyphs.is_empty());
    }

    #[test]
    fn test_shape_cjk() {
        let shaper = FontShaper::new(FONT_DATA, 14.0);
        let glyphs = shaper.shape_text("中文");
        assert_eq!(glyphs.len(), 2);
    }

    #[test]
    fn test_cluster_indices() {
        let shaper = FontShaper::new(FONT_DATA, 14.0);
        let glyphs = shaper.shape_text("ABC");
        // Clusters should be sequential for simple text
        assert_eq!(glyphs[0].cluster, 0);
        assert_eq!(glyphs[1].cluster, 1);
        assert_eq!(glyphs[2].cluster, 2);
    }

    #[test]
    fn test_monospace_equal_advance() {
        let shaper = FontShaper::new(FONT_DATA, 14.0);
        let glyphs = shaper.shape_text("ABCDEF");
        // Menlo is monospace — all advances should be equal
        let first_advance = glyphs[0].x_advance;
        for g in &glyphs {
            assert_eq!(g.x_advance, first_advance, "monospace font should have equal advances");
        }
    }
}
