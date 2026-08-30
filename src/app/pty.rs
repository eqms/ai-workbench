use std::path::Path;

use crate::backend::AiBackend;
use crate::config::Config;
use crate::terminal::PseudoTerminal;
use crate::types::{ClaudeEffort, ClaudeModel, ClaudePermissionMode, PaneId};
use crate::update::log_update;

use super::App;

/// Bundled Claude Code startup options, assembled from dialog state + config.
#[derive(Debug, Clone, Default)]
pub(crate) struct StartupOptions {
    pub permission_mode: ClaudePermissionMode,
    pub model: ClaudeModel,
    pub effort: ClaudeEffort,
    pub session_name: String,
    pub worktree: String,
}

impl App {
    pub(super) fn build_ai_command(
        config: &Config,
        backend: AiBackend,
        opts: &StartupOptions,
    ) -> Vec<String> {
        let backend_cmd = config.pty.command_for(backend);
        let mut cmd = if backend_cmd.is_empty() {
            // Default: use the same shell as Terminal pane
            let mut shell_cmd = vec![config.terminal.shell_path.clone()];
            shell_cmd.extend(config.terminal.shell_args.clone());
            shell_cmd
        } else {
            backend_cmd.clone()
        };

        // Claude-specific startup flags apply only in Claude mode, and only when
        // a real backend command (not the shell fallback) is used.
        if backend.supports_claude_flags() && !backend_cmd.is_empty() {
            // Permission mode
            if opts.permission_mode.is_yolo() {
                if !cmd
                    .iter()
                    .any(|a| a.contains("--dangerously-skip-permissions"))
                {
                    cmd.push("--dangerously-skip-permissions".to_string());
                }
            } else if let Some(flag_value) = opts.permission_mode.cli_flag() {
                if !cmd.iter().any(|a| a.contains("--permission-mode")) {
                    cmd.push("--permission-mode".to_string());
                    cmd.push(flag_value.to_string());
                }
            }

            // Model
            if let Some(model) = opts.model.cli_flag() {
                if !cmd.iter().any(|a| a == "--model") {
                    cmd.push("--model".to_string());
                    cmd.push(model.to_string());
                }
            }

            // Effort
            if let Some(effort) = opts.effort.cli_flag() {
                if !cmd.iter().any(|a| a == "--effort") {
                    cmd.push("--effort".to_string());
                    cmd.push(effort.to_string());
                }
            }

            // Session name (--name)
            if !opts.session_name.is_empty() && !cmd.iter().any(|a| a == "--name") {
                cmd.push("--name".to_string());
                cmd.push(opts.session_name.clone());
            }

            // Worktree (--worktree)
            if !opts.worktree.is_empty() && !cmd.iter().any(|a| a == "--worktree") {
                cmd.push("--worktree".to_string());
                cmd.push(opts.worktree.clone());
            }
        }

        cmd
    }

    pub(super) fn build_agent_command(
        config: &Config,
        backend: AiBackend,
        extra_args: &[String],
    ) -> Vec<String> {
        let has_backend_command = !config.pty.command_for(backend).is_empty();
        let mut command = Self::build_ai_command(config, backend, &StartupOptions::default());
        if has_backend_command {
            command.extend(extra_args.iter().cloned());
        }
        command
    }

    /// Initialize Claude PTY with the given startup options
    pub(super) fn init_claude_pty(&mut self, opts: StartupOptions) {
        self.claude_permission_mode = opts.permission_mode;
        self.claude_pty_pending = false;

        let claude_cmd = Self::build_ai_command(&self.config, self.backend, &opts);
        self.claude_command_used = claude_cmd.join(" ");

        let cwd = self.file_browser.current_dir.clone();
        let rows = 24;
        let cols = 80;

        match PseudoTerminal::new(&claude_cmd, rows, cols, &cwd) {
            Ok(pty) => {
                self.terminals.insert(PaneId::Claude, pty);
                self.claude_error = None;
            }
            Err(e) => {
                self.claude_error = Some(format!(
                    "Failed to start shell\n\nCommand: {}\n\nError: {}",
                    self.claude_command_used, e
                ));
            }
        }
    }

