//! Central choke-point for "open this file/directory locally".
//!
//! On a local session this behaves exactly as before (`xdg-open`/`open`/a
//! configured browser). On an SSH session, opening on the *server* is wrong
//! and corrupts the TUI, so instead the file is streamed to the user's Mac
//! terminal via iTerm2 OSC 1337 (see [`crate::browser::iterm_transfer`]).
//! Terminals without transfer support (e.g. Terminus/Tabby) fall back to
//! leaving the file on disk and reporting its path — never spawning a
//! console browser that would hijack the TTY.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::browser::{iterm_transfer, opener};

/// What is being opened — drives the fallback behavior when no terminal
/// transfer is possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    /// Preview (`o`): ephemeral HTML/text conversion or an image/PDF.
    Preview,
    /// Export (`Ctrl+X` single file): already written to `export_dir`.
    Export,
    /// Directory (`O` / batch export): a folder, cannot be transferred.
    Directory,
}

/// Result of an open/transfer attempt.
#[derive(Debug, Clone)]
pub enum OpenOutcome {
    /// Opened locally as before (not an SSH session).
    OpenedLocally,
    /// Streamed to the user's Mac via OSC 1337 (landed in `~/Downloads`).
    Transferred,
    /// Not transferred — the file stays on disk at this path.
    KeptWithPath(PathBuf),
    /// An error occurred (I/O, spawn, or transfer failure).
    Error(String),
}

/// Which terminal-side file-transfer protocol the local terminal supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferCapability {
    /// iTerm2 — OSC 1337 File download.
    Iterm2,
    /// WezTerm — also speaks iTerm2 OSC 1337.
    WezTerm,
    /// Kitty — different protocol, not implemented here (treated as fallback).
    Kitty,
    /// No known transfer support (Terminus/Tabby, plain xterm, …).
    None,
}

/// Pure capability detection from environment values (testable in isolation).
fn detect_transfer_capability_from(
    term_program: Option<&str>,
    lc_terminal: Option<&str>,
    wezterm_pane: Option<&str>,
    kitty_window_id: Option<&str>,
    term: Option<&str>,
) -> TransferCapability {
    // WEZTERM_PANE is the most reliable WezTerm marker (TERM_PROGRAM is often
    // stripped across SSH). Check it before iTerm2 markers.
    if wezterm_pane.is_some() || term_program == Some("WezTerm") {
        return TransferCapability::WezTerm;
    }
    if term_program == Some("iTerm.app") || lc_terminal == Some("iTerm2") {
        return TransferCapability::Iterm2;
    }
    if kitty_window_id.is_some() || term == Some("xterm-kitty") {
        return TransferCapability::Kitty;
    }
    TransferCapability::None
}

/// Detect the local terminal's transfer capability from the live environment.
pub fn detect_transfer_capability() -> TransferCapability {
    detect_transfer_capability_from(
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("LC_TERMINAL").ok().as_deref(),
        std::env::var_os("WEZTERM_PANE").is_some().then_some("1"),
        std::env::var_os("KITTY_WINDOW_ID").is_some().then_some("1"),
        std::env::var("TERM").ok().as_deref(),
    )
}

/// Resolve the effective capability, honoring the `ui.remote_transfer` config
/// override ("auto" | "off" | "iterm2" | "wezterm").
pub fn effective_capability(mode: &str) -> TransferCapability {
    match mode {
        "off" => TransferCapability::None,
        "iterm2" => TransferCapability::Iterm2,
        "wezterm" => TransferCapability::WezTerm,
        // "auto" or any unknown value -> environment detection
        _ => detect_transfer_capability(),
    }
}

