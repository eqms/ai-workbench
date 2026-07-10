//! iTerm2 OSC 1337 file-transfer escape sequences.
//!
//! When the workbench runs on a remote (SSH) host, the exported/previewed
//! file lives on the *server* — `xdg-open` there is useless and, worse, can
//! grab the TUI's TTY. Instead we stream the file to the user's Mac terminal
//! (iTerm2 / WezTerm) via iTerm2's proprietary "File Download" escape code,
//! which lands the bytes in the local `~/Downloads` folder over the existing
//! SSH connection — no daemon, no port forwarding.
//!
//! This module contains only pure sequence builders (no I/O) so the encoding
//! is unit-testable in isolation. The actual writing to the host terminal is
//! done by [`crate::browser::remote_open`], mirroring how `clipboard::osc52_copy`
//! writes raw escapes to `stdout` alongside the ratatui backend.

/// Unencoded bytes per multipart chunk. ~200 KiB keeps each `FilePart`
/// escape well under any known terminal single-OSC buffer limit (~270 KiB
/// after base64). Value follows common `imgcat`/iTerm2 practice.
pub const CHUNK_SIZE: usize = 200 * 1024;

/// Files at or below this size are sent as a single `File=` sequence; larger
/// files use the `MultipartFile`/`FilePart`/`FileEnd` protocol.
pub const MULTIPART_THRESHOLD: usize = 1024 * 1024;

/// Hard upper bound: refuse to stream more than this over the TTY. base64
/// inflates payloads ~4/3 and a multi-second blocking write would freeze the
/// UI; above this the caller falls back to leaving the file on disk.
pub const MAX_TRANSFER_BYTES: usize = 25 * 1024 * 1024;

/// Build a single-shot `File=` download sequence for `content`.
///
/// `\x1b]1337;File=name=<b64name>;size=<len>;inline=0:<b64content>\x07`
pub fn build_file_sequence(name: &str, content: &[u8]) -> Vec<u8> {
    let b64_name = crate::clipboard::base64_encode_bytes(name.as_bytes());
    let b64_content = crate::clipboard::base64_encode_bytes(content);
    format!(
        "\x1b]1337;File=name={};size={};inline=0:{}\x07",
        b64_name,
        content.len(),
        b64_content
    )
    .into_bytes()
}

/// Build the multipart sequence list (`MultipartFile` header, N × `FilePart`,
/// `FileEnd`) for files larger than [`MULTIPART_THRESHOLD`].
pub fn build_multipart_sequences(name: &str, content: &[u8]) -> Vec<Vec<u8>> {
    let b64_name = crate::clipboard::base64_encode_bytes(name.as_bytes());
    let mut seqs = Vec::new();
    seqs.push(
        format!(
            "\x1b]1337;MultipartFile=name={};size={};inline=0\x07",
            b64_name,
            content.len()
        )
        .into_bytes(),
    );
    for chunk in content.chunks(CHUNK_SIZE) {
        let b64_chunk = crate::clipboard::base64_encode_bytes(chunk);
        seqs.push(format!("\x1b]1337;FilePart={}\x07", b64_chunk).into_bytes());
    }
    seqs.push(b"\x1b]1337;FileEnd\x07".to_vec());
    seqs
}

/// Wrap a single escape sequence for tmux passthrough (used when `$TMUX` is
/// set). tmux otherwise interprets/swallows the OSC. Each `ESC` inside the
/// payload is doubled, the whole block is wrapped in a DCS `tmux;` … `ST`.
///
/// Note: tmux ≥ 3.3 additionally requires `set -g allow-passthrough on`; we
/// cannot detect that, only wrap correctly. GNU `screen` uses an incompatible
/// passthrough and is treated as unsupported by the caller.
pub fn wrap_for_multiplexer(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() + 8);
    out.extend_from_slice(b"\x1bPtmux;");
    for &b in seq {
        if b == 0x1b {
            out.push(0x1b);
        }
        out.push(b);
    }
    out.extend_from_slice(b"\x1b\\");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_file_sequence_matches_known_bytes() {
        // base64("a.txt") = "YS50eHQ=", base64("hi") = "aGk="
        let seq = build_file_sequence("a.txt", b"hi");
        assert_eq!(
            seq,
            b"\x1b]1337;File=name=YS50eHQ=;size=2;inline=0:aGk=\x07".to_vec()
        );
    }

    #[test]
    fn test_build_file_sequence_empty_content() {
        let seq = build_file_sequence("x", b"");
        // base64("x") = "eA=="
        assert_eq!(
            seq,
            b"\x1b]1337;File=name=eA==;size=0;inline=0:\x07".to_vec()
        );
    }

    #[test]
    fn test_multipart_exact_boundary() {
        // Exactly N*CHUNK_SIZE bytes -> N FileParts, no empty trailing part.
        let content = vec![0u8; CHUNK_SIZE * 2];
        let seqs = build_multipart_sequences("big.pdf", &content);
        // header + 2 parts + end
        assert_eq!(seqs.len(), 1 + 2 + 1);
    }

    #[test]
    fn test_multipart_remainder() {
        let content = vec![0u8; CHUNK_SIZE * 2 + 1];
        let seqs = build_multipart_sequences("big.pdf", &content);
        // header + 3 parts (last is 1 byte) + end
        assert_eq!(seqs.len(), 1 + 3 + 1);
    }

    #[test]
    fn test_multipart_header_and_footer_shape() {
        let content = vec![7u8; CHUNK_SIZE + 10];
        let seqs = build_multipart_sequences("r.pdf", &content);
        assert!(seqs
            .first()
            .unwrap()
            .starts_with(b"\x1b]1337;MultipartFile=name="));
        assert!(seqs[1].starts_with(b"\x1b]1337;FilePart="));
        assert_eq!(seqs.last().unwrap(), b"\x1b]1337;FileEnd\x07");
    }

    #[test]
    fn test_wrap_for_multiplexer_doubles_escapes() {
        let seq = b"\x1b]1337;FileEnd\x07";
        let wrapped = wrap_for_multiplexer(seq);
        assert!(wrapped.starts_with(b"\x1bPtmux;"));
        assert!(wrapped.ends_with(b"\x1b\\"));
        // The single ESC inside the payload must be doubled.
        assert!(wrapped.windows(2).any(|w| w == b"\x1b\x1b"));
    }
}
