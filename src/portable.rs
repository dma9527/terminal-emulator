/// Portable config: export/import config + keybindings + themes as a single bundle.
/// Enables "same experience everywhere" across macOS/Linux machines.

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBundle {
    pub version: u32,
    pub config_toml: String,
    pub theme_toml: Option<String>,
    pub keybindings: Vec<KeybindingEntry>,
    pub shell_scripts: ShellScripts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingEntry {
    pub modifiers: Vec<String>,
    pub key: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShellScripts {
    pub bash: Option<String>,
    pub zsh: Option<String>,
    pub fish: Option<String>,
}

impl ConfigBundle {
    pub fn new(config_toml: &str) -> Self {
        Self {
            version: 1,
            config_toml: config_toml.into(),
            theme_toml: None,
            keybindings: Vec::new(),
            shell_scripts: ShellScripts::default(),
        }
    }

    /// Export bundle to JSON string.
    pub fn export(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Import bundle from JSON string.
    pub fn import(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    /// Export to file.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Import from file.
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_import_roundtrip() {
        let mut bundle = ConfigBundle::new("[font]\nfamily = \"Fira Code\"");
        bundle.theme_toml = Some("background = \"#1a1a2e\"".into());
        bundle.keybindings.push(KeybindingEntry {
            modifiers: vec!["Super".into()],
            key: "c".into(),
            action: "Copy".into(),
        });

        let json = bundle.export().unwrap();
        let imported = ConfigBundle::import(&json).unwrap();

        assert_eq!(imported.version, 1);
        assert!(imported.config_toml.contains("Fira Code"));
        assert_eq!(imported.keybindings.len(), 1);
    }

    #[test]
    fn test_file_roundtrip() {
        let path = std::env::temp_dir().join("term_test_bundle.json");
        let bundle = ConfigBundle::new("scrollback = 5000");
        bundle.save(&path).unwrap();
        let loaded = ConfigBundle::load(&path).unwrap();
        assert!(loaded.config_toml.contains("5000"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_invalid_import() {
        assert!(ConfigBundle::import("not json").is_err());
    }
}
