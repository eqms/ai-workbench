//! Transparent once-per-day background `claude update`.
//!
//! On the first start of each calendar day, ai-workbench kicks off `claude
//! update` in a **detached background process** so the user always runs a fresh
//! Claude CLI. It is deliberately invisible: nothing is shown in the TUI, the
//! process is fully detached, and its output is redirected to the update log
//! (`dirs::cache_dir()/ai-workbench/update.log`). The "already ran today" marker
//! lives in `session.yaml` (`last_claude_update`, a `YYYY-MM-DD` key).
//!
//! Gated by `config.claude.daily_update` (default `true`). Runs regardless of
//! the active backend as long as a `claude` binary is resolvable on `$PATH`.

use std::fs::OpenOptions;
use std::process::{Command, Stdio};

use crate::update::log_update;

use super::App;

impl App {
    /// Start the daily background `claude update` if it hasn't run today.
    ///
    /// Non-blocking and best-effort: the child is spawned detached and never
    /// awaited, so a slow or hanging update cannot stall startup or the UI.
    /// Called once from `App::new`.
    pub(super) fn maybe_daily_claude_update(&mut self) {
        if !self.config.claude.daily_update {
            return;
        }

        let today = crate::ui::footer::today_key();
        if self.session.last_claude_update == today {
            return; // Already started today.
        }

        // Resolve the claude binary (first token of the configured command, or
        // the bare `claude`). Skip silently if it isn't on PATH — do NOT mark
        // the day done, so a later start (once installed) still triggers it.
        let bin = self
            .config
            .pty
            .claude_command
            .first()
            .cloned()
            .unwrap_or_else(|| "claude".to_string());
        if crate::clipboard::which(&bin).is_none() {
            log_update(&format!(
                "daily claude update: '{}' not on PATH — skipping",
                bin
            ));
            return;
        }

        // Redirect child stdout/stderr to the update log so the run is
        // debuggable without ever touching the TUI's terminal. Fall back to
        // null if the log file can't be opened.
        let (out, err) = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(crate::update::log_file_path())
        {
            Ok(f) => match f.try_clone() {
                Ok(f2) => (Stdio::from(f), Stdio::from(f2)),
                Err(_) => (Stdio::null(), Stdio::null()),
            },
            Err(_) => (Stdio::null(), Stdio::null()),
        };

        match Command::new(&bin)
            .arg("update")
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .spawn()
        {
            Ok(_child) => {
                // Intentionally not awaited — detached, transparent.
                log_update(&format!("daily claude update: started '{} update'", bin));
                self.session.last_claude_update = today;
                // Keep the persisted backend consistent with the running one.
                self.session.last_backend = self.backend;
                crate::session::save_session(&self.session);
            }
            Err(e) => {
                // Don't mark the day done — retry on the next start.
                log_update(&format!("daily claude update: spawn failed: {}", e));
            }
        }
    }
}
