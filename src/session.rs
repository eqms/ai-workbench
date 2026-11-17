//! Session persistence.
//!
//! Stores lightweight runtime state that should survive across runs — the
//! last-used AI backend, so launching `ai-workbench` without a positional
//! argument resumes the previous mode, and the Claude startup choices made per
//! project. Persisted as YAML at `~/.config/ai-workbench/session.yaml`.
//!
//! Per-project values live here and not in the project directory: writing into
//! a repository would show up in `git status`, and a repo-local file is exactly
//! what the local-config approval exists to distrust.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::backend::AiBackend;
use crate::config::get_config_dir;
use crate::types::{ClaudeEffort, ClaudeModel, ClaudePermissionMode};

/// Claude startup choices remembered for one project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProjectStartup {
    #[serde(default)]
    pub permission_mode: ClaudePermissionMode,
    #[serde(default)]
    pub model: ClaudeModel,
    #[serde(default)]
    pub effort: ClaudeEffort,
}

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
    /// Claude startup choices per project, keyed by [`project_key`].
    #[serde(default)]
    pub projects: BTreeMap<String, ProjectStartup>,
}

/// The project a directory belongs to: the nearest ancestor (or the directory
/// itself) that contains `.git` — a directory for a normal checkout, a file
/// for a worktree or submodule. Without one, the directory itself.
///
/// Pure path walk, no `git` process: a git call would execute the
/// repository's own config before the user has done anything.
pub fn project_root(dir: &Path) -> PathBuf {
    let start = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    start
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_path_buf)
        .unwrap_or(start)
}

/// Key under which a directory's project settings are stored.
pub fn project_key(dir: &Path) -> String {
    project_root(dir).to_string_lossy().into_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_root_finds_enclosing_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let nested = repo.join("src").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir(repo.join(".git")).unwrap();
        assert_eq!(project_root(&nested), repo.canonicalize().unwrap());
    }

    #[test]
    fn project_root_accepts_git_file_of_a_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let worktree = tmp.path().join("wt");
        std::fs::create_dir_all(worktree.join("sub")).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: /elsewhere\n").unwrap();
        assert_eq!(
            project_root(&worktree.join("sub")),
            worktree.canonicalize().unwrap()
        );
    }

    #[test]
    fn project_root_without_repository_is_the_directory_itself() {
        let tmp = tempfile::tempdir().unwrap();
        let plain = tmp.path().join("plain");
        std::fs::create_dir(&plain).unwrap();
        assert_eq!(project_root(&plain), plain.canonicalize().unwrap());
    }

    #[test]
    fn session_without_projects_still_parses() {
        let yaml = "last_cwd: ''\nlast_backend: claude\nlast_claude_update: ''\n";
        let state: SessionState = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(state.projects.is_empty());
    }

    #[test]
    fn project_startup_round_trips() {
        let mut state = SessionState::default();
        state.projects.insert(
            "/p".to_string(),
            ProjectStartup {
                permission_mode: ClaudePermissionMode::Auto,
                model: ClaudeModel::Opus,
                effort: ClaudeEffort::High,
            },
        );
        let yaml = serde_yaml_ng::to_string(&state).unwrap();
        let back: SessionState = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(back.projects.get("/p"), state.projects.get("/p"));
    }
}
