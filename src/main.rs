pub mod app;
pub mod app_detector;
pub mod backend;
pub mod browser;
pub mod clipboard;
pub mod config;
pub mod filter;
pub mod git;
pub mod input;
pub mod session;
pub mod setup;
pub mod syntax_registry;
pub mod terminal;
pub mod types;
pub mod ui;
pub mod update;

use anyhow::Result;
use app::App;
use backend::AiBackend;
use clap::Parser;
use config::load_config;
use session::{load_session, save_session, SessionState};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use update::{check_for_update_with_version, UpdateCheckResult};
#[cfg(debug_assertions)]
use update::{perform_update_to_version_sync, UpdateResult};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// AI backend for the primary pane: `claude`, `opencode`, `pi`, or
    /// `codex` (case-insensitive). When omitted, the last-used backend is
    /// resumed (default on first run: claude).
    #[arg(value_name = "BACKEND")]
    mode: Option<String>,

    #[arg(short, long)]
    config: Option<PathBuf>,

    #[arg(short, long)]
    session: Option<String>,

    /// Check for updates and exit (without starting TUI)
    #[arg(long)]
    check_update: bool,

    /// Fake current version for testing (e.g., "0.37.0").
    /// Only available in debug builds to prevent update-suppression attacks.
    #[cfg(debug_assertions)]
    #[arg(long, env = "WORKBENCH_FAKE_VERSION")]
    fake_version: Option<String>,

    /// Update to a specific version (for testing/downgrade, e.g., "v0.38.5" or "0.38.5").
    /// Only available in debug builds — release binaries do not expose this flag
    /// to prevent privilege-escalation via intentional downgrade to unsigned releases.
    #[cfg(debug_assertions)]
    #[arg(long)]
    update_to: Option<String>,

    /// Diagnose clipboard backends and exit (without starting TUI).
    /// Reports which fallback chain stage is active, which helper binaries
    /// (xclip/xsel/wl-copy/wl-paste) are present, relevant environment
    /// variables, and runs a copy/paste roundtrip test.
    #[arg(long)]
    clipboard_diag: bool,

    /// Diagnose SSH image-paste readiness and exit (without starting TUI).
    /// Reports SSH session state, presence of the `cc-clip` helper on
    /// `$PATH`, and TCP reachability of the cc-clip daemon port (9998).
    /// Use when image paste in the Claude pane fails over SSH from a Mac.
    #[arg(long)]
    ssh_paste_diag: bool,

    /// Diagnose remote open/export file transfer and exit (without starting
    /// TUI). Reports SSH session state, the terminal file-transfer capability
    /// (iTerm2/WezTerm/Kitty/none), relevant environment variables, the
    /// effective export directory, and the configured `remote_transfer` mode.
    /// Use when `o` / `Ctrl+X` export doesn't reach your Mac over SSH.
    #[arg(long)]
    open_diag: bool,

    /// Diagnose keyboard input for the AI pane and exit (without starting the
    /// TUI). Probes kitty-keyboard-protocol support, then echoes every key
    /// event the terminal actually delivers. Use when Shift+Enter submits
    /// instead of inserting a newline: press Shift+Enter in the diagnostic to
    /// see whether the terminal reports it (or a key binding intercepts it).
    #[arg(long)]
    key_diag: bool,
}

/// Run update check from CLI and exit
fn run_update_check_cli(fake_version: Option<String>) -> Result<()> {
    let current = fake_version.as_deref().unwrap_or(update::CURRENT_VERSION);
    let is_fake = fake_version.is_some();

    println!(
        "Current version: {}{}",
        current,
        if is_fake { " (fake)" } else { "" }
    );
    println!("Checking GitHub releases...");
    println!();

    match check_for_update_with_version(current) {
        UpdateCheckResult::UpToDate => {
            println!("✅ Already up-to-date (v{})", current);
        }
        UpdateCheckResult::UpdateAvailable {
            version,
            release_notes,
        } => {
            println!("🔄 Update available: {}", version);
            if let Some(notes) = release_notes {
                println!();
                println!("── What's New ──────────────────────────────────────");
                for line in notes.lines().take(20) {
                    println!("  {}", line);
                }
                if notes.lines().count() > 20 {
                    println!("  ... (truncated)");
                }
            }
        }
        UpdateCheckResult::NoReleasesFound => {
            println!("⚠️  No releases found for this platform");
            println!(
                "   Platform: {}-{}",
                std::env::consts::ARCH,
                std::env::consts::OS
            );
        }
        UpdateCheckResult::Error(msg) => {
            println!("❌ Error checking for updates: {}", msg);
        }
    }

    Ok(())
}