    /// Initialize a non-Claude AI PTY with arguments selected in its startup
    /// form. The configured command remains authoritative; form arguments are
    /// appended for this invocation only and are not persisted into config.
    pub(super) fn init_agent_pty(&mut self, extra_args: Vec<String>) {
        self.claude_pty_pending = false;
        let command = Self::build_agent_command(&self.config, self.backend, &extra_args);
        self.claude_command_used = command.join(" ");

        let cwd = self.file_browser.current_dir.clone();
        match PseudoTerminal::new(&command, 24, 80, &cwd) {
            Ok(pty) => {
                self.terminals.insert(PaneId::Claude, pty);
                self.claude_error = None;
            }
            Err(error) => {
                self.claude_error = Some(format!(
                    "Failed to start shell\n\nCommand: {}\n\nError: {}",
                    self.claude_command_used, error
                ));
            }
        }
        self.active_pane = PaneId::Claude;
    }

    /// Initialize Claude PTY after wizard completion
    /// Shows permission mode dialog if configured, otherwise starts Claude directly
    pub(super) fn init_claude_after_wizard(&mut self) {
        // Remove existing Claude PTY (started with pre-wizard config)
        self.terminals.remove(&PaneId::Claude);
        self.claude_error = None;

        let should_show_permission_dialog =
            self.backend.supports_claude_flags() && self.config.claude.show_permission_dialog;

        if should_show_permission_dialog {
            self.claude_pty_pending = true;
            self.permission_mode_dialog.open_with_defaults(
                self.config.claude.default_permission_mode,
                self.config.claude.default_model,
                self.config.claude.default_effort,
                &self.config.claude.default_session_name,
                &self.config.claude.default_worktree,
            );
        } else if self.backend.supports_claude_flags() {
            let opts = StartupOptions {
                permission_mode: self
                    .config
                    .claude
                    .default_permission_mode
                    .unwrap_or(ClaudePermissionMode::Default),
                model: self.config.claude.default_model,
                effort: self.config.claude.default_effort,
                session_name: self.config.claude.default_session_name.clone(),
                worktree: self.config.claude.default_worktree.clone(),
            };
            self.init_claude_pty(opts);
            self.active_pane = PaneId::Claude;
        } else {
            self.claude_pty_pending = true;
            self.agent_startup.open(self.backend);
        }
    }

    /// Switch the AI backend to `target`, persist the choice to the session,
    /// and respawn the AI pane using the new backend. Invoked from the F8
    /// backend-selection menu on confirm (Enter). A switch to the already-active
    /// backend still respawns the pane (acts as a restart). The respawn reuses
    /// [`Self::init_claude_after_wizard`], which shows the Claude startup dialog
    /// only when switching to Claude with `show_permission_dialog` enabled, and
    /// otherwise opens the selected backend's sectioned startup dialog.
    pub(super) fn apply_ai_backend(&mut self, target: crate::backend::AiBackend) {
        self.backend = target;
        self.session.last_backend = target;
        crate::session::save_session(&self.session);

        self.init_claude_after_wizard();

        // Footer confirmation (reuses the 2 s copy-flash channel).
        self.copy_flash_message = Some(format!("Backend: {}", target.short_label()));
        self.last_copy_time = Some(std::time::Instant::now());
    }

