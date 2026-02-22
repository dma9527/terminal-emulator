/// Session save/restore: persist terminal sessions across app restarts.

use serde::{Serialize, Deserialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub working_dir: String,
    pub shell: String,
    pub cols: usize,
    pub rows: usize,
    pub title: String,
    pub scrollback_lines: Vec<String>,
}

impl SessionState {
    pub fn new(working_dir: &str, shell: &str, cols: usize, rows: usize) -> Self {
        Self {
            working_dir: working_dir.into(),
            shell: shell.into(),
            cols, rows,
            title: String::new(),
            scrollback_lines: Vec::new(),
        }
    }

    /// Save session to file.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)
    }

    /// Load session from file.
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    /// Default session directory.
    pub fn sessions_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".config").join("term").join("sessions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_roundtrip() {
        let dir = std::env::temp_dir().join("term_test_session");
        let path = dir.join("test.json");

        let session = SessionState {
            working_dir: "/home/user".into(),
            shell: "/bin/zsh".into(),
            cols: 80, rows: 24,
            title: "test session".into(),
            scrollback_lines: vec!["line1".into(), "line2".into()],
        };

        session.save(&path).unwrap();
        let loaded = SessionState::load(&path).unwrap();

        assert_eq!(loaded.working_dir, "/home/user");
        assert_eq!(loaded.cols, 80);
        assert_eq!(loaded.title, "test session");
        assert_eq!(loaded.scrollback_lines.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_session_load_missing() {
        let result = SessionState::load(std::path::Path::new("/nonexistent/path.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_sessions_dir() {
        let dir = SessionState::sessions_dir();
        assert!(dir.to_str().unwrap().contains("sessions"));
    }
}
