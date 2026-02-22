/// Theme system: bundled color schemes + custom themes via TOML.

use crate::core::Color;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub fg: Color,
    pub bg: Color,
    pub cursor: Color,
    pub ansi: [Color; 16],
}

#[derive(Debug, Deserialize)]
struct ThemeToml {
    name: Option<String>,
    foreground: Option<String>,
    background: Option<String>,
    cursor: Option<String>,
    ansi: Option<Vec<String>>,
}

fn hex_to_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 { return None; }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color { r, g, b })
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            name: "default".into(),
            fg: Color { r: 204, g: 204, b: 204 },
            bg: Color { r: 0, g: 0, b: 0 },
            cursor: Color { r: 204, g: 204, b: 204 },
            ansi: DEFAULT_ANSI,
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "dracula".into(),
            fg: Color { r: 248, g: 248, b: 242 },
            bg: Color { r: 40, g: 42, b: 54 },
            cursor: Color { r: 248, g: 248, b: 242 },
            ansi: [
                Color { r: 33, g: 34, b: 44 },    // black
                Color { r: 255, g: 85, b: 85 },    // red
                Color { r: 80, g: 250, b: 123 },   // green
                Color { r: 241, g: 250, b: 140 },  // yellow
                Color { r: 98, g: 114, b: 164 },   // blue
                Color { r: 255, g: 121, b: 198 },  // magenta
                Color { r: 139, g: 233, b: 253 },  // cyan
                Color { r: 248, g: 248, b: 242 },  // white
                Color { r: 98, g: 114, b: 164 },   // bright black
                Color { r: 255, g: 110, b: 110 },  // bright red
                Color { r: 105, g: 255, b: 148 },  // bright green
                Color { r: 255, g: 255, b: 165 },  // bright yellow
                Color { r: 125, g: 141, b: 191 },  // bright blue
                Color { r: 255, g: 146, b: 215 },  // bright magenta
                Color { r: 164, g: 248, b: 255 },  // bright cyan
                Color { r: 255, g: 255, b: 255 },  // bright white
            ],
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            name: "solarized-dark".into(),
            fg: Color { r: 131, g: 148, b: 150 },
            bg: Color { r: 0, g: 43, b: 54 },
            cursor: Color { r: 131, g: 148, b: 150 },
            ansi: [
                Color { r: 7, g: 54, b: 66 },
                Color { r: 220, g: 50, b: 47 },
                Color { r: 133, g: 153, b: 0 },
                Color { r: 181, g: 137, b: 0 },
                Color { r: 38, g: 139, b: 210 },
                Color { r: 211, g: 54, b: 130 },
                Color { r: 42, g: 161, b: 152 },
                Color { r: 238, g: 232, b: 213 },
                Color { r: 0, g: 43, b: 54 },
                Color { r: 203, g: 75, b: 22 },
                Color { r: 88, g: 110, b: 117 },
                Color { r: 101, g: 123, b: 131 },
                Color { r: 131, g: 148, b: 150 },
                Color { r: 108, g: 113, b: 196 },
                Color { r: 147, g: 161, b: 161 },
                Color { r: 253, g: 246, b: 227 },
            ],
        }
    }

    /// Look up a bundled theme by name.
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::default_dark()),
            "dracula" => Some(Self::dracula()),
            "solarized-dark" => Some(Self::solarized_dark()),
            _ => None,
        }
    }

    /// Parse a custom theme from TOML string.
    pub fn from_toml(s: &str) -> Option<Self> {
        let t: ThemeToml = toml::from_str(s).ok()?;
        let base = Self::default_dark();
        let fg = t.foreground.as_deref().and_then(hex_to_color).unwrap_or(base.fg);
        let bg = t.background.as_deref().and_then(hex_to_color).unwrap_or(base.bg);
        let cursor = t.cursor.as_deref().and_then(hex_to_color).unwrap_or(fg);
        let mut ansi = base.ansi;
        if let Some(colors) = &t.ansi {
            for (i, hex) in colors.iter().enumerate().take(16) {
                if let Some(c) = hex_to_color(hex) {
                    ansi[i] = c;
                }
            }
        }
        Some(Self {
            name: t.name.unwrap_or_else(|| "custom".into()),
            fg, bg, cursor, ansi,
        })
    }

    /// List all bundled theme names.
    pub fn bundled_names() -> &'static [&'static str] {
        &["default", "dracula", "solarized-dark"]
    }
}

const DEFAULT_ANSI: [Color; 16] = [
    Color { r: 0,   g: 0,   b: 0   },
    Color { r: 205, g: 49,  b: 49  },
    Color { r: 13,  g: 188, b: 121 },
    Color { r: 229, g: 229, b: 16  },
    Color { r: 36,  g: 114, b: 200 },
    Color { r: 188, g: 63,  b: 188 },
    Color { r: 17,  g: 168, b: 205 },
    Color { r: 204, g: 204, b: 204 },
    Color { r: 102, g: 102, b: 102 },
    Color { r: 241, g: 76,  b: 76  },
    Color { r: 35,  g: 209, b: 139 },
    Color { r: 245, g: 245, b: 67  },
    Color { r: 59,  g: 142, b: 234 },
    Color { r: 214, g: 112, b: 214 },
    Color { r: 41,  g: 184, b: 219 },
    Color { r: 242, g: 242, b: 242 },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_color() {
        assert_eq!(hex_to_color("#ff0000"), Some(Color { r: 255, g: 0, b: 0 }));
        assert_eq!(hex_to_color("00ff00"), Some(Color { r: 0, g: 255, b: 0 }));
        assert_eq!(hex_to_color("#zzzzzz"), None);
        assert_eq!(hex_to_color("#fff"), None); // too short
    }

    #[test]
    fn test_default_theme() {
        let t = Theme::default_dark();
        assert_eq!(t.name, "default");
        assert_eq!(t.bg, Color { r: 0, g: 0, b: 0 });
    }

    #[test]
    fn test_bundled_themes() {
        for name in Theme::bundled_names() {
            let t = Theme::by_name(name);
            assert!(t.is_some(), "missing theme: {}", name);
        }
        assert!(Theme::by_name("nonexistent").is_none());
    }

    #[test]
    fn test_dracula_theme() {
        let t = Theme::dracula();
        assert_eq!(t.name, "dracula");
        assert_eq!(t.bg, Color { r: 40, g: 42, b: 54 });
        assert_eq!(t.fg, Color { r: 248, g: 248, b: 242 });
    }

    #[test]
    fn test_custom_theme_from_toml() {
        let t = Theme::from_toml(r##"
            name = "my-theme"
            foreground = "#e0e0e0"
            background = "#1a1a2e"
            cursor = "#ffffff"
            ansi = [
                "#000000", "#ff0000", "#00ff00", "#ffff00",
                "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
                "#808080", "#ff8080", "#80ff80", "#ffff80",
                "#8080ff", "#ff80ff", "#80ffff", "#ffffff"
            ]
        "##).unwrap();
        assert_eq!(t.name, "my-theme");
        assert_eq!(t.bg, Color { r: 26, g: 26, b: 46 });
        assert_eq!(t.ansi[1], Color { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn test_partial_custom_theme() {
        let t = Theme::from_toml(r##"
            background = "#112233"
        "##).unwrap();
        assert_eq!(t.bg, Color { r: 17, g: 34, b: 51 });
        // fg falls back to default
        assert_eq!(t.fg, Color { r: 204, g: 204, b: 204 });
    }

    #[test]
    fn test_invalid_theme_toml() {
        assert!(Theme::from_toml("{{invalid}}").is_none());
    }
}