    /// Cancel the backend-selection menu, whatever the input path — `Esc` or a
    /// click outside the menu.
    ///
    /// Cancelling an F8 switch is nothing but closing the menu. Cancelling the
    /// *initial* launcher is not: no AI PTY has been spawned at that point
    /// (`App::new` deliberately waits for the chooser), so merely closing the
    /// menu leaves the AI pane without a terminal for the rest of the session.
    /// The startup cancel therefore still applies the current backend — the
    /// same "cancel means keep what was configured" semantics the permission
    /// dialog uses.
    ///
    /// Keyboard and mouse must call *this*, never `backend_switch.close()`
    /// directly: the two paths drifted apart once already, and only the
    /// keyboard side carried the startup branch.
    pub(super) fn cancel_backend_switch(&mut self) {
        let was_startup = self.backend_switch.startup;
        let target = self.backend;
        self.backend_switch.close();
        if was_startup {
            self.apply_ai_backend(target);
        }
    }

    /// Sync directory to Terminal pane only (not Claude - Claude only gets cd at startup)
    pub(super) fn sync_terminals(&mut self) {
        let path_str = self.file_browser.current_dir.to_string_lossy();
        match quote_path_for_cd(&path_str) {
            Some(cmd) => {
                // Only sync to Terminal, not Claude (Claude should keep its initial directory)
                if let Some(pty) = self.terminals.get_mut(&PaneId::Terminal) {
                    let _ = pty.write_input(cmd.as_bytes());
                }
            }
            None => {
                log_update(&format!(
                    "sync_terminals: skipping unquotable path: {:?}",
                    self.file_browser.current_dir
                ));
            }
        }
    }

    /// Send cd command to a specific terminal pane
    pub(super) fn sync_terminal_to_current_dir(&mut self, pane: PaneId) {
        let path_str = self.file_browser.current_dir.to_string_lossy();
        match quote_path_for_cd(&path_str) {
            Some(cmd) => {
                if let Some(pty) = self.terminals.get_mut(&pane) {
                    let _ = pty.write_input(cmd.as_bytes());
                }
            }
            None => {
                log_update(&format!(
                    "sync_terminals: skipping unquotable path: {:?}",
                    self.file_browser.current_dir
                ));
            }
        }
    }

    /// Ensure a PTY exists for the given pane. Used for lazy-init of LazyGit/Terminal.
    ///
    /// - No-op if a PTY for `pane_id` is already in `terminals`.
    /// - For `PaneId::Terminal` and `PaneId::LazyGit`: spawns a new PTY in the
    ///   current file-browser directory; on failure stores the error message in
    ///   `terminal_error` / `lazygit_error` (rendered in `terminal_pane.rs`).
    /// - For other pane IDs: no-op (Claude has its own dedicated init paths via
    ///   `init_claude_pty` / `init_claude_after_wizard`).
    pub(super) fn ensure_pty_for_pane(&mut self, pane_id: PaneId) {
        if self.terminals.contains_key(&pane_id) {
            return;
        }

        let cwd = self.file_browser.current_dir.clone();
        let rows = 24;
        let cols = 80;

        let cmd = match pane_id {
            PaneId::Terminal => {
                let mut c = vec![self.config.terminal.shell_path.clone()];
                c.extend(self.config.terminal.shell_args.clone());
                c
            }
            PaneId::LazyGit => {
                if self.config.pty.lazygit_command.is_empty() {
                    vec!["lazygit".to_string()]
                } else {
                    self.config.pty.lazygit_command.clone()
                }
            }
            _ => return,
        };

        match PseudoTerminal::new(&cmd, rows, cols, &cwd) {
            Ok(pty) => {
                self.terminals.insert(pane_id, pty);
                match pane_id {
                    PaneId::Terminal => self.terminal_error = None,
                    PaneId::LazyGit => self.lazygit_error = None,
                    _ => {}
                }
            }
            Err(e) => {
                let msg = format!(
                    "Failed to start {}\n\nCommand: {}\n\nError: {}",
                    if pane_id == PaneId::Terminal {
                        "shell"
                    } else {
                        "LazyGit"
                    },
                    cmd.join(" "),
                    e
                );
                match pane_id {
                    PaneId::Terminal => self.terminal_error = Some(msg),
                    PaneId::LazyGit => self.lazygit_error = Some(msg),
                    _ => {}
                }
            }
        }
    }

