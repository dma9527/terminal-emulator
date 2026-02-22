/// Cross-platform keybinding system.
/// Same keybindings work on macOS (Cmd) and Linux (Ctrl),
/// with user-customizable overrides via config.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    Super,  // Cmd on macOS, Ctrl on Linux
    Ctrl,
    Alt,
    Shift,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    Copy,
    Paste,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    NextPane,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    SearchOpen,
    SearchNext,
    SearchPrev,
    ScrollUp,
    ScrollDown,
    ScrollToTop,
    ScrollToBottom,
    PrevPrompt,
    NextPrompt,
    ClearScreen,
    Custom(String),
}

pub struct KeybindingManager {
    bindings: HashMap<KeyBinding, Action>,
    platform: Platform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    MacOS,
    Linux,
}

impl Platform {
    pub fn detect() -> Self {
        if cfg!(target_os = "macos") { Platform::MacOS }
        else { Platform::Linux }
    }
}

impl KeybindingManager {
    pub fn new(platform: Platform) -> Self {
        let mut mgr = Self { bindings: HashMap::new(), platform };
        mgr.load_defaults();
        mgr
    }

    fn load_defaults(&mut self) {
        let sup = Modifier::Super; // Cmd on mac, Ctrl on linux

        let defaults = vec![
            (vec![sup], "c", Action::Copy),
            (vec![sup], "v", Action::Paste),
            (vec![sup], "t", Action::NewTab),
            (vec![sup], "w", Action::CloseTab),
            (vec![sup, Modifier::Shift], "]", Action::NextTab),
            (vec![sup, Modifier::Shift], "[", Action::PrevTab),
            (vec![sup], "d", Action::SplitVertical),
            (vec![sup, Modifier::Shift], "d", Action::SplitHorizontal),
            (vec![sup], "=", Action::IncreaseFontSize),
            (vec![sup], "-", Action::DecreaseFontSize),
            (vec![sup], "0", Action::ResetFontSize),
            (vec![sup], "f", Action::SearchOpen),
            (vec![sup], "g", Action::SearchNext),
            (vec![sup, Modifier::Shift], "g", Action::SearchPrev),
            (vec![sup], "k", Action::ClearScreen),
            (vec![sup, Modifier::Shift], "Up", Action::PrevPrompt),
            (vec![sup, Modifier::Shift], "Down", Action::NextPrompt),
        ];

        for (mods, key, action) in defaults {
            self.bind(KeyBinding { modifiers: mods, key: key.into() }, action);
        }
    }

    /// Add or override a keybinding.
    pub fn bind(&mut self, binding: KeyBinding, action: Action) {
        self.bindings.insert(binding, action);
    }

    /// Remove a keybinding.
    pub fn unbind(&mut self, binding: &KeyBinding) {
        self.bindings.remove(binding);
    }

    /// Look up action for a key event.
    pub fn lookup(&self, binding: &KeyBinding) -> Option<&Action> {
        self.bindings.get(binding)
    }

    /// Get all bindings for an action.
    pub fn bindings_for(&self, action: &Action) -> Vec<&KeyBinding> {
        self.bindings.iter()
            .filter(|(_, a)| *a == action)
            .map(|(k, _)| k)
            .collect()
    }

    /// Get display string for a keybinding (platform-aware).
    pub fn display(&self, binding: &KeyBinding) -> String {
        let mut parts = Vec::new();
        for m in &binding.modifiers {
            parts.push(match (m, self.platform) {
                (Modifier::Super, Platform::MacOS) => "⌘",
                (Modifier::Super, Platform::Linux) => "Ctrl",
                (Modifier::Ctrl, _) => "Ctrl",
                (Modifier::Alt, Platform::MacOS) => "⌥",
                (Modifier::Alt, Platform::Linux) => "Alt",
                (Modifier::Shift, _) => "⇧",
            });
        }
        parts.push(&binding.key);
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bindings() {
        let mgr = KeybindingManager::new(Platform::MacOS);
        let binding = KeyBinding { modifiers: vec![Modifier::Super], key: "c".into() };
        assert_eq!(mgr.lookup(&binding), Some(&Action::Copy));
    }

    #[test]
    fn test_custom_binding() {
        let mut mgr = KeybindingManager::new(Platform::Linux);
        let binding = KeyBinding { modifiers: vec![Modifier::Alt], key: "x".into() };
        mgr.bind(binding.clone(), Action::Custom("my_action".into()));
        assert_eq!(mgr.lookup(&binding), Some(&Action::Custom("my_action".into())));
    }

    #[test]
    fn test_unbind() {
        let mut mgr = KeybindingManager::new(Platform::MacOS);
        let binding = KeyBinding { modifiers: vec![Modifier::Super], key: "c".into() };
        mgr.unbind(&binding);
        assert!(mgr.lookup(&binding).is_none());
    }

    #[test]
    fn test_display_macos() {
        let mgr = KeybindingManager::new(Platform::MacOS);
        let binding = KeyBinding { modifiers: vec![Modifier::Super, Modifier::Shift], key: "t".into() };
        assert_eq!(mgr.display(&binding), "⌘+⇧+t");
    }

    #[test]
    fn test_display_linux() {
        let mgr = KeybindingManager::new(Platform::Linux);
        let binding = KeyBinding { modifiers: vec![Modifier::Super], key: "c".into() };
        assert_eq!(mgr.display(&binding), "Ctrl+c");
    }

    #[test]
    fn test_bindings_for_action() {
        let mgr = KeybindingManager::new(Platform::MacOS);
        let bindings = mgr.bindings_for(&Action::Copy);
        assert_eq!(bindings.len(), 1);
    }

    #[test]
    fn test_platform_detect() {
        let p = Platform::detect();
        assert_eq!(p, Platform::MacOS); // running on macOS
    }
}
