//! Async startup dependency check.
//!
//! `DependencyReport::check()` spawns ~12-20 subprocesses sequentially
//! (tool `--version` probes plus `which` lookups). Running it synchronously
//! in `App::new()` was the largest contributor to the startup black screen,
//! so it is dispatched to a background thread and polled from the event loop
//! like the other `JobState` jobs.

use crate::setup::DependencyReport;

use super::{App, JobState, PollOutcome};

impl App {
    pub(super) fn start_dependency_check(&mut self) {
        self.dependency_check_job = JobState::running(DependencyReport::check_async());
    }

    /// Poll for the async dependency check result. On completion, store the
    /// report and seed the Linux clipboard warning banner.
    pub(super) fn poll_dependency_check(&mut self) {
        let report = match self.dependency_check_job.poll() {
            PollOutcome::Ready(report) => report,
            // Disconnected: worker thread died before sending — keep the
            // default (empty) report; F12 help simply shows nothing found.
            PollOutcome::Disconnected | PollOutcome::Pending => return,
        };

        self.dependency_report = report;
        self.seed_clipboard_warning();
    }

    /// Seed the clipboard warning banner if no Linux helpers are available
    /// (xclip / xsel / wl-copy / wl-paste). On macOS / Windows / native Linux
    /// with arboard the helpers are optional, so we only nag on Linux without
    /// any helper present.
    pub(super) fn seed_clipboard_warning(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let helpers = &self.dependency_report.clipboard_helpers;
            if helpers.none_available() {
                self.clipboard_warning = Some((
                    "xclip / xsel / wl-copy fehlen — Clipboard eingeschränkt. F12 für Details, Esc zum Schließen.".to_string(),
                    std::time::Instant::now(),
                ));
            }
        }
    }
}
