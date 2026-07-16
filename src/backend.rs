//! AI backend selection.
//!
//! AI Workbench drives one of several AI coding-agent CLIs in its primary
//! (AI) pane. The concrete backend is chosen via a positional CLI argument
//! (`ai-workbench claude|opencode|pi|codex`) and persisted across runs. Every
//! other pane (file browser, preview, LazyGit, terminal) is backend-agnostic.

use serde::{Deserialize, Serialize};

/// The AI coding agent driven in the primary pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiBackend {
    /// Anthropic Claude Code CLI (`claude`). Supports permission/model/effort flags.
    #[default]
    Claude,
    /// OpenCode CLI (`opencode`).
    OpenCode,
    /// Pi CLI (`pi`).
    Pi,
    /// OpenAI Codex CLI (`codex`).
    Codex,
}

impl AiBackend {
    /// Parse a user-supplied backend name, case-insensitively.
    /// Accepts e.g. "claude", "Claude", "opencode", "OpenCode", "pi", "codex".
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            "pi" => Some(Self::Pi),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    /// All backends, in display order.
    pub fn all() -> [Self; 4] {
        [Self::Claude, Self::OpenCode, Self::Pi, Self::Codex]
    }

    /// The next backend in the cycle (wraps Codex → Claude). Drives the F8 switch.
    pub fn next(self) -> Self {
        match self {
            Self::Claude => Self::OpenCode,
            Self::OpenCode => Self::Pi,
            Self::Pi => Self::Codex,
            Self::Codex => Self::Claude,
        }
    }

    /// The default executable name looked up on `$PATH`.
    pub fn binary_name(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Codex => "codex",
        }
    }

    /// Canonical lowercase identifier (used for CLI parsing / persistence).
    pub fn id(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Codex => "codex",
        }
    }

    /// Title shown on the AI pane border (with surrounding spaces).
    pub fn pane_title(&self) -> &'static str {
        match self {
            Self::Claude => " Claude Code ",
            Self::OpenCode => " OpenCode ",
            Self::Pi => " Pi ",
            Self::Codex => " Codex ",
        }
    }

    /// Short label shown in the footer hotkey row.
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::OpenCode => "OpenCode",
            Self::Pi => "Pi",
            Self::Codex => "Codex",
        }
    }

    /// Whether this backend understands the Claude-specific startup flags
    /// (`--permission-mode`, `--model`, `--effort`, `--name`, `--worktree`,
    /// `--remote-control`, `--dangerously-skip-permissions`). Only Claude does.
    /// Codex has its own flag set (`-s`, `-a`, `-m`, `--search`) which users
    /// configure via `pty.codex_command` instead of a startup dialog.
    pub fn supports_claude_flags(&self) -> bool {
        matches!(self, Self::Claude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_is_case_insensitive() {
        assert_eq!(AiBackend::parse("claude"), Some(AiBackend::Claude));
        assert_eq!(AiBackend::parse("Claude"), Some(AiBackend::Claude));
        assert_eq!(AiBackend::parse("OpenCode"), Some(AiBackend::OpenCode));
        assert_eq!(AiBackend::parse("opencode"), Some(AiBackend::OpenCode));
        assert_eq!(AiBackend::parse("Pi"), Some(AiBackend::Pi));
        assert_eq!(AiBackend::parse("  pi  "), Some(AiBackend::Pi));
        assert_eq!(AiBackend::parse("Codex"), Some(AiBackend::Codex));
        assert_eq!(AiBackend::parse("  codex  "), Some(AiBackend::Codex));
        assert_eq!(AiBackend::parse("gpt"), None);
    }

    #[test]
    fn default_is_claude() {
        assert_eq!(AiBackend::default(), AiBackend::Claude);
    }

    #[test]
    fn next_cycles_and_wraps() {
        assert_eq!(AiBackend::Claude.next(), AiBackend::OpenCode);
        assert_eq!(AiBackend::OpenCode.next(), AiBackend::Pi);
        assert_eq!(AiBackend::Pi.next(), AiBackend::Codex);
        assert_eq!(AiBackend::Codex.next(), AiBackend::Claude);
    }

    #[test]
    fn only_claude_supports_flags() {
        assert!(AiBackend::Claude.supports_claude_flags());
        assert!(!AiBackend::OpenCode.supports_claude_flags());
        assert!(!AiBackend::Pi.supports_claude_flags());
        assert!(!AiBackend::Codex.supports_claude_flags());
    }
}
