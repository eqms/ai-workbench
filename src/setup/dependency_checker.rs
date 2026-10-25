//! Dependency checker for verifying required and optional tools

use std::path::PathBuf;
use std::process::Command;

/// Result of checking a single dependency
#[derive(Debug, Clone, Default)]
pub struct DependencyStatus {
    pub name: String,
    pub found: bool,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub required: bool,
}

/// Clipboard helper binaries detected at startup. Used by the clipboard
/// fallback chain (X11 sessions, esp. XRDP) and rendered in F12 help.
#[derive(Debug, Clone, Default)]
pub struct ClipboardHelpers {
    pub xclip: DependencyStatus,
    pub xsel: DependencyStatus,
    pub wl_copy: DependencyStatus,
    pub wl_paste: DependencyStatus,
}

impl ClipboardHelpers {
    /// Locate the helpers on PATH. Pure-Rust lookup: they are plain binaries,
    /// never shell aliases, so no `--version` probe and no interactive-shell
    /// fallback (which once activated fish job control at startup).
    pub fn detect() -> Self {
        Self {
            xclip: check_binary("xclip"),
            xsel: check_binary("xsel"),
            wl_copy: check_binary("wl-copy"),
            wl_paste: check_binary("wl-paste"),
        }
    }

    /// At least one X11 selection helper is present.
    pub fn linux_x11_ok(&self) -> bool {
        self.xclip.found || self.xsel.found
    }

    /// Both Wayland helpers present (need both for copy + paste).
    pub fn linux_wayland_ok(&self) -> bool {
        self.wl_copy.found && self.wl_paste.found
    }

    /// True if no helper at all is available — Linux fallback to OSC 52 only.
    pub fn none_available(&self) -> bool {
        !self.xclip.found && !self.xsel.found && !self.wl_copy.found && !self.wl_paste.found
    }
}

/// All dependencies checked at startup
#[derive(Debug, Clone, Default)]
pub struct DependencyReport {
    pub git: DependencyStatus,
    pub claude_cli: DependencyStatus,
    pub opencode_cli: DependencyStatus,
    pub pi_cli: DependencyStatus,
    pub codex_cli: DependencyStatus,
    pub antigravity_cli: DependencyStatus,
    pub ollama_cli: DependencyStatus,
    pub lazygit: DependencyStatus,
    pub shells: Vec<DependencyStatus>,
    pub clipboard_helpers: ClipboardHelpers,
}

impl DependencyReport {
    /// Check all dependencies and return report
    pub fn check() -> Self {
        Self {
            git: check_command("git", &["--version"], true),
            claude_cli: check_command("claude", &["--version"], false),
            opencode_cli: check_command("opencode", &["--version"], false),
            pi_cli: check_command("pi", &["--version"], false),
            codex_cli: check_command("codex", &["--version"], false),
            antigravity_cli: check_command("agy", &["--version"], false),
            ollama_cli: check_command("ollama", &["--version"], false),
            lazygit: check_command("lazygit", &["--version"], false),
            shells: check_available_shells(),
            clipboard_helpers: ClipboardHelpers::detect(),
        }
    }

    /// The report the regular (non-wizard) startup needs: only the clipboard
    /// helpers, which feed the Linux clipboard warning. Pure PATH lookups, no
    /// subprocess — the full `check()` costs several seconds of CPU because
    /// every `<cli> --version` of the Node-based agents boots a Node runtime,
    /// and that competed with the AI pane's own startup.
    pub fn clipboard_only() -> Self {
        Self {
            clipboard_helpers: ClipboardHelpers::detect(),
            ..Self::default()
        }
    }

    /// The CLI status for a given AI backend.
    pub fn backend_cli(&self, backend: crate::backend::AiBackend) -> &DependencyStatus {
        use crate::backend::AiBackend;
        match backend {
            AiBackend::Claude => &self.claude_cli,
            AiBackend::OpenCode => &self.opencode_cli,
            AiBackend::Pi => &self.pi_cli,
            AiBackend::Codex => &self.codex_cli,
            AiBackend::Antigravity => &self.antigravity_cli,
            AiBackend::OllamaOpenCode | AiBackend::OllamaPi => &self.ollama_cli,
        }
    }

    /// True if the Ollama wrapper binary is available.
    pub fn ollama_available(&self) -> bool {
        self.ollama_cli.found
    }

    /// Returns true if all required dependencies are met
    pub fn all_required_met(&self) -> bool {
        !self.git.required || self.git.found
    }

    /// Returns true if any optional but recommended dep is missing
    pub fn has_missing_optional(&self) -> bool {
        !self.claude_cli.found || !self.lazygit.found
    }

    /// Get summary counts
    pub fn summary(&self) -> (usize, usize, usize) {
        let mut found = 0;
        let mut missing_required = 0;
        let mut missing_optional = 0;

        // Git
        if self.git.found {
            found += 1;
        } else if self.git.required {
            missing_required += 1;
        }

        // Claude CLI
        if self.claude_cli.found {
            found += 1;
        } else {
            missing_optional += 1;
        }

        // LazyGit
        if self.lazygit.found {
            found += 1;
        } else {
            missing_optional += 1;
        }

        // Shells
        for shell in &self.shells {
            if shell.found {
                found += 1;
            }
        }

        (found, missing_required, missing_optional)
    }
}