/// Run update to a specific version from CLI and exit.
/// Debug-only: paired with the `#[cfg(debug_assertions)]` arm in `main()`
/// that parses `--update-to` (gated by CR-02). Release builds neither
/// expose the flag nor compile this handler.
#[cfg(debug_assertions)]
fn run_update_to_version_cli(target_version: &str) -> Result<()> {
    println!("Current version: {}", update::CURRENT_VERSION);
    println!("Target version:  {}", target_version);
    println!();
    println!("Downloading and installing...");
    println!();

    match perform_update_to_version_sync(target_version) {
        UpdateResult::Success {
            old_version,
            new_version,
        } => {
            println!("✅ Update successful: {} -> {}", old_version, new_version);
            println!();
            println!("Please restart the application to use the new version.");
        }
        UpdateResult::Error(msg) => {
            println!("❌ Update failed: {}", msg);
            println!();
            println!(
                "Check the log file for details: {}",
                update::log_file_path().display()
            );
        }
    }

    Ok(())
}

/// Restore terminal to normal state - called on exit, panic, or signal
fn restore_terminal() {
    // Pop is safe even when the enhancement flags were never pushed —
    // terminals without kitty-protocol support ignore the sequence.
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::PopKeyboardEnhancementFlags,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    );
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = std::io::stdout().flush();
}

/// Run clipboard diagnostic from CLI and exit.
fn run_clipboard_diag_cli() -> Result<()> {
    use clipboard::{ClipboardDiag, ClipboardOutcome};

    println!(
        "ai-workbench v{} — clipboard diagnostic",
        env!("CARGO_PKG_VERSION")
    );
    println!();

    let diag = ClipboardDiag::collect();
    println!("Strategy:           {:?}", diag.strategy);
    match &diag.strategy_env {
        Some(v) if !v.is_empty() => {
            println!("  ENV override:     {}={}", clipboard::STRATEGY_ENV, v)
        }
        _ => println!(
            "  ENV override:     (unset — set {}=osc52 to bypass xclip/xsel)",
            clipboard::STRATEGY_ENV
        ),
    }
    println!();
    println!("Helper binaries:");
    fn show(name: &str, path: &Option<std::path::PathBuf>) {
        match path {
            Some(p) => println!("  {:<10} ✓ {}", name, p.display()),
            None => println!("  {:<10} ✗ not found", name),
        }
    }
    show("xclip", &diag.xclip);
    show("xsel", &diag.xsel);
    show("wl-copy", &diag.wl_copy);
    show("wl-paste", &diag.wl_paste);
    println!();

    println!("Environment:");
    fn show_env(name: &str, val: &Option<String>) {
        match val {
            Some(v) if !v.is_empty() => println!("  {:<18} = {}", name, v),
            _ => println!("  {:<18} = (unset)", name),
        }
    }
    show_env("DISPLAY", &diag.display);
    show_env("WAYLAND_DISPLAY", &diag.wayland_display);
    show_env("XDG_SESSION_TYPE", &diag.xdg_session_type);
    show_env("XRDP_SESSION", &diag.xrdp_session);
    show_env("SSH_TTY", &diag.ssh_tty);
    println!();

    let test_marker = format!("workbench-diag-{}", std::process::id());
    println!("Roundtrip test (marker: {}):", test_marker);
    // Diag uses the synchronous path so the reported outcome is the
    // real backend result — not the worker's `Submitted` placeholder.
    let outcome = clipboard::copy_to_clipboard_sync(&test_marker);
    println!("  Copy backend:     {} ({:?})", outcome.label(), outcome);
    if matches!(outcome, ClipboardOutcome::Osc52) {
        println!("  Note: OSC 52 has no read path, skipping paste verification.");
    } else {
        match clipboard::paste_from_clipboard() {
            Some(text) if text == test_marker => {
                println!("  Paste roundtrip:  ✓ matches");
            }
            Some(text) => {
                println!(
                    "  Paste roundtrip:  ✗ mismatch (read back: {:?})",
                    text.chars().take(40).collect::<String>()
                );
            }
            None => {
                println!("  Paste roundtrip:  ✗ paste returned None");
            }
        }
    }
    println!();
    println!("F11 in the TUI uses the same fallback chain to inject paste");
    println!("into the active pane — useful when Kitty's bracketed-paste");
    println!("forwarding is broken (e.g., under XRDP).");

    Ok(())
}

