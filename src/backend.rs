//! AI backend selection.
//!
//! AI Workbench drives one of several AI coding-agent CLIs in its primary
//! (AI) pane. The concrete backend is chosen via a positional CLI argument
//! (`ai-workbench claude|codex|antigravity|opencode|pi|ollama-opencode|ollama-pi`) and
//! persisted across runs. Every other pane (file browser, preview, LazyGit,
//! terminal) is backend-agnostic.

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
    /// Google Antigravity CLI (`agy`).
    Antigravity,
    /// OpenCode launched via Ollama (`ollama launch opencode ...`).
    #[serde(rename = "ollama-opencode")]
    OllamaOpenCode,
    /// Pi launched via Ollama (`ollama launch pi ...`).
    #[serde(rename = "ollama-pi")]
    OllamaPi,
}

impl AiBackend {
    /// Parse a user-supplied backend name, case-insensitively.
    /// Accepts e.g. "claude", "opencode", "ollama-opencode", "ollama-pi", ...
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            "pi" => Some(Self::Pi),
            "codex" => Some(Self::Codex),
            "antigravity" | "agy" => Some(Self::Antigravity),
            "ollama-opencode" => Some(Self::OllamaOpenCode),
            "ollama-pi" => Some(Self::OllamaPi),
            _ => None,
        }
    }

    /// All backends, in display order.
    pub fn all() -> [Self; 7] {
        [
            Self::Claude,
            Self::Codex,
            Self::Antigravity,
            Self::OpenCode,
            Self::Pi,
            Self::OllamaOpenCode,
            Self::OllamaPi,
        ]
    }

    /// The next backend in the cycle (wraps OllamaPi → Claude). Drives the F8 switch.
    pub fn next(self) -> Self {
        match self {
            Self::Claude => Self::Codex,
            Self::Codex => Self::Antigravity,
            Self::Antigravity => Self::OpenCode,
            Self::OpenCode => Self::Pi,
            Self::Pi => Self::OllamaOpenCode,
            Self::OllamaOpenCode => Self::OllamaPi,
            Self::OllamaPi => Self::Claude,
        }
    }

    /// The default executable name looked up on `$PATH`.
    /// For Ollama variants this is the wrapper binary name used as a fallback.
    pub fn binary_name(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Codex => "codex",
            Self::Antigravity => "agy",
            Self::OllamaOpenCode => "ollama",
            Self::OllamaPi => "ollama",
        }
    }

    /// Canonical lowercase identifier (used for CLI parsing / persistence).
    pub fn id(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
            Self::OllamaOpenCode => "ollama-opencode",
            Self::OllamaPi => "ollama-pi",
        }
    }

    /// Title shown on the AI pane border (with surrounding spaces).
    pub fn pane_title(&self) -> &'static str {
        match self {
            Self::Claude => " Claude Code ",
            Self::OpenCode => " OpenCode ",
            Self::Pi => " Pi ",
            Self::Codex => " Codex ",
            Self::Antigravity => " Antigravity ",
            Self::OllamaOpenCode => " Ollama OpenCode ",
            Self::OllamaPi => " Ollama Pi ",
        }
    }

    /// Short label shown in the footer hotkey row.
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::OpenCode => "OpenCode",
            Self::Pi => "Pi",
            Self::Codex => "Codex",
            Self::Antigravity => "Antigravity",
            Self::OllamaOpenCode => "OllamaOC",
            Self::OllamaPi => "OllamaPi",
        }
    }

    /// Whether this backend understands the Claude-specific startup flags
    /// (`--permission-mode`, `--model`, `--effort`, `--name`, `--worktree`,
    /// `--remote-control`, `--dangerously-skip-permissions`). Only Claude does.
    /// Other CLIs use backend-specific profiles in `ui::agent_startup`.
    pub fn supports_claude_flags(&self) -> bool {
        matches!(self, Self::Claude)
    }

    /// Whether this backend uses a custom Ollama-wrapped command.
    pub fn is_ollama(&self) -> bool {
        matches!(self, Self::OllamaOpenCode | Self::OllamaPi)
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
        assert_eq!(
            AiBackend::parse("Antigravity"),
            Some(AiBackend::Antigravity)
        );
        assert_eq!(AiBackend::parse("agy"), Some(AiBackend::Antigravity));
        assert_eq!(
            AiBackend::parse("ollama-opencode"),
            Some(AiBackend::OllamaOpenCode)
        );
        assert_eq!(AiBackend::parse("OLLAMA-Pi"), Some(AiBackend::OllamaPi));
        assert_eq!(AiBackend::parse("gpt"), None);
    }

    #[test]
    fn default_is_claude() {
        assert_eq!(AiBackend::default(), AiBackend::Claude);
    }

    #[test]
    fn next_cycles_and_wraps() {
        assert_eq!(AiBackend::Claude.next(), AiBackend::Codex);
        assert_eq!(AiBackend::Codex.next(), AiBackend::Antigravity);
        assert_eq!(AiBackend::Antigravity.next(), AiBackend::OpenCode);
        assert_eq!(AiBackend::OpenCode.next(), AiBackend::Pi);
        assert_eq!(AiBackend::Pi.next(), AiBackend::OllamaOpenCode);
        assert_eq!(AiBackend::OllamaOpenCode.next(), AiBackend::OllamaPi);
        assert_eq!(AiBackend::OllamaPi.next(), AiBackend::Claude);
    }

    #[test]
    fn only_claude_supports_flags() {
        assert!(AiBackend::Claude.supports_claude_flags());
        assert!(!AiBackend::OpenCode.supports_claude_flags());
        assert!(!AiBackend::Pi.supports_claude_flags());
        assert!(!AiBackend::Codex.supports_claude_flags());
        assert!(!AiBackend::Antigravity.supports_claude_flags());
        assert!(!AiBackend::OllamaOpenCode.supports_claude_flags());
        assert!(!AiBackend::OllamaPi.supports_claude_flags());
    }

    #[test]
    fn ollama_variants_are_detected() {
        assert!(AiBackend::OllamaOpenCode.is_ollama());
        assert!(AiBackend::OllamaPi.is_ollama());
        assert!(!AiBackend::OpenCode.is_ollama());
    }
}