/// Lightweight PATH-only existence check for simple binaries (no version
/// query, no interactive-shell fallback). Used for clipboard helpers where
/// invoking the binary at startup just to read a version string was costly
/// on macOS (helpers absent → 4× `fish -i -c` fallback → terminal job
/// control corruption on next launch).
fn check_binary(name: &str) -> DependencyStatus {
    let path = crate::clipboard::which(name);
    DependencyStatus {
        name: name.to_string(),
        found: path.is_some(),
        path,
        version: None,
        required: false,
    }
}

/// Check if a command exists and get its version
fn check_command(name: &str, args: &[&str], required: bool) -> DependencyStatus {
    // First try direct execution
    let result = Command::new(name).args(args).output();

    if let Ok(output) = result {
        if output.status.success() {
            let version = extract_version(&output.stdout, name);
            let path = find_executable_path(name);

            return DependencyStatus {
                name: name.to_string(),
                found: true,
                path,
                version,
                required,
            };
        }
    }

    // Direct execution failed — binary not found on PATH.
    // The interactive-shell fallback ($SHELL -i -c) was removed (SEC-03/WR-02):
    // all probed binaries (git, claude, lazygit, shells) are real executables,
    // not shell functions, on supported systems (Assumption A3). The fallback
    // was an injection surface and also triggered fish job-control side-effects
    // on macOS startup when clipboard helpers were absent.
    DependencyStatus {
        name: name.to_string(),
        found: false,
        path: None,
        version: None,
        required,
    }
}

/// Extract version from command output
fn extract_version(stdout: &[u8], name: &str) -> Option<String> {
    let output = String::from_utf8_lossy(stdout);
    let first_line = output.lines().next()?.trim().to_string();

    // Special handling for Claude Code: "2.0.76 (Claude Code)"
    if name == "claude" {
        // Return just the version number
        if let Some(version) = first_line.split_whitespace().next() {
            return Some(version.to_string());
        }
    }

    Some(first_line)
}

/// Find the executable's path with the pure-Rust PATH walk (no `which`
/// subprocess; the binary was just executed successfully, so it is on PATH).
fn find_executable_path(name: &str) -> Option<PathBuf> {
    if let Some(path) = crate::clipboard::which(name) {
        return Some(path);
    }

    // Fallback: Check common Claude installation locations
    if name == "claude" {
        let common_paths = [
            dirs::home_dir().map(|h| h.join(".claude/local/claude")),
            Some(PathBuf::from("/usr/local/bin/claude")),
            dirs::home_dir().map(|h| h.join(".local/bin/claude")),
        ];
        for path_opt in common_paths.into_iter().flatten() {
            if path_opt.exists() {
                return Some(path_opt);
            }
        }
    }

    None
}

/// Check for available shells
fn check_available_shells() -> Vec<DependencyStatus> {
    #[cfg(windows)]
    let candidates: &[&str] = &["pwsh", "powershell", "cmd"];
    #[cfg(not(windows))]
    let candidates: &[&str] = &["bash", "zsh", "fish", "sh"];

    candidates
        .iter()
        .map(|shell| {
            // `cmd /?` prints help and exits 0; `pwsh --version` works on PS7+.
            // For Windows-classic `cmd.exe` we use `/?` to avoid spawning a sub-shell.
            #[cfg(windows)]
            let args: &[&str] = if *shell == "cmd" {
                &["/?"]
            } else {
                &["--version"]
            };
            #[cfg(not(windows))]
            let args: &[&str] = &["--version"];
            check_command(shell, args, false)
        })
        .filter(|s| s.found)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_check() {
        let report = DependencyReport::check();
        // Git should be available in most dev environments
        // This is more of an integration test
        println!("Git found: {}", report.git.found);
        println!("Claude CLI found: {}", report.claude_cli.found);
        println!("LazyGit found: {}", report.lazygit.found);
        println!(
            "Shells found: {:?}",
            report.shells.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn clipboard_only_report_skips_tool_probes() {
        let report = DependencyReport::clipboard_only();
        // No `--version` probes: every tool stays at its default (not found),
        // while the clipboard helpers were looked up.
        assert!(!report.git.found);
        assert!(!report.claude_cli.found);
        assert!(report.shells.is_empty());
        assert_eq!(report.clipboard_helpers.xclip.name, "xclip");
        assert_eq!(report.clipboard_helpers.wl_paste.name, "wl-paste");
    }

    #[test]
    fn find_executable_path_uses_pure_rust_lookup() {
        // `git` is on PATH in every environment that runs the tests; the
        // lookup must return an absolute, existing path without spawning
        // `which`.
        let path = find_executable_path("git").expect("git on PATH");
        assert!(path.is_absolute() && path.exists());
        assert!(find_executable_path("nonexistent_command_12345").is_none());
    }

    #[test]
    fn test_check_command_not_found() {
        let status = check_command("nonexistent_command_12345", &["--version"], false);
        assert!(!status.found);
        assert!(status.path.is_none());
    }

    #[test]
    fn test_check_command_finds_git_directly() {
        // git must be found via direct exec, not shell fallback
        let status = check_command("git", &["--version"], true);
        assert!(status.found, "git should be findable via direct exec");
        assert!(status.version.is_some(), "git version should be captured");
    }

    #[test]
    fn test_check_command_returns_false_for_nonexistent() {
        let status = check_command("__nonexistent_binary_xyz_abc__", &[], false);
        assert!(!status.found, "nonexistent binary should not be found");
        assert!(status.path.is_none());
    }
}
