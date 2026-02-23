/// Shell integration scripts — injected automatically at shell startup.
/// These emit OSC 133 markers for command boundaries and OSC 7 for cwd.

pub const ZSH_INTEGRATION: &str = r#"
# Terminal shell integration (zsh)
__term_precmd() {
    local exit_code=$?
    printf '\e]133;D;%d\a' "$exit_code"
    printf '\e]133;A\a'
    printf '\e]7;file://%s%s\a' "$(hostname)" "$PWD"
}
__term_preexec() {
    printf '\e]133;B\a'
    printf '\e]133;C\a'
}
[[ -z "$__term_integrated" ]] && {
    export __term_integrated=1
    precmd_functions+=(__term_precmd)
    preexec_functions+=(__term_preexec)
    # Emit initial prompt marker
    printf '\e]133;A\a'
    printf '\e]7;file://%s%s\a' "$(hostname)" "$PWD"
}
"#;

pub const BASH_INTEGRATION: &str = r#"
# Terminal shell integration (bash)
__term_prompt_cmd() {
    local exit_code=$?
    printf '\e]133;D;%d\a' "$exit_code"
    printf '\e]133;A\a'
    printf '\e]7;file://%s%s\a' "$(hostname)" "$PWD"
}
__term_preexec() {
    printf '\e]133;B\a'
    printf '\e]133;C\a'
}
if [[ -z "$__term_integrated" ]]; then
    export __term_integrated=1
    PROMPT_COMMAND='__term_prompt_cmd'
    trap '__term_preexec' DEBUG
    printf '\e]133;A\a'
    printf '\e]7;file://%s%s\a' "$(hostname)" "$PWD"
fi
"#;

/// Write shell integration scripts to a temp directory and return the path.
/// For zsh: writes a .zshrc that sources the user's real .zshrc then adds integration.
/// For bash: writes a .bashrc similarly.
pub fn write_integration_scripts() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("term_shell_integration");
    let _ = std::fs::create_dir_all(&dir);

    // Zsh: create a wrapper .zshenv that sources user config then adds integration
    let zsh_script = dir.join("zsh_integration.zsh");
    std::fs::write(&zsh_script, ZSH_INTEGRATION).ok();

    let zshrc = dir.join(".zshrc");
    let user_zshrc = dirs_home().join(".zshrc");
    let content = format!(
        "# Load user config\n[[ -f \"{}\" ]] && source \"{}\"\n# Shell integration\nsource \"{}\"\n",
        user_zshrc.display(), user_zshrc.display(), zsh_script.display()
    );
    std::fs::write(&zshrc, content).ok();

    // Bash
    let bash_script = dir.join("bash_integration.bash");
    std::fs::write(&bash_script, BASH_INTEGRATION).ok();

    let bashrc = dir.join(".bashrc");
    let user_bashrc = dirs_home().join(".bashrc");
    let content = format!(
        "[[ -f \"{}\" ]] && source \"{}\"\nsource \"{}\"\n",
        user_bashrc.display(), user_bashrc.display(), bash_script.display()
    );
    std::fs::write(&bashrc, content).ok();

    dir
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_else(|_| ".".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zsh_integration_has_osc133() {
        assert!(ZSH_INTEGRATION.contains("133;A"));
        assert!(ZSH_INTEGRATION.contains("133;B"));
        assert!(ZSH_INTEGRATION.contains("133;C"));
        assert!(ZSH_INTEGRATION.contains("133;D"));
    }

    #[test]
    fn test_bash_integration_has_osc133() {
        assert!(BASH_INTEGRATION.contains("133;A"));
        assert!(BASH_INTEGRATION.contains("133;D"));
    }

    #[test]
    fn test_write_scripts() {
        let dir = write_integration_scripts();
        assert!(dir.join(".zshrc").exists());
        assert!(dir.join(".bashrc").exists());
    }
}
