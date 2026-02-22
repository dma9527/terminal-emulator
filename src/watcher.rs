/// Config hot-reload: watches config file for changes.
/// Uses polling (stat-based) to avoid external deps.

use crate::config::Config;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub struct ConfigWatcher {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    poll_interval: Duration,
    last_check: std::time::Instant,
}

impl ConfigWatcher {
    pub fn new() -> Self {
        let path = Config::path();
        let last_modified = std::fs::metadata(&path).ok()
            .and_then(|m| m.modified().ok());
        Self {
            path,
            last_modified,
            poll_interval: Duration::from_secs(2),
            last_check: std::time::Instant::now(),
        }
    }

    /// Check if config file changed. Call this periodically (e.g. each frame).
    /// Returns Some(Config) if file was modified since last check.
    pub fn poll(&mut self) -> Option<Config> {
        if self.last_check.elapsed() < self.poll_interval {
            return None;
        }
        self.last_check = std::time::Instant::now();

        let modified = std::fs::metadata(&self.path).ok()
            .and_then(|m| m.modified().ok());

        if modified != self.last_modified {
            self.last_modified = modified;
            let contents = std::fs::read_to_string(&self.path).unwrap_or_default();
            Some(Config::from_str(&contents))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_watcher_no_change() {
        let mut w = ConfigWatcher::new();
        // Immediate poll should return None (within interval)
        assert!(w.poll().is_none());
    }

    #[test]
    fn test_watcher_detects_change() {
        let dir = std::env::temp_dir().join("term_test_watcher");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");

        // Write initial config
        std::fs::write(&path, "scrollback = 1000").unwrap();

        let mut w = ConfigWatcher {
            path: path.clone(),
            last_modified: None, // force detection
            poll_interval: Duration::from_millis(0),
            last_check: std::time::Instant::now() - Duration::from_secs(10),
        };

        // Should detect the file exists
        let cfg = w.poll();
        assert!(cfg.is_some());

        // Modify file
        std::thread::sleep(Duration::from_millis(50));
        std::fs::write(&path, "scrollback = 2000").unwrap();
        w.last_check = std::time::Instant::now() - Duration::from_secs(10);

        let cfg = w.poll().unwrap();
        assert_eq!(cfg.scrollback, 2000);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
