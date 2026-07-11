# Release Notes

## Version 1.0.2 (11.07.2026)

### Fixed

- **[FIX] Init-Wizard zerschoss das TUI-Layout (Terminal-Buffer-Korruption).**
  Beim First-Run-Wizard erschienen überlappende Geister-Inhalte mehrerer
  Wizard-Schritte plus Fremdzeilen (`[Update] Platform: …`, `[Update] GitHub
  version: …`, `[Update] Already up-to-date`), sodass das Fenster „viel zu groß und
  deplatziert" wirkte. Ursache war nicht die Wizard-Geometrie, sondern der
  Update-Check: er lief in einem Hintergrund-Thread und schrieb unter
  `#[cfg(debug_assertions)]` per `eprintln!` direkt auf stderr — dasselbe Terminal,
  das Ratatui im Alternate-Screen zeichnet. Diese Out-of-Band-Writes
  desynchronisierten Ratatuis Diff-Buffer. Alle Diagnose-Ausgaben in
  `check_for_update_with_version` wandern jetzt über `log_update()` ausschließlich
  in die Log-Datei (`~/Library/Caches/ai-workbench/update.log`). Zusätzlich wurden
  die sieben `eprintln!`-Fehlerausgaben in `src/app/file_ops.rs` (Datei-I/O,
  Config-Speichern) auf denselben Log-Sink umgestellt, um dieselbe
  Korruptionsklasse künftig auszuschließen.

## Version 1.0.1 (10.07.2026)

### Fixed

- **[SECURITY] Typst PDF export: template injection + path traversal.** Untrusted
  markdown link/image URLs were interpolated into Typst string literals without
  escaping `"`, and fenced code was wrapped in fixed ` ``` ` fences — a crafted
  `.md` file could break out and inject arbitrary Typst code, which (via an
  unsanitized `World::file()`) could read arbitrary local files into the exported
  PDF. Now all URLs pass through a `typst_str_escape()` boundary, code fences are
  sized dynamically, language tags are whitelisted, and `World::file()` enforces a
  canonicalized `starts_with` path-traversal guard. Six regression tests added.
- **File operations now surface errors.** Creating, renaming, and deleting files
  in the browser previously swallowed I/O errors silently (`let _ = ...`);
  failures (permission denied, name collision, disk full) now show a footer flash
  instead of appearing to do nothing.

### Changed

- Documented the deferred self-update signing work (re-confirmed by the
  10.07.2026 audit) in `SECURITY-NOTES.md`; corrected the stale `App` field-count
  note in `CLAUDE.md` (≈165 → ~65) and removed the dead `sync_terminals_initial()`.

## Version 1.0.0 (10.07.2026)

Initial release of **AI Workbench** — a Rust/Ratatui TUI multiplexer that drives
one of several AI coding-agent CLIs in its primary pane, alongside a file
browser, preview, LazyGit, and a system terminal. AI Workbench is a
multi-backend evolution of `claude-workbench`.

### Added

- **Selectable AI backend via a positional launch argument.** Start the primary
  pane with the AI agent of your choice:
  - `ai-workbench claude` — Anthropic Claude Code CLI (full permission / model /
    effort / session / worktree / remote-control flags)
  - `ai-workbench opencode` — OpenCode CLI
  - `ai-workbench pi` — Pi CLI

  The backend name is case-insensitive (`Claude`, `OpenCode`, `Pi` all work). An
  unknown value fails fast with a clear message and a non-zero exit code.
- **Backend is remembered across runs.** The chosen backend is persisted to
  `~/.config/ai-workbench/session.yaml`; launching `ai-workbench` with no
  argument resumes the last-used backend (default on first run: `claude`). An
  explicit argument always overrides the remembered value.
- **Per-backend command configuration.** New `pty.opencode_command` and
  `pty.pi_command` config keys sit alongside `pty.claude_command`; each defaults
  to its CLI binary so the AI pane runs out of the box.
- **Dynamic pane labelling.** The AI pane title and the footer `F4` hotkey label
  reflect the active backend (`Claude Code` / `OpenCode` / `Pi`).
- **Claude-only dialogs are backend-aware.** The permission-mode dialog and the
  startup-prefix dialog are Claude-specific and are suppressed in OpenCode / Pi
  mode — those backends start directly.
- **Onboarding wizard covers all backends.** The first-run wizard checks the
  availability of `claude`, `opencode`, and `pi`, lets you edit each CLI path,
  and pick the default backend (keys `1` / `2` / `3`). The choice is persisted.

### Changed

- Rebranded from `claude-workbench` to `ai-workbench` throughout: crate/binary
  name, self-update repository (`eqms/ai-workbench`), config directory
  (`~/.config/ai-workbench/`), update-log cache path, Homebrew tap
  (`eqms/homebrew-ai-workbench`), and installer scripts.

### Inherited

All existing claude-workbench capabilities are retained unchanged: file browser
with git-status colouring, syntax-highlighted preview, character-level mouse
selection with clipboard integration, PTY auto-restart, directory sync across
panes, scrollback, Markdown/PDF/browser preview, the 5-stage clipboard fallback
chain, remote (SSH) escape-transfer for export/preview, and self-update from
GitHub Releases.
