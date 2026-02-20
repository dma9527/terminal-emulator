/// UTF-8 streaming decoder. Handles bytes arriving across chunk boundaries.

pub struct Utf8Decoder {
    buf: [u8; 4],
    len: u8,
    expected: u8,
}

impl Utf8Decoder {
    pub fn new() -> Self {
        Self { buf: [0; 4], len: 0, expected: 0 }
    }

    /// Feed one byte. Returns Some(char) when a complete codepoint is decoded.
    /// Returns None if more bytes are needed.
    /// Returns Some(REPLACEMENT_CHARACTER) on invalid sequences.
    pub fn feed(&mut self, byte: u8) -> Option<char> {
        if self.expected == 0 {
            // Start of new sequence
            if byte < 0x80 {
                return Some(byte as char);
            } else if byte & 0xE0 == 0xC0 {
                self.expected = 2;
            } else if byte & 0xF0 == 0xE0 {
                self.expected = 3;
            } else if byte & 0xF8 == 0xF0 {
                self.expected = 4;
            } else {
                // Invalid lead byte or unexpected continuation
                return Some(char::REPLACEMENT_CHARACTER);
            }
            self.buf[0] = byte;
            self.len = 1;
            None
        } else if byte & 0xC0 == 0x80 {
            // Valid continuation byte
            self.buf[self.len as usize] = byte;
            self.len += 1;
            if self.len == self.expected {
                let result = std::str::from_utf8(&self.buf[..self.len as usize])
                    .ok()
                    .and_then(|s| s.chars().next())
                    .unwrap_or(char::REPLACEMENT_CHARACTER);
                self.expected = 0;
                self.len = 0;
                Some(result)
            } else {
                None
            }
        } else {
            // Expected continuation but got something else — invalid
            self.expected = 0;
            self.len = 0;
            // Re-feed this byte as a new sequence start
            let replacement = Some(char::REPLACEMENT_CHARACTER);
            // If this byte is a valid lead, start new sequence
            if byte >= 0x80 {
                let _ = self.feed(byte); // queue it up
            }
            replacement
        }
    }

    /// Check if decoder is mid-sequence.
    pub fn is_pending(&self) -> bool {
        self.expected > 0
    }
}

impl Default for Utf8Decoder {
    fn default() -> Self { Self::new() }
}

/// Returns the display width of a character (0, 1, or 2 for CJK).
pub fn char_width(ch: char) -> usize {
    let cp = ch as u32;
    match cp {
        // C0/C1 controls, DEL
        0x00..=0x1f | 0x7f..=0x9f => 0,
        // Combining marks (common ranges)
        0x0300..=0x036f | 0x0483..=0x0489 | 0x0591..=0x05bd |
        0x05bf | 0x05c1..=0x05c2 | 0x05c4..=0x05c5 | 0x05c7 |
        0x0610..=0x061a | 0x064b..=0x065f | 0x0670 |
        0xfe00..=0xfe0f |  // Variation selectors
        0x200b..=0x200f |  // Zero-width spaces
        0x2028..=0x202e |  // Bidi controls
        0x2060..=0x2064 |  // Word joiner etc
        0xfeff => 0,       // BOM / ZWNBSP
        // CJK wide characters
        0x1100..=0x115f |  // Hangul Jamo
        0x2e80..=0x303e |  // CJK Radicals, Kangxi, CJK Symbols
        0x3041..=0x33bf |  // Hiragana, Katakana, Bopomofo, Hangul Compat, Kanbun, CJK
        0x3400..=0x4dbf |  // CJK Unified Ext A
        0x4e00..=0xa4cf |  // CJK Unified, Yi
        0xa960..=0xa97c |  // Hangul Jamo Extended-A
        0xac00..=0xd7a3 |  // Hangul Syllables
        0xf900..=0xfaff |  // CJK Compatibility Ideographs
        0xfe30..=0xfe6f |  // CJK Compatibility Forms, Small Form Variants
        0xff01..=0xff60 |  // Fullwidth Forms
        0xffe0..=0xffe6 |  // Fullwidth Signs
        0x20000..=0x2fffd | // CJK Unified Ext B-F
        0x30000..=0x3fffd => 2, // CJK Unified Ext G+
        // Emoji that are typically wide
        0x1f300..=0x1f9ff | 0x1fa00..=0x1fa6f | 0x1fa70..=0x1faff => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii() {
        let mut d = Utf8Decoder::new();
        assert_eq!(d.feed(b'A'), Some('A'));
        assert_eq!(d.feed(b'z'), Some('z'));
    }

    #[test]
    fn test_two_byte() {
        let mut d = Utf8Decoder::new();
        // é = 0xC3 0xA9
        assert_eq!(d.feed(0xC3), None);
        assert_eq!(d.feed(0xA9), Some('é'));
    }

    #[test]
    fn test_three_byte_cjk() {
        let mut d = Utf8Decoder::new();
        // 中 = 0xE4 0xB8 0xAD
        assert_eq!(d.feed(0xE4), None);
        assert_eq!(d.feed(0xB8), None);
        assert_eq!(d.feed(0xAD), Some('中'));
    }

    #[test]
    fn test_four_byte_emoji() {
        let mut d = Utf8Decoder::new();
        // 😀 = 0xF0 0x9F 0x98 0x80
        assert_eq!(d.feed(0xF0), None);
        assert_eq!(d.feed(0x9F), None);
        assert_eq!(d.feed(0x98), None);
        assert_eq!(d.feed(0x80), Some('😀'));
    }

    #[test]
    fn test_invalid_continuation() {
        let mut d = Utf8Decoder::new();
        assert_eq!(d.feed(0xC3), None);
        // Feed a non-continuation byte
        assert_eq!(d.feed(b'A'), Some(char::REPLACEMENT_CHARACTER));
    }

    #[test]
    fn test_char_width() {
        assert_eq!(char_width('A'), 1);
        assert_eq!(char_width('中'), 2);
        assert_eq!(char_width('é'), 1);
        assert_eq!(char_width('😀'), 2);
        assert_eq!(char_width('\0'), 0);
    }
}
