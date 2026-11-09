//! Program status relay (OSC 7501) for the AI pane.
//!
//! Terminal hosts such as Herdr list coding agents in a sidebar. They learn
//! about an agent only through program status reports the agent writes to
//! its terminal: `ESC ] 7501 ; state=working:app=claude-code ST`. Claude Code
//! first asks `ESC ] 7501 ; ?` and sends reports only when the terminal
//! answers. Inside the workbench the agent talks to our embedded vt100
//! parser, which neither answered nor forwarded anything — so the agent
//! vanished from the host's list.
//!
//! The relay closes that gap in three steps:
//! 1. At startup, [`probe_host`] asks the real terminal whether it speaks the
//!    protocol (OSC 7501 query followed by DA1, which every terminal
//!    answers — no fixed timeout on terminals that support neither).
//! 2. The AI pane's PTY answers the agent's query with the host's own reply.
//! 3. Reports from the AI pane are queued and written to the host by the UI
//!    thread between two frames, so they never land inside a frame.
//!
//! Nothing happens when the host does not support the protocol: queries stay
//! unanswered, as before, and no bytes reach the host.
//!
//! Specification: <https://www.superlogical.com/rex/docs/build/program-status>

use std::io::Write;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Upper bound for a relayed payload. Real reports are a few hundred bytes;
/// the cap keeps a misbehaving pane from pushing megabytes at the host.
const MAX_PAYLOAD: usize = 4096;

/// The host's reply to `OSC 7501 ; ?`, without the `7501;` prefix (e.g.
/// `?:states=idle,working,done,blocked,error:kinds=permission,question,auth`).
/// `None` means the host does not support the protocol (or was never probed).
static HOST_REPLY: OnceLock<Option<String>> = OnceLock::new();

/// The host's capability reply, if the startup probe found support.
pub fn host_reply() -> Option<&'static str> {
    HOST_REPLY.get().and_then(|r| r.as_deref())
}

/// Record the probe result. Only the first call wins.
pub fn set_host_reply(reply: Option<String>) {
    let _ = HOST_REPLY.set(reply);
}

/// Build a complete OSC 7501 sequence, ST-terminated.
pub fn osc_sequence(payload: &str) -> Vec<u8> {
    format!("\x1b]7501;{payload}\x1b\\").into_bytes()
}

/// Sequence that removes every record this terminal reported. Sent when the
/// agent behind the relayed reports is gone, so the host does not keep
/// showing a stale state.
pub fn clear_sequence() -> Vec<u8> {
    osc_sequence("state=clear")
}

/// Accept a payload only if it is short, printable ASCII. The protocol uses
/// `key=value` pairs and base64 for free text, so anything else is either
/// malformed or an attempt to smuggle control sequences to the host.
pub fn sanitize_payload(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.len() > MAX_PAYLOAD {
        return None;
    }
    if !bytes.iter().all(|b| (0x20..=0x7e).contains(b)) {
        return None;
    }
    String::from_utf8(bytes.to_vec()).ok()
}

/// What an OSC from the AI pane asks of the relay.
#[derive(Debug, PartialEq, Eq)]
pub enum OscAction {
    /// `OSC 7501 ; ?` — the program asks whether the terminal supports reports.
    Query,
    /// A report to forward to the host (the payload after `7501;`).
    Report(String),
}

/// Classify an OSC as vt100 hands it over (split on `;`). Returns `None` for
/// every OSC that is not a well-formed 7501 sequence.
pub fn classify_osc(params: &[&[u8]]) -> Option<OscAction> {
    let (first, rest) = params.split_first()?;
    if *first != b"7501" || rest.is_empty() {
        return None;
    }
    let payload = rest.join(&b';');
    if payload == b"?" {
        return Some(OscAction::Query);
    }
    // A reply-shaped payload (`?:states=…`) coming *from* the program is not
    // a report; forwarding it would confuse the host.
    if payload.starts_with(b"?") {
        return None;
    }
    sanitize_payload(&payload).map(OscAction::Report)
}

/// Outcome of scanning the host's answer to the startup probe.
#[derive(Debug, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// The host answered the 7501 query; payload without the `7501;` prefix.
    Supported(String),
    /// The DA1 reply arrived without a 7501 reply before it.
    Unsupported,
    /// Neither the 7501 reply nor DA1 has arrived yet.
    Incomplete,
}

/// Scan bytes read from the host after the probe. Terminals answer in the
/// order they were asked, so a 7501 reply, if any, precedes the DA1 reply.
pub fn parse_probe_reply(buf: &[u8]) -> ProbeOutcome {
    const PREFIX: &[u8] = b"\x1b]7501;";
    if let Some(start) = find(buf, PREFIX) {
        let body = &buf[start + PREFIX.len()..];
        let end = body
            .iter()
            .position(|&b| b == 0x07)
            .or_else(|| find(body, b"\x1b\\"));
        return match end {
            Some(end) => match sanitize_payload(&body[..end]) {
                Some(payload) if payload.starts_with('?') => ProbeOutcome::Supported(payload),
                _ => ProbeOutcome::Unsupported,
            },
            None => ProbeOutcome::Incomplete,
        };
    }
    if contains_da1_reply(buf) {
        ProbeOutcome::Unsupported
    } else {
        ProbeOutcome::Incomplete
    }
}

