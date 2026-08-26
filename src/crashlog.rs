//! Post-mortem crash logging.
//!
//! A TUI owns the alternate screen, so a panic message printed to stderr is
//! immediately overpainted by the next rendered frame and is lost. Without a
//! file on disk there is nothing left to diagnose after the fact — which is
//! why a panic in a background thread (PTY reader, clipboard worker, git
//! check) used to leave no trace at all.
//!
//! Every panic is recorded here, main-thread or not, together with the thread
//! name and a backtrace.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once a panic has been recorded. The UI reads it to surface a hint
/// instead of leaving the user with a silently degraded application.
static CRASH_RECORDED: AtomicBool = AtomicBool::new(false);

/// Path of the crash log, next to the update log in the platform cache dir.
pub fn log_file_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ai-workbench")
        .join("crash.log")
}

/// True when this session has recorded at least one panic.
pub fn has_recorded_crash() -> bool {
    CRASH_RECORDED.load(Ordering::Relaxed)
}

/// Whether the panicking thread is allowed to tear the terminal down.
///
/// Only the thread that owns the rendering loop may do so. A background
/// thread that restores the terminal leaves the alternate screen while the
/// event loop keeps drawing, which paints the UI over the shell's scrollback
/// and disables raw mode under a running app.
pub fn may_restore_terminal(ui_thread: std::thread::ThreadId) -> bool {
    std::thread::current().id() == ui_thread
}

/// Install the panic hook, replacing whatever is currently registered.
///
/// Must be called **again after `ratatui::init()`**: ratatui registers its own
/// hook there which restores the terminal on every panic regardless of the
/// thread it came from. Replacing it (rather than chaining onto it) is the
/// point — a background thread must not tear the terminal down.
///
/// `restore` is the terminal cleanup, run only on the UI thread.
pub fn install_panic_hook(ui_thread: std::thread::ThreadId, restore: fn()) {
    // Drop the previous hook instead of chaining: chaining would keep
    // ratatui's unconditional restore alive, and would log every panic twice.
    let _ = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        record_panic(info);

        if !may_restore_terminal(ui_thread) {
            // Background thread: leave the terminal alone. The event loop is
            // still drawing, and stderr would land in the middle of the TUI.
            return;
        }

        restore();
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        eprintln!(
            "\nai-workbench {} panicked at {}:\n  {}\n\nFull report: {}",
            env!("CARGO_PKG_VERSION"),
            location,
            panic_message(info),
            log_file_path().display()
        );
    }));
}

/// Append one panic report: location, message, thread, and backtrace.
///
/// Called from the panic hook, so it must not panic itself — every fallible
/// step is best-effort.
pub fn record_panic(info: &std::panic::PanicHookInfo<'_>) {
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown location".to_string());

    let message = panic_message(info);

    let thread = std::thread::current();
    let thread_desc = match thread.name() {
        Some(name) => format!("{} ({:?})", name, thread.id()),
        None => format!("<unnamed> ({:?})", thread.id()),
    };
    let is_main = thread.name() == Some("main");

    let backtrace = std::backtrace::Backtrace::force_capture().to_string();

    let report = format_report(&thread_desc, is_main, &location, &message, &backtrace);

    write_report(&report);
    CRASH_RECORDED.store(true, Ordering::Relaxed);
}

/// Build the report body. Pure, so the layout is unit-testable.
fn format_report(
    thread_desc: &str,
    is_main: bool,
    location: &str,
    message: &str,
    backtrace: &str,
) -> String {
    format!(
        "--- panic ---\n\
         version:   {}\n\
         thread:    {}{}\n\
         location:  {}\n\
         message:   {}\n\
         backtrace:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        thread_desc,
        if is_main { " [main]" } else { " [background]" },
        location,
        message,
        backtrace,
    )
}

/// Extract the panic payload as a string (the two types `panic!` produces).
fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Append a pre-formatted report with a timestamp header.
fn write_report(report: &str) {
    let path = log_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}", timestamp, report);
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_sits_in_the_ai_workbench_cache_dir() {
        let path = log_file_path();
        assert_eq!(path.file_name().unwrap(), "crash.log");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "ai-workbench");
    }

    #[test]
    fn only_the_ui_thread_may_restore_the_terminal() {
        let ui_thread = std::thread::current().id();
        assert!(may_restore_terminal(ui_thread));

        let from_worker = std::thread::spawn(move || may_restore_terminal(ui_thread))
            .join()
            .unwrap();
        assert!(
            !from_worker,
            "a background thread must never restore the terminal"
        );
    }

    #[test]
    fn report_marks_a_background_thread_as_such() {
        let report = format_report(
            "<unnamed> (ThreadId(12))",
            false,
            "src/terminal.rs:231:9",
            "boom",
            "  0: nothing",
        );
        assert!(report.contains("[background]"));
        assert!(!report.contains("[main]"));
        assert!(report.contains("src/terminal.rs:231:9"));
        assert!(report.contains("boom"));
        assert!(report.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn report_marks_the_main_thread_as_such() {
        let report = format_report("main (ThreadId(1))", true, "src/app/mod.rs:1:1", "x", "bt");
        assert!(report.contains("[main]"));
        assert!(!report.contains("[background]"));
    }
}
