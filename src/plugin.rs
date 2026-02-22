/// Plugin system: Lua-based extension API.
/// Plugins can hook into terminal events and add custom behavior.

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum HookEvent {
    SessionStart,
    SessionEnd,
    LineOutput(String),
    TitleChange(String),
    DirectoryChange(String),
    Bell,
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub path: PathBuf,
    pub enabled: bool,
}

pub struct PluginManager {
    plugins: Vec<PluginInfo>,
    hooks: HashMap<String, Vec<usize>>, // event_name -> plugin indices
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: Vec::new(), hooks: HashMap::new() }
    }

    /// Register a plugin.
    pub fn register(&mut self, info: PluginInfo) -> usize {
        let idx = self.plugins.len();
        self.plugins.push(info);
        idx
    }

    /// Subscribe a plugin to an event.
    pub fn subscribe(&mut self, plugin_idx: usize, event: &str) {
        self.hooks.entry(event.to_string()).or_default().push(plugin_idx);
    }

    /// Get plugins subscribed to an event.
    pub fn subscribers(&self, event: &str) -> Vec<&PluginInfo> {
        self.hooks.get(event)
            .map(|indices| indices.iter()
                .filter_map(|&i| self.plugins.get(i))
                .filter(|p| p.enabled)
                .collect())
            .unwrap_or_default()
    }

    /// List all registered plugins.
    pub fn list(&self) -> &[PluginInfo] { &self.plugins }

    /// Enable/disable a plugin by index.
    pub fn set_enabled(&mut self, idx: usize, enabled: bool) {
        if let Some(p) = self.plugins.get_mut(idx) {
            p.enabled = enabled;
        }
    }

    /// Plugin directory: `~/.config/term/plugins/`
    pub fn plugins_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".config").join("term").join("plugins")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plugin() -> PluginInfo {
        PluginInfo {
            name: "test-plugin".into(),
            version: "1.0.0".into(),
            description: "A test plugin".into(),
            path: PathBuf::from("/tmp/test.lua"),
            enabled: true,
        }
    }

    #[test]
    fn test_register_and_list() {
        let mut mgr = PluginManager::new();
        mgr.register(test_plugin());
        assert_eq!(mgr.list().len(), 1);
        assert_eq!(mgr.list()[0].name, "test-plugin");
    }

    #[test]
    fn test_subscribe_and_notify() {
        let mut mgr = PluginManager::new();
        let idx = mgr.register(test_plugin());
        mgr.subscribe(idx, "bell");
        let subs = mgr.subscribers("bell");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].name, "test-plugin");
    }

    #[test]
    fn test_disabled_plugin_not_notified() {
        let mut mgr = PluginManager::new();
        let idx = mgr.register(test_plugin());
        mgr.subscribe(idx, "bell");
        mgr.set_enabled(idx, false);
        assert!(mgr.subscribers("bell").is_empty());
    }

    #[test]
    fn test_no_subscribers() {
        let mgr = PluginManager::new();
        assert!(mgr.subscribers("nonexistent").is_empty());
    }

    #[test]
    fn test_plugins_dir() {
        let dir = PluginManager::plugins_dir();
        assert!(dir.to_str().unwrap().contains("plugins"));
    }
}