/// DA1 reply: `ESC [ ? <digits and ;> c`.
fn contains_da1_reply(buf: &[u8]) -> bool {
    let mut i = 0;
    while let Some(off) = find(&buf[i..], b"\x1b[?") {
        let mut j = i + off + 3;
        while j < buf.len() && (buf[j].is_ascii_digit() || buf[j] == b';') {
            j += 1;
        }
        if j < buf.len() && buf[j] == b'c' {
            return true;
        }
        i += off + 3;
    }
    false
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Ask the host terminal whether it supports program status reports.
///
/// Must run in raw mode and before crossterm starts reading stdin (i.e.
/// before the kitty keyboard probe), because it reads the reply straight
/// from the tty. Gives up after `timeout` on terminals that never answer DA1.
#[cfg(unix)]
pub fn probe_host(timeout: Duration) -> Option<String> {
    use std::os::unix::io::AsRawFd;

    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();
    if unsafe { libc::isatty(fd) } != 1 {
        return None;
    }

    {
        let mut out = std::io::stdout();
        out.write_all(b"\x1b]7501;?\x1b\\\x1b[c").ok()?;
        out.flush().ok()?;
    }

    let deadline = Instant::now() + timeout;
    let mut buf: Vec<u8> = Vec::with_capacity(256);
    let mut chunk = [0u8; 256];
    loop {
        match parse_probe_reply(&buf) {
            ProbeOutcome::Supported(payload) => return Some(payload),
            ProbeOutcome::Unsupported => return None,
            ProbeOutcome::Incomplete => {}
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || buf.len() > 8192 {
            return None;
        }
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = remaining.as_millis().min(i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut pfd, 1, ms) };
        if ready <= 0 {
            return None;
        }
        let n = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), chunk.len()) };
        if n <= 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n as usize]);
    }
}

#[cfg(not(unix))]
pub fn probe_host(_timeout: Duration) -> Option<String> {
    None
}

/// Write queued reports to the host. Called by the UI thread only, after a
/// frame has been flushed, so the sequences never split a frame.
pub fn write_to_host(sequences: &[Vec<u8>]) {
    if sequences.is_empty() {
        return;
    }
    let mut out = std::io::stdout().lock();
    for seq in sequences {
        let _ = out.write_all(seq);
    }
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: &str = "?:states=idle,working,done,blocked,error:kinds=permission,question,auth";

    #[test]
    fn query_is_recognized() {
        assert_eq!(classify_osc(&[b"7501", b"?"]), Some(OscAction::Query));
    }

    #[test]
    fn report_is_forwarded_verbatim() {
        let p: &[&[u8]] = &[
            b"7501",
            b"state=blocked:kind=permission:app=claude-code:msg=QXBwbHk/",
        ];
        assert_eq!(
            classify_osc(p),
            Some(OscAction::Report(
                "state=blocked:kind=permission:app=claude-code:msg=QXBwbHk/".into()
            ))
        );
    }

    #[test]
    fn semicolons_inside_payload_are_rejoined() {
        assert_eq!(
            classify_osc(&[b"7501", b"state=idle", b"x"]),
            Some(OscAction::Report("state=idle;x".into()))
        );
    }

    #[test]
    fn other_oscs_and_reply_shapes_are_ignored() {
        assert_eq!(classify_osc(&[b"0", b"title"]), None);
        assert_eq!(classify_osc(&[b"7501"]), None);
        assert_eq!(classify_osc(&[b"7501", REPLY.as_bytes()]), None);
    }

    #[test]
    fn control_bytes_are_rejected() {
        assert_eq!(classify_osc(&[b"7501", b"state=idle\x1b[2J"]), None);
        assert_eq!(
            classify_osc(&[b"7501", "state=idle:msg=ü".as_bytes()]),
            None
        );
        assert_eq!(sanitize_payload(&vec![b'a'; MAX_PAYLOAD + 1]), None);
        assert_eq!(sanitize_payload(b""), None);
    }

    #[test]
    fn probe_reply_with_st_or_bel_is_supported() {
        let st = format!("\x1b]7501;{REPLY}\x1b\\\x1b[?62;22c");
        assert_eq!(
            parse_probe_reply(st.as_bytes()),
            ProbeOutcome::Supported(REPLY.into())
        );
        let bel = format!("\x1b]7501;{REPLY}\x07\x1b[?1;2c");
        assert_eq!(
            parse_probe_reply(bel.as_bytes()),
            ProbeOutcome::Supported(REPLY.into())
        );
    }

    #[test]
    fn da1_alone_means_unsupported() {
        assert_eq!(
            parse_probe_reply(b"\x1b[?62;22c"),
            ProbeOutcome::Unsupported
        );
        assert_eq!(parse_probe_reply(b"\x1b[?1;2c"), ProbeOutcome::Unsupported);
    }

    #[test]
    fn partial_reads_stay_incomplete() {
        assert_eq!(parse_probe_reply(b""), ProbeOutcome::Incomplete);
        assert_eq!(parse_probe_reply(b"\x1b[?62;2"), ProbeOutcome::Incomplete);
        assert_eq!(
            parse_probe_reply(b"\x1b]7501;?:states=idle"),
            ProbeOutcome::Incomplete
        );
    }

    #[test]
    fn sequences_are_st_terminated() {
        assert_eq!(
            osc_sequence("state=idle"),
            b"\x1b]7501;state=idle\x1b\\".to_vec()
        );
        assert_eq!(clear_sequence(), b"\x1b]7501;state=clear\x1b\\".to_vec());
    }
}