    /// Restart LazyGit PTY in current directory
    pub(super) fn restart_lazygit_in_current_dir(&mut self) {
        let cwd = self.file_browser.current_dir.clone();
        // Use default size, will be resized on first draw
        let rows = 24;
        let cols = 80;

        // Get lazygit command from config
        let lazygit_cmd = if self.config.pty.lazygit_command.is_empty() {
            vec!["lazygit".to_string()]
        } else {
            self.config.pty.lazygit_command.clone()
        };

        // Remove old PTY
        self.terminals.remove(&PaneId::LazyGit);

        // Create new PTY in current directory
        if let Ok(pty) = PseudoTerminal::new(&lazygit_cmd, rows, cols, &cwd) {
            self.terminals.insert(PaneId::LazyGit, pty);
        }
    }

    pub(super) fn check_and_restart_exited_ptys(&mut self) {
        // Skip if auto-restart is disabled
        if !self.config.pty.auto_restart {
            return;
        }

        let cwd = self.file_browser.current_dir.clone();
        let rows = 24;
        let cols = 80;

        // Check each terminal PTY
        let panes_to_restart: Vec<PaneId> = self
            .terminals
            .iter()
            .filter(|(_, pty)| pty.has_exited())
            .map(|(id, _)| *id)
            .collect();

        for pane_id in panes_to_restart {
            // Skip restart for hidden lazy-init panes — leave the dead PTY removed
            // so the slot stays empty until the user toggles the pane visible again.
            let is_visible = match pane_id {
                PaneId::LazyGit => self.show_lazygit,
                PaneId::Terminal => self.show_terminal,
                PaneId::Claude => true,
                _ => false,
            };
            if !is_visible {
                self.terminals.remove(&pane_id);
                continue;
            }

            // Remove the old PTY
            self.terminals.remove(&pane_id);

            // Determine the command to restart based on pane type
            let cmd = match pane_id {
                PaneId::Claude => {
                    let backend_cmd = self.config.pty.command_for(self.backend);
                    if backend_cmd.is_empty() {
                        let mut cmd = vec![self.config.terminal.shell_path.clone()];
                        cmd.extend(self.config.terminal.shell_args.clone());
                        cmd
                    } else {
                        backend_cmd.clone()
                    }
                }
                PaneId::LazyGit => {
                    if self.config.pty.lazygit_command.is_empty() {
                        vec!["lazygit".to_string()]
                    } else {
                        self.config.pty.lazygit_command.clone()
                    }
                }
                PaneId::Terminal => {
                    let mut cmd = vec![self.config.terminal.shell_path.clone()];
                    cmd.extend(self.config.terminal.shell_args.clone());
                    cmd
                }
                _ => continue, // Skip non-terminal panes
            };

            // Start a fresh shell/process
            if let Ok(new_pty) = PseudoTerminal::new(&cmd, rows, cols, &cwd) {
                self.terminals.insert(pane_id, new_pty);
            }
        }
    }

    /// Restart a single PTY (manual restart when auto_restart is disabled)
    pub(super) fn restart_single_pty(&mut self, pane_id: PaneId) {
        // Don't restart hidden lazy-init panes — they will be re-spawned by
        // ensure_pty_for_pane() the next time the user toggles them visible.
        let is_visible = match pane_id {
            PaneId::LazyGit => self.show_lazygit,
            PaneId::Terminal => self.show_terminal,
            PaneId::Claude => true,
            _ => false,
        };
        if !is_visible {
            self.terminals.remove(&pane_id);
            return;
        }

        let cwd = self.file_browser.current_dir.clone();
        let rows = 24;
        let cols = 80;

        // Remove the old PTY
        self.terminals.remove(&pane_id);

        // Determine the command to restart based on pane type
        let cmd = match pane_id {
            PaneId::Claude => {
                let backend_cmd = self.config.pty.command_for(self.backend);
                if backend_cmd.is_empty() {
                    let mut cmd = vec![self.config.terminal.shell_path.clone()];
                    cmd.extend(self.config.terminal.shell_args.clone());
                    cmd
                } else {
                    backend_cmd.clone()
                }
            }
            PaneId::LazyGit => {
                if self.config.pty.lazygit_command.is_empty() {
                    vec!["lazygit".to_string()]
                } else {
                    self.config.pty.lazygit_command.clone()
                }
            }
            PaneId::Terminal => {
                let mut cmd = vec![self.config.terminal.shell_path.clone()];
                cmd.extend(self.config.terminal.shell_args.clone());
                cmd
            }
            _ => return, // Skip non-terminal panes
        };

        // Start a fresh shell/process
        if let Ok(new_pty) = PseudoTerminal::new(&cmd, rows, cols, &cwd) {
            self.terminals.insert(pane_id, new_pty);
        }
    }