/// Run SSH-image-paste diagnostic from CLI and exit.
///
/// Three checks:
///  1. SSH session detection (`SSH_TTY` / `SSH_CONNECTION`).
///  2. `cc-clip` binary on `$PATH`.
///  3. TCP reachability of the cc-clip daemon on `127.0.0.1:9998` —
///     when set up correctly the user runs `ssh -R 9998:localhost:9998`
///     so the remote port forwards to the Mac-side daemon.
fn run_ssh_paste_diag_cli() -> Result<()> {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    println!(
        "ai-workbench v{} — SSH image-paste diagnostic",
        env!("CARGO_PKG_VERSION")
    );
    println!();

    // 1. SSH session detection
    let in_ssh = clipboard::is_ssh_session();
    println!("SSH session:");
    if in_ssh {
        println!("  ✓ detected (SSH_TTY or SSH_CONNECTION set)");
    } else {
        println!("  ✗ not detected — these settings only matter when running over SSH");
    }
    if let Ok(v) = std::env::var("SSH_TTY") {
        println!("    SSH_TTY        = {}", v);
    }
    if let Ok(v) = std::env::var("SSH_CONNECTION") {
        println!("    SSH_CONNECTION = {}", v);
    }
    println!();

    // 2. cc-clip on PATH
    println!("cc-clip helper:");
    match clipboard::which("cc-clip") {
        Some(p) => println!("  ✓ found: {}", p.display()),
        None => {
            println!("  ✗ not on $PATH");
            println!("    Install on this host:  cargo install cc-clip");
            println!("    Project page:           https://github.com/ShunmeiCho/cc-clip");
        }
    }
    println!();

    // 3. cc-clip daemon port reachability (the daemon runs on the Mac;
    //    `ssh -R 9998:localhost:9998` exposes it on this host).
    println!("Daemon reachability (127.0.0.1:9998):");
    let addr: SocketAddr = "127.0.0.1:9998".parse().expect("hardcoded address parses");
    match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
        Ok(_) => println!("  ✓ port 9998 reachable — daemon or reverse-tunnel is up"),
        Err(e) => {
            println!("  ✗ port 9998 unreachable: {}", e);
            println!("    On your Mac:    start the cc-clip daemon");
            println!("    ~/.ssh/config:  RemoteForward 9998 localhost:9998");
        }
    }
    println!();
    println!("If all three checks pass, image paste in the Claude pane");
    println!("(Ctrl+V) will route through cc-clip and inject the image path.");

    Ok(())
}

