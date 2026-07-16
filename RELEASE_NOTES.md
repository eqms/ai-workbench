# Release Notes

## Version 1.7.1 (16.07.2026)

### Fixed

- **[FIX] Shift+Enter now also inserts a newline when a terminal key binding intercepts it (iTerm2 `/terminal-setup`).** Root cause of "Shift+Enter still submits" on iTerm2 despite v1.7.0: Claude Code's `/terminal-setup` installs a **global iTerm2 key binding** (`GlobalKeyMap`, `0xd-0x20000` → "Send Text: `\n`") that fires *before* the kitty keyboard protocol, so the workbench never sees `CSI 13;2u` — only a bare LF (`0x0a`). Two-part fix: (1) `main.rs` now calls the direct crossterm 0.28 `enable_raw_mode()` after `ratatui::init()` — ratatui 0.30 enables raw mode through its own transitive crossterm 0.29, leaving 0.28's raw-mode bookkeeping stale, which made its parser map bare LF to `Enter` (submit) instead of `Ctrl+J`; the call is a termios no-op but fixes the bookkeeping. (2) `map_key_to_pty` maps `Ctrl+J` (= bare LF in raw mode) in the AI pane to `ESC+CR` (insert newline) — this also repairs Claude Code's own documented Ctrl+J newline shortcut inside the workbench, which previously submitted because the inner PTY runs in legacy keyboard mode. Verified end-to-end with a PTY harness simulating a kitty-protocol terminal: `CSI 13;2u` → `ESC+CR`, bare LF → `ESC+CR`, plain Enter → `CR` (submit) unchanged. 2 new unit tests in `src/input.rs`.

### Added

- **[ADD] `--key-diag` CLI flag.** Interactive keyboard diagnostic (pattern of `--clipboard-diag`): prints terminal markers (`TERM_PROGRAM`, `TMUX`, …), probes kitty-keyboard-protocol support, pushes `DISAMBIGUATE_ESCAPE_CODES`, then echoes every key event the terminal delivers. Pressing Shift+Enter shows immediately whether it arrives as a distinct key, a plain Enter (no protocol support), or a bare LF (a key binding intercepts it — with a pointer to the iTerm2 `/terminal-setup` GlobalKeyMap entry). The startup probe result is now also written to `update.log` (`kitty keyboard probe: …`) instead of failing silently.

## Version 1.7.0 (16.07.2026)

### Added