    /// Insert file path at cursor in target terminal pane
    pub(super) fn insert_path_at_cursor(&mut self, target: PaneId, path: &Path) {
        if let Some(pty) = self.terminals.get_mut(&target) {
            let path_str = path.to_string_lossy();
            // shlex::try_quote fails only on NUL-byte paths (invalid on Unix/Windows).
            // On failure, log a debug note and return without writing anything to the PTY.
            // Never write a raw, unquoted path — it could execute shell metacharacters.
            let escaped = match shlex::try_quote(&path_str) {
                Ok(c) => c.into_owned(),
                Err(_) => {
                    // Path contains a NUL byte — reject silently (unreachable on sane FS).
                    return;
                }
            };
            // Write to PTY (no newline - just insert the path)
            let _ = pty.write_input(escaped.as_bytes());
        }
    }
}

/// Quote a filesystem path for use in a `cd` PTY command.
///
/// Returns `None` only when `shlex::try_quote` rejects the path — which
/// happens exclusively for paths containing a NUL byte. On any real Unix
/// filesystem NUL cannot appear in a path, so `None` is effectively
/// unreachable in practice. Callers that receive `None` must **not** fall
/// back to an unescaped path; they should log and skip instead.
fn quote_path_for_cd(path_str: &str) -> Option<String> {
    shlex::try_quote(path_str)
        .ok()
        .map(|q| format!("cd {}\r", q.into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_quote_path_for_cd_handles_spaces() {
        let result = quote_path_for_cd("/home/user/my project");
        assert!(result.is_some(), "space path must be quotable");
        let cmd = result.unwrap();
        assert!(cmd.starts_with("cd "), "must start with cd");
        assert!(cmd.ends_with('\r'), "must end with CR");
        // shlex quotes spaces with single quotes
        assert!(
            cmd.contains('\'') || cmd.contains('"'),
            "spaces must be quoted: {cmd}"
        );
    }

    #[test]
    fn test_quote_path_for_cd_simple_path() {
        let result = quote_path_for_cd("/home/user/projects");
        assert_eq!(result, Some("cd /home/user/projects\r".to_string()));
    }

    fn config_with_claude_command() -> Config {
        let mut cfg = Config::default();
        cfg.pty.claude_command = vec!["claude".to_string()];
        cfg
    }

    fn base_opts() -> StartupOptions {
        StartupOptions {
            permission_mode: ClaudePermissionMode::Default,
            model: ClaudeModel::Unset,
            effort: ClaudeEffort::Unset,
            session_name: String::new(),
            worktree: String::new(),
        }
    }

    #[test]
    fn test_build_command_shell_fallback_adds_no_flags() {
        // When the backend command is empty, the shell fallback is used and no
        // Claude flags are appended.
        let mut cfg = Config::default();
        cfg.pty.claude_command = vec![]; // force shell fallback
        let mut opts = base_opts();
        opts.permission_mode = ClaudePermissionMode::Auto;
        opts.model = ClaudeModel::Sonnet;
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(!cmd.iter().any(|a| a == "--permission-mode"));
        assert!(!cmd.iter().any(|a| a == "--model"));
    }

    #[test]
    fn test_build_command_auto_mode() {
        let cfg = config_with_claude_command();
        let mut opts = base_opts();
        opts.permission_mode = ClaudePermissionMode::Auto;
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(cmd.contains(&"--permission-mode".to_string()));
        assert!(cmd.contains(&"auto".to_string()));
    }

    #[test]
    fn test_build_command_with_model() {
        let cfg = config_with_claude_command();
        let mut opts = base_opts();
        opts.model = ClaudeModel::Sonnet;
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"sonnet".to_string()));
    }

    #[test]
    fn test_build_command_with_effort() {
        let cfg = config_with_claude_command();
        let mut opts = base_opts();
        opts.effort = ClaudeEffort::High;
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(cmd.contains(&"--effort".to_string()));
        assert!(cmd.contains(&"high".to_string()));
    }

    #[test]
    fn test_build_command_with_session_name() {
        let cfg = config_with_claude_command();
        let mut opts = base_opts();
        opts.session_name = "test-session".to_string();
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(cmd.contains(&"--name".to_string()));
        assert!(cmd.contains(&"test-session".to_string()));
    }

    #[test]
    fn test_build_command_with_worktree() {
        let cfg = config_with_claude_command();
        let mut opts = base_opts();
        opts.worktree = "feature-x".to_string();
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(cmd.contains(&"--worktree".to_string()));
        assert!(cmd.contains(&"feature-x".to_string()));
    }

    #[test]
    fn test_build_command_yolo_mode_uses_dangerously_skip() {
        let cfg = config_with_claude_command();
        let mut opts = base_opts();
        opts.permission_mode = ClaudePermissionMode::DangerouslySkipPermissions;
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(cmd.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!cmd.iter().any(|a| a == "--permission-mode"));
    }

    #[test]
    fn test_build_command_empty_values_do_not_emit_flags() {
        let cfg = config_with_claude_command();
        let opts = base_opts(); // all empty/Unset/false
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert!(!cmd.iter().any(|a| a == "--model"));
        assert!(!cmd.iter().any(|a| a == "--effort"));
        assert!(!cmd.iter().any(|a| a == "--name"));
        assert!(!cmd.iter().any(|a| a == "--worktree"));
    }

    #[test]
    fn test_build_command_all_flags_combined() {
        let cfg = config_with_claude_command();
        let opts = StartupOptions {
            permission_mode: ClaudePermissionMode::Auto,
            model: ClaudeModel::Opus,
            effort: ClaudeEffort::Max,
            session_name: "session1".to_string(),
            worktree: "feat".to_string(),
        };
        let cmd = App::build_ai_command(&cfg, AiBackend::Claude, &opts);
        assert_eq!(cmd[0], "claude");
        assert!(cmd.contains(&"--permission-mode".to_string()));
        assert!(cmd.contains(&"auto".to_string()));
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"opus".to_string()));
        assert!(cmd.contains(&"--effort".to_string()));
        assert!(cmd.contains(&"max".to_string()));
        assert!(cmd.contains(&"--name".to_string()));
        assert!(cmd.contains(&"session1".to_string()));
        assert!(cmd.contains(&"--worktree".to_string()));
        assert!(cmd.contains(&"feat".to_string()));
    }

    #[test]
    fn test_opencode_backend_uses_own_command_without_claude_flags() {
        let cfg = Config::default();
        let opts = StartupOptions {
            permission_mode: ClaudePermissionMode::Auto,
            model: ClaudeModel::Opus,
            effort: ClaudeEffort::High,
            ..base_opts()
        };
        let cmd = App::build_ai_command(&cfg, AiBackend::OpenCode, &opts);
        assert_eq!(cmd[0], "opencode");
        // Claude-only flags must never leak into non-Claude backends.
        assert!(!cmd.iter().any(|a| a == "--permission-mode"));
        assert!(!cmd.iter().any(|a| a == "--model"));
        assert!(!cmd.iter().any(|a| a == "--effort"));
    }

    #[test]
    fn test_pi_backend_uses_own_command() {
        let cfg = Config::default();
        let cmd = App::build_ai_command(&cfg, AiBackend::Pi, &base_opts());
        assert_eq!(cmd[0], "pi");
    }

    #[test]
    fn test_codex_backend_uses_own_command_without_claude_flags() {
        let cfg = Config::default();
        let opts = StartupOptions {
            permission_mode: ClaudePermissionMode::Auto,
            model: ClaudeModel::Opus,
            effort: ClaudeEffort::High,
            ..base_opts()
        };
        let cmd = App::build_ai_command(&cfg, AiBackend::Codex, &opts);
        assert_eq!(cmd[0], "codex");
        // Claude-only flags must never leak into non-Claude backends.
        assert!(!cmd.iter().any(|a| a == "--permission-mode"));
        assert!(!cmd.iter().any(|a| a == "--model"));
        assert!(!cmd.iter().any(|a| a == "--effort"));
    }

    #[test]
    fn test_antigravity_backend_uses_agy_command() {
        let cfg = Config::default();
        let cmd = App::build_ai_command(&cfg, AiBackend::Antigravity, &base_opts());
        assert_eq!(cmd, ["agy"]);
    }

    #[test]
    fn test_agent_profile_args_are_appended_for_one_invocation() {
        let cfg = Config::default();
        let args = vec!["--sandbox".to_string(), "read-only".to_string()];
        let cmd = App::build_agent_command(&cfg, AiBackend::Codex, &args);
        assert_eq!(cmd, ["codex", "--sandbox", "read-only"]);
        assert_eq!(cfg.pty.codex_command, ["codex"]);
    }

    #[test]
    fn test_profile_args_do_not_leak_into_shell_fallback() {
        let mut cfg = Config::default();
        cfg.pty.pi_command.clear();
        let cmd = App::build_agent_command(&cfg, AiBackend::Pi, &["--continue".to_string()]);
        assert_eq!(cmd[0], cfg.terminal.shell_path);
        assert!(!cmd.iter().any(|arg| arg == "--continue"));
    }

    #[test]
    fn test_ollama_opencode_backend_uses_configured_command() {
        let mut cfg = Config::default();
        cfg.pty.ollama_opencode_command = vec![
            "ollama".to_string(),
            "launch".to_string(),
            "opencode".to_string(),
            "--model".to_string(),
            "kimi-k2.7-code:cloud".to_string(),
        ];
        let opts = base_opts();
        let cmd = App::build_ai_command(&cfg, AiBackend::OllamaOpenCode, &opts);
        assert_eq!(cmd[0], "ollama");
        assert!(cmd.contains(&"launch".to_string()));
        assert!(cmd.contains(&"opencode".to_string()));
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"kimi-k2.7-code:cloud".to_string()));
        assert!(!cmd.iter().any(|a| a == "--permission-mode"));
    }

    #[test]
    fn test_ollama_pi_backend_uses_configured_command() {
        let mut cfg = Config::default();
        cfg.pty.ollama_pi_command = vec![
            "ollama".to_string(),
            "launch".to_string(),
            "pi".to_string(),
            "--model".to_string(),
            "qwen3.5:cloud".to_string(),
        ];
        let opts = base_opts();
        let cmd = App::build_ai_command(&cfg, AiBackend::OllamaPi, &opts);
        assert_eq!(cmd[0], "ollama");
        assert!(cmd.contains(&"launch".to_string()));
        assert!(cmd.contains(&"pi".to_string()));
        assert!(cmd.contains(&"--model".to_string()));
        assert!(cmd.contains(&"qwen3.5:cloud".to_string()));
        assert!(!cmd.iter().any(|a| a == "--permission-mode"));
    }
}
