//! Session persistence.
//!
//! Stores lightweight, global (non-project-local) runtime state that should
//! survive across runs — currently the last-used AI backend, so launching
//! `ai-workbench` without a positional argument resumes the previous mode.
//! Persisted as YAML at `~/.config/ai-workbench/session.yaml`.

use serde::{Deserialize, Serialize};

use crate::backend::AiBackend;
use crate::config::get_config_dir;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionState {
    #[serde(default)]
    pub last_cwd: String,
    /// The AI backend used in the most recent run.
    #[serde(default)]
    pub last_backend: AiBackend,
    /// Date (`YYYY-MM-DD`) the daily background `claude update` was last started.
    /// Empty until the first run. Used to trigger the update at most once per day.
    #[serde(default)]
    pub last_claude_update: String,
}

/// Absolute path to the session file, or `None` if no config dir is resolvable.
fn session_path() -> Option<std::path::PathBuf> {
    get_config_dir().map(|dir| dir.join("session.yaml"))
}

/// Persist session state. Best-effort: failures are silently ignored (a TUI
/// cannot surface I/O errors here, and a lost session file is non-critical).
pub fn save_session(state: &SessionState) {
    let Some(path) = session_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml_ng::to_string(state) {
        let _ = std::fs::write(&path, yaml);
    }
}

/// Load session state, falling back to defaults (last_backend = Claude) when
/// the file is absent or unparseable.
pub fn load_session() -> SessionState {
    let Some(path) = session_path() else {
        return SessionState::default();
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_yaml_ng::from_str(&s).ok())
        .unwrap_or_default()
}