- **[ADD] Shift+Enter inserts a newline in the AI pane (F4).** The workbench now pushes the kitty keyboard protocol flag `DISAMBIGUATE_ESCAPE_CODES` at startup (guarded by `supports_keyboard_enhancement()`, popped in `restore_terminal()`), so terminals that support the protocol (iTerm2 3.5+, Kitty, WezTerm, Ghostty, Alacritty ≥0.13) report Shift+Enter as a distinct key event. `map_key_to_pty` translates it to `ESC+CR`, which Claude Code and OpenCode interpret as "insert newline" in legacy keyboard mode — the mode the inner PTY always runs in, since the vt100 parser never answers kitty-protocol queries. Scoped to the AI pane only (shell/LazyGit behavior unchanged). On terminals without protocol support (e.g. Terminal.app) nothing changes; the `\` + Enter fallback keeps working everywhere. 6 new unit tests in `src/input.rs`.

### Fixed

- **[FIX] Alt/Option+Enter now inserts a newline in the AI pane.** Previously the ALT branch in `map_key_to_pty` only handled word navigation (Left/Right); Alt+Enter fell through to plain `\r`, silently dropping the ESC prefix. It is now mapped to `ESC+CR` in all PTY panes — this works even without kitty-protocol support.

## Version 1.6.0 (11.07.2026)

### Added

- **[ADD] Release archives are now cryptographically signed (zipsign, SEC-01 Half 1).** The `release.yml` workflow signs every `.tar.gz`/`.zip` with an ed25519 key before publishing (signature embedded in the archive — no sidecar, signed archives still extract normally) and verifies each one against the committed public key `signing/ai-workbench-pub.bin` in CI. The private key lives only as the `ZIPSIGN_PRIVATE_KEY` GitHub Actions secret. **Client-side verification is intentionally NOT enabled yet** — per the SECURITY-NOTES.md rollout order, the next 2–3 releases ship signed first so existing self-updates keep working; enabling `self_update`'s `.verifying_keys()` (Half 2) will be a later major release.

## Version 1.5.0 (11.07.2026)

### Added

- **[ADD] `F8` now opens an AI backend selection menu.** Instead of silently cycling Claude → OpenCode → Pi on each keypress, `F8` opens a modal that lists all three backends with the active one marked `← active`. `F8` or `↑↓`/`j k` move the highlight, `Enter` applies the switch (respawning the AI pane), `Esc` cancels without a change. Backed by the new `BackendSwitchState` (`src/ui/backend_switch.rs`) following the established `visible + selected` dialog pattern, wired through `keyboard/mod.rs` dispatch (before global shortcuts so `F8` cycles the highlight), a new `handle_backend_switch_key` handler, and `drawing.rs`/`mouse.rs` overlay handling. The footer gained an `F8 Backend` button (clickable) in the terminal and file-browser contexts. `Shift+F8` still opens Settings. 4 new unit tests.
- **[ADD] Release helper `scripts/release.sh`.** Bumps `Cargo.toml` + `Cargo.lock`, drafts a `RELEASE_NOTES.md` section from the commit log (grouped by `[ADD]`/`[CHG]`/`[FIX]` prefixes), opens `$EDITOR` to finalize, then commits, tags and pushes both remotes (origin=GitLab, upstream=GitHub) after a confirmation prompt. Supports `--dry-run` and `--no-push`.

### Changed

- **[CHG] GitHub Release body now comes from `RELEASE_NOTES.md`.** The `release.yml` "Generate changelog" step extracts the curated section for the tag's version (pure portable `awk`, trimmed of blank lines) instead of a raw `git log` dump; it falls back to `git log` when no matching section exists. The published release now matches the hand-written notes.
- **[CHG] Refreshed the README.** Replaced the old banner with the new `docs/ai_workbench.png` graphic and removed two pre-existing broken screenshot links. Stripped ~330 lines of embedded `What's New in vX.Y.Z` history (down to v0.59.0, inherited from claude-workbench) in favour of a short pointer to `RELEASE_NOTES.md`; rescued the still-relevant clipboard troubleshooting into `USAGE.md` (EN + DE). Removed the three obsolete PNGs from `docs/`.

### Notes

- Release-archive signing (zipsign, SEC-01) remains a documented follow-up in `SECURITY-NOTES.md`: it is blocked on generating the operator keypair, and client-side verification must wait until 2–3 signed releases have shipped to avoid bricking in-flight self-updates.

## Version 1.4.0 (11.07.2026)

### Added

- **[ADD] Transparentes tägliches `claude update` im Hintergrund.** Beim ersten
  Start pro Kalendertag startet ai-workbench `claude update` als **detachten
  Hintergrundprozess** — vollständig transparent: nichts im TUI, Output geht in
  die Update-Logdatei (`…/ai-workbench/update.log`). Der „schon heute
  gelaufen"-Marker liegt in `session.yaml` (`last_claude_update`, `YYYY-MM-DD`).
  Läuft unabhängig vom aktiven Backend, sofern ein `claude`-Binary auf `$PATH`
  auffindbar ist; nicht-blockierend (Kind wird nie abgewartet). Abschaltbar über
  `claude.daily_update: false` (Default `true`). Neues Modul
  `src/app/daily_claude.rs`, Datums-Key via `footer::today_key()`.
