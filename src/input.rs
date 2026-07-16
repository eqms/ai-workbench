use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::types::PaneId;

/// Translate a crossterm key event into the byte sequence to write to a PTY.
/// Pane-agnostic except for one documented asymmetry: Shift+Enter is only
/// remapped in the AI pane (see below).
pub fn map_key_to_pty(key: KeyEvent, pane: PaneId) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();

    // Shift+Enter (AI pane only) and Alt+Enter (any pane) → ESC+CR.
    // Claude Code / OpenCode interpret ESC+CR as "insert newline" in legacy
    // keyboard mode — the mode the inner PTY always runs in, since the vt100
    // parser never answers kitty-protocol queries. SHIFT on Enter only
    // arrives when the outer terminal supports DISAMBIGUATE_ESCAPE_CODES
    // (pushed in main.rs); without it this branch is unreachable and the
    // `\` + Enter fallback stays the only path. Shell/LazyGit panes don't
    // treat ESC+CR as newline, so Shift+Enter is left alone there.
    if key.code == KeyCode::Enter
        && (key.modifiers.contains(KeyModifiers::ALT)
            || (pane == PaneId::Claude && key.modifiers.contains(KeyModifiers::SHIFT)))
    {
        return Some(vec![0x1b, b'\r']);
    }

    // Handle Alt + Arrow keys for word navigation
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Left => return Some(vec![0x1b, b'b']), // ESC b = word back
            KeyCode::Right => return Some(vec![0x1b, b'f']), // ESC f = word forward
            _ => {}
        }
    }

    // Handle Control + Char
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let ch = c.to_ascii_lowercase();
            // a=1 ... z=26
            if ch.is_ascii_lowercase() {
                bytes.push(ch as u8 - b'a' + 1);
                return Some(bytes);
            }
            match c {
                '[' => return Some(vec![27]),
                '\\' => return Some(vec![28]),
                ']' => return Some(vec![29]),
                '^' => return Some(vec![30]),
                '_' => return Some(vec![31]),
                '?' => return Some(vec![127]),
                _ => {}
            }
        }
    }

    match key.code {
        KeyCode::Char(c) => bytes.extend_from_slice(c.to_string().as_bytes()),
        KeyCode::Enter => bytes.push(b'\r'),
        KeyCode::Backspace => bytes.push(127),
        KeyCode::Tab => bytes.push(9),
        KeyCode::BackTab => bytes.extend_from_slice(b"\x1b[Z"),
        KeyCode::Esc => bytes.push(27),

        KeyCode::Up => bytes.extend_from_slice(b"\x1b[A"),
        KeyCode::Down => bytes.extend_from_slice(b"\x1b[B"),
        KeyCode::Right => bytes.extend_from_slice(b"\x1b[C"),
        KeyCode::Left => bytes.extend_from_slice(b"\x1b[D"),

        KeyCode::Home => bytes.extend_from_slice(b"\x1b[H"),
        KeyCode::End => bytes.extend_from_slice(b"\x1b[F"),

        // REMAPPED: PageUp → Home (line start) for better CLI editing
        KeyCode::PageUp => bytes.extend_from_slice(b"\x1b[H"),
        // REMAPPED: PageDown → End (line end) for better CLI editing
        KeyCode::PageDown => bytes.extend_from_slice(b"\x1b[F"),
        KeyCode::Delete => bytes.extend_from_slice(b"\x1b[3~"),
        KeyCode::Insert => bytes.extend_from_slice(b"\x1b[2~"),

        // Function keys (xterm sequences) — required so TUI apps running in the
        // terminal pane (nano, mc, vim) receive F1-F12 when passthrough forwards
        // them. F1-F4 use SS3, F5-F12 use CSI, matching xterm terminfo.
        KeyCode::F(1) => bytes.extend_from_slice(b"\x1bOP"),
        KeyCode::F(2) => bytes.extend_from_slice(b"\x1bOQ"),
        KeyCode::F(3) => bytes.extend_from_slice(b"\x1bOR"),
        KeyCode::F(4) => bytes.extend_from_slice(b"\x1bOS"),
        KeyCode::F(5) => bytes.extend_from_slice(b"\x1b[15~"),
        KeyCode::F(6) => bytes.extend_from_slice(b"\x1b[17~"),
        KeyCode::F(7) => bytes.extend_from_slice(b"\x1b[18~"),
        KeyCode::F(8) => bytes.extend_from_slice(b"\x1b[19~"),
        KeyCode::F(9) => bytes.extend_from_slice(b"\x1b[20~"),
        KeyCode::F(10) => bytes.extend_from_slice(b"\x1b[21~"),
        KeyCode::F(11) => bytes.extend_from_slice(b"\x1b[23~"),
        KeyCode::F(12) => bytes.extend_from_slice(b"\x1b[24~"),

        _ => return None,
    }

    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn shift_enter_claude_pane_inserts_newline_escape() {
        let k = key(KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!(map_key_to_pty(k, PaneId::Claude), Some(vec![0x1b, b'\r']));
    }

    #[test]
    fn shift_enter_terminal_and_lazygit_panes_unchanged() {
        let k = key(KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!(map_key_to_pty(k, PaneId::Terminal), Some(vec![b'\r']));
        assert_eq!(map_key_to_pty(k, PaneId::LazyGit), Some(vec![b'\r']));
    }

    #[test]
    fn alt_enter_inserts_newline_escape_in_any_pane() {
        let k = key(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(map_key_to_pty(k, PaneId::Claude), Some(vec![0x1b, b'\r']));
        assert_eq!(map_key_to_pty(k, PaneId::Terminal), Some(vec![0x1b, b'\r']));
    }

    #[test]
    fn plain_enter_unchanged() {
        let k = key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(map_key_to_pty(k, PaneId::Claude), Some(vec![b'\r']));
    }

    // Regression guard for the DISAMBIGUATE_ESCAPE_CODES rollout: the
    // legacy-ambiguous control combos must keep producing the same single
    // control byte regardless of how the outer terminal reports them.
    #[test]
    fn ctrl_letter_combos_produce_legacy_control_bytes() {
        let cases = [('h', 8u8), ('i', 9), ('m', 13)];
        for (c, byte) in cases {
            assert_eq!(
                map_key_to_pty(key(KeyCode::Char(c), KeyModifiers::CONTROL), PaneId::Claude),
                Some(vec![byte])
            );
        }
        assert_eq!(
            map_key_to_pty(
                key(KeyCode::Char('['), KeyModifiers::CONTROL),
                PaneId::Claude
            ),
            Some(vec![27])
        );
    }

    #[test]
    fn alt_left_right_word_nav_still_works() {
        assert_eq!(
            map_key_to_pty(key(KeyCode::Left, KeyModifiers::ALT), PaneId::Claude),
            Some(vec![0x1b, b'b'])
        );
        assert_eq!(
            map_key_to_pty(key(KeyCode::Right, KeyModifiers::ALT), PaneId::Claude),
            Some(vec![0x1b, b'f'])
        );
    }
}
