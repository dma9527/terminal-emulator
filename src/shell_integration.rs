/// Shell Integration: detect command boundaries, track working directory,
/// measure command duration, and enable semantic navigation.
///
/// Works by injecting OSC markers into shell prompts (bash/zsh/fish).
/// Protocol uses OSC 133 (FinalTerm/iTerm2 compatible):
///   OSC 133;A — prompt start
///   OSC 133;B — command start (user pressed enter)
///   OSC 133;C — command output start
///   OSC 133;D;exit_code — command finished

use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CommandRegion {
    pub prompt_row: usize,
    pub command_row: usize,
    pub output_start_row: usize,
    pub output_end_row: Option<usize>,
    pub command_text: String,
    pub exit_code: Option<i32>,
    pub duration: Option<std::time::Duration>,
    pub working_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ShellState {
    Idle,
    Prompt,
    Command,
    Output,
}

pub struct ShellIntegration {
    state: ShellState,
    commands: Vec<CommandRegion>,
    current_prompt_row: usize,
    current_command_row: usize,
    current_output_row: usize,
    current_command: String,
    command_start: Option<Instant>,
    pub working_dir: String,
    max_history: usize,
}

impl ShellIntegration {
    pub fn new() -> Self {
        Self {
            state: ShellState::Idle,
            commands: Vec::new(),
            current_prompt_row: 0,
            current_command_row: 0,
            current_output_row: 0,
            current_command: String::new(),
            command_start: None,
            working_dir: String::new(),
            max_history: 1000,
        }
    }

    /// Handle OSC 133 sequences.
    pub fn handle_osc133(&mut self, param: &str, cursor_row: usize) {
        match param.chars().next() {
            Some('A') => { // Prompt start
                self.state = ShellState::Prompt;
                self.current_prompt_row = cursor_row;
            }
            Some('B') => { // Command start (enter pressed)
                self.state = ShellState::Command;
                self.current_command_row = cursor_row;
                self.command_start = Some(Instant::now());
            }
            Some('C') => { // Output start
                self.state = ShellState::Output;
                self.current_output_row = cursor_row;
            }
            Some('D') => { // Command finished
                let exit_code = param.strip_prefix("D;")
                    .and_then(|s| s.parse::<i32>().ok());
                let duration = self.command_start.map(|s| s.elapsed());

                let region = CommandRegion {
                    prompt_row: self.current_prompt_row,
                    command_row: self.current_command_row,
                    output_start_row: self.current_output_row,
                    output_end_row: Some(cursor_row),
                    command_text: self.current_command.clone(),
                    exit_code,
                    duration,
                    working_dir: self.working_dir.clone(),
                };
                self.commands.push(region);
                if self.commands.len() > self.max_history {
                    self.commands.remove(0);
                }
                self.state = ShellState::Idle;
                self.current_command.clear();
                self.command_start = None;
            }
            _ => {}
        }
    }

    /// Handle OSC 7 — working directory update.
    /// Format: `file://hostname/path`
    pub fn handle_osc7(&mut self, data: &str) {
        if let Some(path) = data.strip_prefix("file://") {
            // Skip hostname
            if let Some(idx) = path.find('/') {
                self.working_dir = path[idx..].to_string();
            }
        }
    }

    /// Set the current command text (captured from input).
    pub fn set_command_text(&mut self, text: &str) {
        self.current_command = text.to_string();
    }

    /// Get all completed commands.
    pub fn history(&self) -> &[CommandRegion] { &self.commands }

    /// Get the last N commands.
    pub fn recent(&self, n: usize) -> &[CommandRegion] {
        let start = self.commands.len().saturating_sub(n);
        &self.commands[start..]
    }

    /// Navigate to previous command prompt row.
    pub fn prev_prompt(&self, current_row: usize) -> Option<usize> {
        self.commands.iter().rev()
            .find(|c| c.prompt_row < current_row)
            .map(|c| c.prompt_row)
    }

    /// Navigate to next command prompt row.
    pub fn next_prompt(&self, current_row: usize) -> Option<usize> {
        self.commands.iter()
            .find(|c| c.prompt_row > current_row)
            .map(|c| c.prompt_row)
    }

    /// Get the last command's exit code.
    pub fn last_exit_code(&self) -> Option<i32> {
        self.commands.last().and_then(|c| c.exit_code)
    }

    /// Check if shell integration is active (received at least one marker).
    pub fn is_active(&self) -> bool {
        self.state != ShellState::Idle || !self.commands.is_empty()
    }

    /// Generate shell init script for bash.
    pub fn bash_init() -> &'static str {
        r#"
__term_prompt_start() { printf '\e]133;A\a'; }
__term_command_start() { printf '\e]133;B\a'; }
__term_output_start() { printf '\e]133;C\a'; }
__term_command_end() { printf '\e]133;D;%s\a' "$?"; }
__term_osc7() { printf '\e]7;file://%s%s\a' "$(hostname)" "$PWD"; }
PROMPT_COMMAND='__term_command_end; __term_prompt_start; __term_osc7'
trap '__term_command_start; __term_output_start' DEBUG
"#
    }

    /// Generate shell init script for zsh.
    pub fn zsh_init() -> &'static str {
        r#"
__term_osc7() { printf '\e]7;file://%s%s\a' "$(hostname)" "$PWD" }
__term_prompt_start() { printf '\e]133;A\a' }
__term_command_end() { printf '\e]133;D;%s\a' "$?" }
precmd() { __term_command_end; __term_prompt_start; __term_osc7 }
preexec() { printf '\e]133;B\a'; printf '\e]133;C\a' }
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_command_cycle() {
        let mut si = ShellIntegration::new();
        si.handle_osc133("A", 0);  // prompt
        si.handle_osc133("B", 0);  // enter
        si.set_command_text("ls -la");
        si.handle_osc133("C", 1);  // output
        si.handle_osc133("D;0", 5); // done, exit 0

        assert_eq!(si.history().len(), 1);
        let cmd = &si.history()[0];
        assert_eq!(cmd.command_text, "ls -la");
        assert_eq!(cmd.exit_code, Some(0));
        assert!(cmd.duration.is_some());
    }

    #[test]
    fn test_multiple_commands() {
        let mut si = ShellIntegration::new();
        for i in 0..3 {
            si.handle_osc133("A", i * 10);
            si.handle_osc133("B", i * 10);
            si.handle_osc133("C", i * 10 + 1);
            si.handle_osc133(&format!("D;{}", i), i * 10 + 5);
        }
        assert_eq!(si.history().len(), 3);
        assert_eq!(si.last_exit_code(), Some(2));
    }

    #[test]
    fn test_osc7_working_dir() {
        let mut si = ShellIntegration::new();
        si.handle_osc7("file://hostname/home/user/projects");
        assert_eq!(si.working_dir, "/home/user/projects");
    }

    #[test]
    fn test_prompt_navigation() {
        let mut si = ShellIntegration::new();
        for i in 0..3 {
            si.handle_osc133("A", i * 10);
            si.handle_osc133("B", i * 10);
            si.handle_osc133("C", i * 10 + 1);
            si.handle_osc133("D;0", i * 10 + 5);
        }
        assert_eq!(si.prev_prompt(25), Some(20));
        assert_eq!(si.prev_prompt(15), Some(10));
        assert_eq!(si.next_prompt(5), Some(10));
    }

    #[test]
    fn test_recent_commands() {
        let mut si = ShellIntegration::new();
        for i in 0..10 {
            si.handle_osc133("A", i);
            si.handle_osc133("B", i);
            si.handle_osc133("C", i);
            si.handle_osc133("D;0", i);
        }
        assert_eq!(si.recent(3).len(), 3);
    }

    #[test]
    fn test_failed_command() {
        let mut si = ShellIntegration::new();
        si.handle_osc133("A", 0);
        si.handle_osc133("B", 0);
        si.handle_osc133("C", 1);
        si.handle_osc133("D;127", 2); // command not found
        assert_eq!(si.last_exit_code(), Some(127));
    }

    #[test]
    fn test_not_active_initially() {
        let si = ShellIntegration::new();
        assert!(!si.is_active());
    }

    #[test]
    fn test_bash_init_not_empty() {
        assert!(ShellIntegration::bash_init().contains("133"));
    }

    #[test]
    fn test_zsh_init_not_empty() {
        assert!(ShellIntegration::zsh_init().contains("133"));
    }
}