/// Open `path` locally, or transfer it to the user's Mac when running over SSH.
///
/// - `kind`: controls the fallback when no transfer is possible.
/// - `browser`: `config.ui.browser` (only used on the local path).
/// - `mode`: `config.ui.remote_transfer`.
pub fn open_or_transfer(path: &Path, kind: OpenKind, browser: &str, mode: &str) -> OpenOutcome {
    // Local session: unchanged behavior.
    if !crate::clipboard::is_ssh_session() {
        let result = match kind {
            OpenKind::Directory => opener::open_in_file_manager(path),
            OpenKind::Preview | OpenKind::Export => opener::open_file_with_browser(path, browser),
        };
        return match result {
            Ok(()) => OpenOutcome::OpenedLocally,
            Err(e) => OpenOutcome::Error(e.to_string()),
        };
    }

    // Remote session: a directory cannot be streamed — leave it on the server.
    if matches!(kind, OpenKind::Directory) {
        return OpenOutcome::KeptWithPath(path.to_path_buf());
    }

    match effective_capability(mode) {
        TransferCapability::Iterm2 | TransferCapability::WezTerm => match transfer_file(path) {
            Ok(()) => OpenOutcome::Transferred,
            Err(e) => OpenOutcome::Error(e),
        },
        // Kitty (different protocol) and None -> keep on disk, report path.
        TransferCapability::Kitty | TransferCapability::None => {
            OpenOutcome::KeptWithPath(path.to_path_buf())
        }
    }
}

/// Read `path` and write its iTerm2 transfer sequence(s) to the host stdout.
fn transfer_file(path: &Path) -> Result<(), String> {
    let content = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    if content.len() > iterm_transfer::MAX_TRANSFER_BYTES {
        return Err(format!(
            "file too large for terminal transfer ({} bytes)",
            content.len()
        ));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let tmux = std::env::var_os("TMUX").is_some();

    // Single locked stdout handle for the whole transfer so no ratatui frame
    // interleaves. Runs on the main thread between draws (see module docs).
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();

    if content.len() <= iterm_transfer::MULTIPART_THRESHOLD {
        let seq = iterm_transfer::build_file_sequence(name, &content);
        write_seq(&mut lock, &seq, tmux)?;
    } else {
        for seq in iterm_transfer::build_multipart_sequences(name, &content) {
            write_seq(&mut lock, &seq, tmux)?;
        }
    }
    lock.flush().map_err(|e| e.to_string())
}

fn write_seq(out: &mut impl Write, seq: &[u8], tmux: bool) -> Result<(), String> {
    if tmux {
        let wrapped = iterm_transfer::wrap_for_multiplexer(seq);
        out.write_all(&wrapped).map_err(|e| e.to_string())
    } else {
        out.write_all(seq).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_iterm2_term_program() {
        assert_eq!(
            detect_transfer_capability_from(Some("iTerm.app"), None, None, None, None),
            TransferCapability::Iterm2
        );
    }

    #[test]
    fn test_detect_iterm2_lc_terminal_variant() {
        assert_eq!(
            detect_transfer_capability_from(
                None,
                Some("iTerm2"),
                None,
                None,
                Some("xterm-256color")
            ),
            TransferCapability::Iterm2
        );
    }

    #[test]
    fn test_detect_wezterm_pane() {
        assert_eq!(
            detect_transfer_capability_from(None, None, Some("1"), None, None),
            TransferCapability::WezTerm
        );
    }

    #[test]
    fn test_detect_wezterm_precedence_over_iterm_markers() {
        // If both are somehow set, WezTerm wins (checked first).
        assert_eq!(
            detect_transfer_capability_from(
                Some("iTerm.app"),
                Some("iTerm2"),
                Some("1"),
                None,
                None
            ),
            TransferCapability::WezTerm
        );
    }

    #[test]
    fn test_detect_kitty() {
        assert_eq!(
            detect_transfer_capability_from(None, None, None, Some("1"), None),
            TransferCapability::Kitty
        );
        assert_eq!(
            detect_transfer_capability_from(None, None, None, None, Some("xterm-kitty")),
            TransferCapability::Kitty
        );
    }

    #[test]
    fn test_detect_none_plain_xterm() {
        assert_eq!(
            detect_transfer_capability_from(None, None, None, None, Some("xterm-256color")),
            TransferCapability::None
        );
    }

    #[test]
    fn test_detect_none_tabby_like() {
        // Terminus/Tabby: no iTerm/WezTerm/Kitty marker -> None (safe fallback).
        assert_eq!(
            detect_transfer_capability_from(
                Some("Tabby"),
                None,
                None,
                None,
                Some("xterm-256color")
            ),
            TransferCapability::None
        );
    }

    #[test]
    fn test_effective_capability_overrides() {
        assert_eq!(effective_capability("off"), TransferCapability::None);
        assert_eq!(effective_capability("iterm2"), TransferCapability::Iterm2);
        assert_eq!(effective_capability("wezterm"), TransferCapability::WezTerm);
    }
}
