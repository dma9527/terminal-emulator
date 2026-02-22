/// Configuration system: TOML-based with sensible defaults.
/// Config file: `~/.config/term/config.toml`

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub window: WindowConfig,
    pub colors: ColorConfig,
    pub shell: ShellConfig,
    pub scrollback: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub opacity: f32,
    pub padding: u32,
    pub decorations: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub theme: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub program: String,
    pub args: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            window: WindowConfig::default(),
            colors: ColorConfig::default(),
            shell: ShellConfig::default(),
            scrollback: 10_000,
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "Menlo".into(),
            size: 14.0,
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            opacity: 1.0,
            padding: 4,
            decorations: true,
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            foreground: "#cccccc".into(),
            background: "#000000".into(),
            cursor: "#cccccc".into(),
            theme: "default".into(),
        }
    }
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            program: std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()),
            args: vec!["--login".into()],
        }
    }
}

impl Config {
    /// Config file path: `~/.config/term/config.toml`
    pub fn path() -> PathBuf {
        dirs_path().join("config.toml")
    }

    /// Load config from file, falling back to defaults.
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => Self::from_str(&contents),
            Err(_) => Self::default(),
        }
    }

    /// Parse config from TOML string.
    pub fn from_str(s: &str) -> Self {
        toml::from_str(s).unwrap_or_default()
    }
}

fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("term")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.font.family, "Menlo");
        assert_eq!(cfg.font.size, 14.0);
        assert_eq!(cfg.scrollback, 10_000);
        assert_eq!(cfg.window.opacity, 1.0);
        assert_eq!(cfg.colors.background, "#000000");
    }

    #[test]
    fn test_parse_empty_toml() {
        let cfg = Config::from_str("");
        assert_eq!(cfg.font.family, "Menlo");
        assert_eq!(cfg.scrollback, 10_000);
    }

    #[test]
    fn test_parse_partial_toml() {
        let cfg = Config::from_str(r#"
            scrollback = 5000

            [font]
            family = "JetBrains Mono"
            size = 16.0
        "#);
        assert_eq!(cfg.font.family, "JetBrains Mono");
        assert_eq!(cfg.font.size, 16.0);
        assert_eq!(cfg.scrollback, 5000);
        // Defaults preserved for unset fields
        assert_eq!(cfg.window.width, 800);
    }

    #[test]
    fn test_parse_full_toml() {
        let cfg = Config::from_str(r##"
            scrollback = 20000

            [font]
            family = "Fira Code"
            size = 13.0

            [window]
            width = 1024
            height = 768
            opacity = 0.95
            padding = 8
            decorations = false

            [colors]
            foreground = "#e0e0e0"
            background = "#1a1a2e"
            cursor = "#ffffff"
            theme = "dracula"

            [shell]
            program = "/bin/bash"
            args = ["-l"]
        "##);
        assert_eq!(cfg.font.family, "Fira Code");
        assert_eq!(cfg.window.opacity, 0.95);
        assert!(!cfg.window.decorations);
        assert_eq!(cfg.colors.theme, "dracula");
        assert_eq!(cfg.shell.program, "/bin/bash");
        assert_eq!(cfg.scrollback, 20000);
    }

    #[test]
    fn test_invalid_toml_falls_back() {
        let cfg = Config::from_str("this is not valid toml {{{}}}");
        // Should fall back to defaults
        assert_eq!(cfg.font.family, "Menlo");
    }

    #[test]
    fn test_config_path() {
        let path = Config::path();
        assert!(path.to_str().unwrap().ends_with(".config/term/config.toml"));
    }
}