- **[ADD] F8 wechselt das KI-Backend zur Laufzeit.** `F8` rotiert den KI-Bereich
  durch Claude → OpenCode → Pi (`AiBackend::next()`), startet das AI-Pane über
  `cycle_ai_backend()` neu (via `init_claude_after_wizard()`, respektiert den
  Claude-Startup-Dialog bzw. startet OpenCode/Pi direkt) und persistiert die Wahl
  in `session.yaml`. Footer zeigt kurz `✓ Backend: …`. Backend-Wechsel schluckt
  keine bestehende Funktionalität — Pane-Titel und Footer-Label aktualisieren sich
  automatisch.
- **[ADD] OpenCode/Pi-Startoptionen in den Settings editierbar.** Die OpenCode- und
  Pi-Kommandozeilen nehmen jetzt volle Argumente auf (z. B.
  `opencode --model glm-5.2:cloud`) und sind unter **Settings (Shift+F8) → Paths**
  („OpenCode Command" / „Pi Command") editierbar sowie über
  `pty.opencode_command` / `pty.pi_command` in der `config.yaml`. Parsing via
  `shlex` mit Fallback auf das nackte Binary bei leerer/ungültiger Eingabe.

### Changed

- **[CHG] Settings von F8 auf Shift+F8 verschoben**, damit `F8` den Backend-Wechsel
  antreibt (bewährtes F-Taste-+-Modifier-Muster wie Shift+F2/Shift+F9).
- **[CHG] Claude-Model-Auswahl auf Fable/Opus/Sonnet/Haiku** (+ CLI-Default)
  erneuert. Die Optionen bilden die CLI-`--model`-Aliase ab und zeigen immer auf
  die neueste Version der jeweiligen Stufe — bewusst ohne feste Versionsnummer im
  Label, da diese ohne CLI-/API-Abfrage nicht zuverlässig ermittelbar ist.

### Removed

- **[CHG] Remote Control entfernt** aus dem Claude-Startup-Dialog, dem
  `--remote-control`-Flag-Pfad und der Config (`claude.remote_control` existiert
  nicht mehr). Bestehende `config.yaml`-Dateien laden weiterhin — der unbekannte
  Key wird ignoriert.

## Version 1.3.0 (11.07.2026)

### Added

- **[ADD] Startup-Intro „Cyberpunk Glitch & Scanline Reveal".** Beim App-Start
  erscheint jetzt ein „AI WORKBENCH"-Block-Logo, das zunächst geglitcht aufflackert
  (zufällige Zeilen-Offsets, korrumpierte Zeichen aus `@ # $ % & ░ ▒ ▓ █`), dann von
  einer hellen Cyan-Scanline von oben nach unten „repariert" wird und schließlich in
  den Branding-Farben stabilisiert — eine ~4,5 s lange Enthüllung (Glitch 0,9 s +
  Sweep 2,4 s + Stabilisierung 1,2 s). Rein zeitgesteuert über `Instant::elapsed()`
  (keine neue Dependency; der Glitch-Zufall stammt aus einem winzigen inline-
  xorshift-PRNG). Umgesetzt als neues Modul `src/ui/intro.rs` mit `IntroState`,
  gerendert als oberstes Vollbild-Overlay in `drawing.rs`, während Panes/PTYs
  dahinter unverändert starten — der 16-ms-Render-Loop liefert die Frames ohne
  zusätzliche Tick-Infrastruktur. Beliebige Taste oder Klick überspringt sofort
  (Skip-Hooks in `keyboard/mod.rs` und `mouse.rs`); Auto-Dismiss nach Ablauf via
  `IntroState::tick()` im Event-Loop. Abschaltbar über neues Config-Feld
  `ui.intro_animation: bool` (Default `true`, rückwärtskompatibel via
  `#[serde(default = "default_true")]`). Kleine Terminals (< 70 Spalten) fallen auf
  einen kompakten gestylten Schriftzug zurück. 8 neue Unit-Tests (Phasen-Grenzen,
  PRNG-Determinismus, Banner-Breite, Render-Smoke-Test über mehrere Größen inkl. 1×1).

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
