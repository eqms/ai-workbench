//! Startup dependency check.
//!
//! Outside the first-run wizard the report has exactly one consumer: the
//! Linux clipboard warning, which needs to know whether xclip / xsel /
//! wl-copy / wl-paste exist. That is four PATH lookups, so it runs inline.
//! The full `DependencyReport::check()` — `--version` probes of every AI CLI,
//! several of them Node programs — stays reserved for the wizard, which shows
//! the versions; running it in the background on every start cost seconds of
//! CPU while the AI pane was booting.

use crate::setup::DependencyReport;

use super::App;

impl App {
    pub(super) fn run_dependency_check(&mut self) {
        self.dependency_report = DependencyReport::clipboard_only();
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