/// Run remote open/export transfer diagnostic from CLI and exit.
///
/// Reports whether the workbench sees an SSH session, which terminal
/// file-transfer protocol the local terminal supports, the raw environment
/// markers used for detection, the effective export directory, and the
/// configured `remote_transfer` mode. Helps identify a terminal (e.g.
/// Terminus/Tabby) whose markers aren't detected without triggering a real
/// export.
fn run_open_diag_cli() -> Result<()> {
    use browser::remote_open::{
        detect_transfer_capability, effective_capability, TransferCapability,
    };

    println!(
        "ai-workbench v{} — remote open/transfer diagnostic",
        env!("CARGO_PKG_VERSION")
    );
    println!();

    // 1. SSH session detection
    let in_ssh = clipboard::is_ssh_session();
    println!("SSH session:");
    if in_ssh {
        println!("  ✓ detected — remote transfer path is active");
    } else {
        println!("  ✗ not detected — files open locally as usual");
    }
    println!();

    // 2. Terminal capability + raw markers
    let env_str = |k: &str| std::env::var(k).unwrap_or_else(|_| "(unset)".to_string());
    println!("Terminal markers:");
    println!("    TERM_PROGRAM    = {}", env_str("TERM_PROGRAM"));
    println!("    LC_TERMINAL     = {}", env_str("LC_TERMINAL"));
    println!("    WEZTERM_PANE    = {}", env_str("WEZTERM_PANE"));
    println!("    KITTY_WINDOW_ID = {}", env_str("KITTY_WINDOW_ID"));
    println!("    TERM            = {}", env_str("TERM"));
    println!("    TMUX            = {}", env_str("TMUX"));
    println!("    STY             = {}", env_str("STY"));
    println!();

    let detected = detect_transfer_capability();
    println!("Detected capability: {:?}", detected);

    // 3. Config-resolved effective capability + export dir
    let config = load_config()?;
    let mode = &config.ui.remote_transfer;
    let effective = effective_capability(mode);
    println!("Configured remote_transfer mode: {:?}", mode);
    println!("Effective capability: {:?}", effective);
    println!(
        "Export directory: {}",
        browser::pdf_export::resolve_export_dir(&config.ui.export_dir).display()
    );
    println!();

    match effective {
        TransferCapability::Iterm2 | TransferCapability::WezTerm => {
            println!("→ Files will stream to your Mac's ~/Downloads over the SSH TTY.");
        }
        TransferCapability::Kitty => {
            println!("→ Kitty file transfer isn't implemented; files stay on the server");
            println!("  and their path is reported. Set ui.remote_transfer if you use");
            println!("  iTerm2/WezTerm instead.");
        }
        TransferCapability::None => {
            println!("→ No terminal transfer support: files stay on the server and their");
            println!("  path is reported. If you actually use iTerm2/WezTerm but markers");
            println!("  are stripped (tmux/SSH), set ui.remote_transfer: \"iterm2\".");
        }
    }
    if std::env::var_os("TMUX").is_some() {
        println!();
        println!("Note: tmux detected — requires `set -g allow-passthrough on` (tmux ≥ 3.3)");
        println!("for the transfer escape to reach the terminal.");
    }

    Ok(())
}

/// Run keyboard input diagnostic from CLI and exit.
///
/// Probes kitty-keyboard-protocol support (the mechanism behind Shift+Enter
/// newline insertion in the AI pane), pushes the same enhancement flags the
/// TUI uses, then echoes every key event crossterm delivers so the user can
/// see what their terminal actually sends. Detects the classic failure mode
/// where an iTerm2 key binding (installed by Claude Code's `/terminal-setup`)
/// intercepts Shift+Enter and sends a bare LF instead.
fn run_key_diag_cli() -> Result<()> {
    use crossterm::event::{
        Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    };
    use std::time::{Duration, Instant};

    println!(
        "ai-workbench v{} — key input diagnostic (Shift+Enter)",
        env!("CARGO_PKG_VERSION")
    );
    println!();

    let env_str = |k: &str| std::env::var(k).unwrap_or_else(|_| "(unset)".to_string());
    println!("Terminal markers:");
    println!("    TERM                 = {}", env_str("TERM"));
    println!("    TERM_PROGRAM         = {}", env_str("TERM_PROGRAM"));
    println!(
        "    TERM_PROGRAM_VERSION = {}",
        env_str("TERM_PROGRAM_VERSION")
    );
    println!("    LC_TERMINAL          = {}", env_str("LC_TERMINAL"));
    println!("    TMUX                 = {}", env_str("TMUX"));
    println!("    SSH_TTY              = {}", env_str("SSH_TTY"));
    println!();

    crossterm::terminal::enable_raw_mode()?;

    // Ensure raw mode and pushed flags are undone even on early return.
    struct RawGuard {
        pushed: bool,
    }
    impl Drop for RawGuard {
        fn drop(&mut self) {
            if self.pushed {
                let _ = crossterm::execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
            }
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    let mut guard = RawGuard { pushed: false };

    let support = crossterm::terminal::supports_keyboard_enhancement();
    match &support {
        Ok(true) => {
            print!("Kitty keyboard protocol: ✓ supported — pushing DISAMBIGUATE_ESCAPE_CODES\r\n")
        }
        Ok(false) => print!(
            "Kitty keyboard protocol: ✗ NOT supported (terminal answered the probe negatively)\r\n"
        ),
        Err(e) => print!(
            "Kitty keyboard protocol: ✗ probe failed: {} (no answer within 2s)\r\n",
            e
        ),
    }
    if matches!(support, Ok(true)) {
        let _ = crossterm::execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
        guard.pushed = true;
    }
    print!("\r\n");
    print!(
        "Press keys to inspect them — try Shift+Enter. Esc or q quits (auto-exit after 30s).\r\n"
    );
    print!("\r\n");

    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if !crossterm::event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(k) = crossterm::event::read()? else {
            continue;
        };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        print!("  key: {:?}  modifiers: {:?}\r\n", k.code, k.modifiers);
        match (k.code, k.modifiers) {
            (KeyCode::Enter, m) if m.contains(KeyModifiers::SHIFT) => {
                print!(
                    "  ✅ Shift+Enter arrives correctly — the AI pane will insert a newline.\r\n"
                );
            }
            (KeyCode::Enter, m) if m == KeyModifiers::NONE => {
                print!("  ○ plain Enter (submit). If you pressed SHIFT+Enter and see this,\r\n");
                print!(
                    "    the terminal does not report the Shift modifier (no kitty protocol).\r\n"
                );
            }
            (KeyCode::Char('j'), m) | (KeyCode::Char('J'), m)
                if m.contains(KeyModifiers::CONTROL) =>
            {
                print!("  ⚠ bare LF / Ctrl+J received. If you pressed Shift+Enter, a terminal\r\n");
                print!(
                    "    key binding intercepts it (iTerm2: Settings → Keys → Key Bindings,\r\n"
                );
                print!(
                    "    entry \"⇧↩ → Send Text: \\n\" from Claude Code's /terminal-setup).\r\n"
                );
                print!("    The AI pane maps this to a newline anyway since v1.7.1.\r\n");
            }
            (KeyCode::Esc, _) | (KeyCode::Char('q'), KeyModifiers::NONE) => {
                break;
            }
            _ => {}
        }
    }

    drop(guard);
    println!();
    Ok(())
}

fn main() -> Result<()> {
    // Parse args early - before tokio runtime
    let args = Args::parse();

    // Extract fake_version (only available in debug builds)
    #[cfg(debug_assertions)]
    let fake_version = args.fake_version;
    #[cfg(not(debug_assertions))]
    let fake_version: Option<String> = None;

    // Handle --check-update CLI mode (exit without starting TUI or tokio)
    if args.check_update {
        return run_update_check_cli(fake_version);
    }

    // Handle --update-to CLI mode (update to specific version and exit)
    // Only available in debug builds (field is cfg-gated in Args struct)
    #[cfg(debug_assertions)]
    if let Some(target_version) = args.update_to {
        return run_update_to_version_cli(&target_version);
    }

    // Handle --clipboard-diag CLI mode (exit without starting TUI)
    if args.clipboard_diag {
        return run_clipboard_diag_cli();
    }

    // Handle --ssh-paste-diag CLI mode (exit without starting TUI)
    if args.ssh_paste_diag {
        return run_ssh_paste_diag_cli();
    }

    // Handle --open-diag CLI mode (exit without starting TUI)
    if args.open_diag {
        return run_open_diag_cli();
    }

    // Handle --key-diag CLI mode (exit without starting TUI)
    if args.key_diag {
        return run_key_diag_cli();
    }

    // Validate the positional backend argument early, before spinning up the
    // tokio runtime or the TUI, so an invalid value fails fast with a clear
    // message and a non-zero exit code.
    if let Some(mode) = &args.mode {
        if AiBackend::parse(mode).is_none() {
            eprintln!("Error: unknown backend '{mode}'. Valid: claude, opencode, pi, codex");
            std::process::exit(2);
        }
    }

    // Run the async main with tokio runtime
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
        .block_on(async_main(fake_version, args.mode))
}

async fn async_main(fake_version: Option<String>, mode: Option<String>) -> Result<()> {
    // Set up panic hook to restore terminal on crash
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    // Ignore SIGTSTP (Ctrl+Z) to prevent suspend with broken terminal state
    // User can still quit with Ctrl+Q or Ctrl+C
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTSTP, libc::SIG_IGN);
    }

    // Startup-Indikator: zeilenweise auf Stderr — sichtbar bevor ratatui::init()
    // den Alternate-Screen auf Stdout zieht. Stderr bleibt im normalen Buffer
    // und stoert die TUI-Ausgabe nicht. Auf Windows mit ConPTY ist der
    // Spawn-Pfad spuerbar langsamer, daher dort das groesste UX-Plus.
    let t0 = std::time::Instant::now();
    {
        let mut err = std::io::stderr();
        let _ = writeln!(
            err,
            "ai-workbench v{} starting...",
            env!("CARGO_PKG_VERSION")
        );
    }

    let config = load_config()?;
    {
        let mut err = std::io::stderr();
        let _ = writeln!(err, "  config loaded ({} ms)", t0.elapsed().as_millis());
    }

    let session = load_session();

    // Resolve the active backend: explicit CLI argument wins, otherwise resume
    // the last-used backend from the session (default: Claude). The argument was
    // already validated in `main()`, so `parse` failing here means "not given".
    let backend = mode
        .as_deref()
        .and_then(AiBackend::parse)
        .unwrap_or(session.last_backend);

    // Persist the resolved backend so a subsequent argument-less run resumes it.
    // Preserve the daily-update timestamp — App::new updates it when it runs.
    save_session(&SessionState {
        last_cwd: session.last_cwd.clone(),
        last_backend: backend,
        last_claude_update: session.last_claude_update.clone(),
    });

    {
        let mut err = std::io::stderr();
        let _ = writeln!(err, "  spawning {} pane...", backend.short_label());
    }

    let terminal = ratatui::init();
    // ratatui::init() enables raw mode through ratatui-crossterm's own
    // crossterm 0.29, so the direct crossterm 0.28 dependency (which runs the
    // event loop and its input parser) still believes the terminal is cooked.
    // Its parser then maps a bare LF (0x0a) to Enter instead of Ctrl+J,
    // breaking the AI-pane newline mapping for terminals/key bindings that
    // send LF (e.g. iTerm2's /terminal-setup Shift+Enter binding). Enabling
    // raw mode here is a termios no-op but fixes 0.28's bookkeeping.
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste
    )?;

    // Kitty keyboard protocol: makes Shift+Enter distinguishable from plain
    // Enter (needed for newline insertion in the AI pane). The probe requires
    // raw mode (enabled by ratatui::init above) and may block up to ~2s on
    // terminals that never answer. No-op on unsupported terminals — legacy
    // key reporting stays in effect and behavior is unchanged. The result is
    // logged (update.log) because a failed probe silently disables Shift+Enter;
    // `--key-diag` gives an interactive view of the same probe.
    let kitty_probe = crossterm::terminal::supports_keyboard_enhancement();
    update::log_update(&format!("kitty keyboard probe: {:?}", kitty_probe));
    if kitty_probe.unwrap_or(false) {
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::PushKeyboardEnhancementFlags(
                crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            )
        );
    }

    let app = App::new(config, session, fake_version, backend);

    let restart_requested = app.run(terminal);

    // Normal cleanup
    restore_terminal();

    // Check if restart was requested (after update)
    match restart_requested {
        Ok(true) => {
            println!("Restarting application...");
            if let Err(e) = update::restart_application() {
                eprintln!("Restart failed: {}", e);
                eprintln!("Please restart manually.");
                return Err(anyhow::anyhow!("Restart failed: {}", e));
            }
            // exec() on Unix replaces the process, so this is only reached on Windows
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(e) => Err(e),
    }
}
