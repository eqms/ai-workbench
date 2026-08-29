# ai-workbench — Architecture Reference

Reference material moved out of `CLAUDE.md` on 29.08.2026. It was measured that
the CLAUDE.md chain cost 29.312 tokens on *every* request; this file holds the
parts an agent can read on demand instead of carrying permanently.

**Read this when:** touching pane layout, adding a module, changing the event
loop, or onboarding to the codebase. Behaviour-governing rules (gotchas,
conventions with teeth, hard prohibitions) stay in `CLAUDE.md` — not here.

---

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| `App` | Master state, event loop, sub-module dispatch | `src/app/mod.rs` |
| `PseudoTerminal` | PTY lifecycle, background reader thread, vt100 parse | `src/terminal.rs` |
| `JobState<T>` | Typed async job lifecycle (Idle/Running/poll) | `src/app/job_state.rs` |
| `compute_layout` | Maps terminal Rect into 6 pane Rects | `src/ui/layout.rs` |
| `keyboard/` | Key event dispatch, split by context | `src/app/keyboard/mod.rs` + submodules |
| `mouse.rs` | Mouse event dispatch, hit-test, drag/select | `src/app/mouse.rs` |
| `drawing.rs` | Full-frame render orchestration | `src/app/drawing.rs` |
| `clipboard.rs` | 5-stage fallback chain, async worker thread | `src/clipboard.rs` |
| `config.rs` | YAML config load/save, struct definitions | `src/config.rs` |
| `update/` | GitHub release check, binary self-replace | `src/update/` |

## Pattern Overview

- `App` is a single monolithic struct (~65 fields). All handler modules implement
  `App` via `impl App` blocks, not separate structs.
- PTY output is produced on N background threads (one per pane); the main thread
  consumes via `Arc<Mutex<vt100::Parser>>` during `draw()`.
- Async jobs (update check, git remote check, PDF export) use
  `std::sync::mpsc` channels wrapped in `JobState<T>`, polled each event-loop
  iteration with `try_recv()`. No blocking inside the loop.
- 16 ms poll timeout gives ~60 fps UI refresh rate.

## Layers

**CLI / bootstrap** — `src/main.rs`
Parses args, handles non-TUI modes, bootstraps the tokio runtime. Contains
`Args` (clap), `run_update_check_cli`, `run_clipboard_diag_cli`,
`run_ssh_paste_diag_cli`, `async_main`. Depends on `update`, `clipboard`,
`session`, `config`, `app`.

**Application state** — `src/app/mod.rs`
Owns all application state and runs the event loop: `App` struct, `App::new()`,
`App::run()`. Depends on all other layers; used by `main.rs` only.

**Input dispatch** — `src/app/keyboard/` (5 submodules), `src/app/mouse.rs`
Translates crossterm events into `App` mutations. Depends on `terminal.rs`,
`types.rs`, `clipboard.rs`.

**PTY layer** — `src/terminal.rs`
Manages subprocess lifecycle and terminal emulation: `PseudoTerminal`,
`PtyCallbacks` (DSR/CPR/DA response handler).
Shared state: `Arc<Mutex<vt100::Parser<PtyCallbacks>>>` read by the UI, written
by the background thread. Writer: `Arc<Mutex<Box<dyn Write + Send>>>`, locked
only during `write_input()`. Exit detection: `Arc<AtomicBool>` set by the reader
thread on EOF.

**Async jobs** — `src/app/job_state.rs`, used in `src/app/update.rs`,
`src/app/git_ops.rs`, `src/app/clipboard.rs`
`JobState<T>` enum (`Idle` | `Running(Receiver<T>)`), `PollOutcome<T>`.
Pattern: `std::thread::spawn` → send on `mpsc::Sender<T>` → `App::run()` calls
`poll()` each loop. Active jobs: `git_check_job`, `update_check_job`,
`update_job`, `export_job`.

**Rendering** — `src/ui/`
Stateless frame rendering from `App` state, one file per widget/pane. Depends on
`ratatui`, `vt100` (reads parser screen), `syntect`. Called by
`src/app/drawing.rs` once per loop iteration.

**Support modules**
- `src/clipboard.rs` — 5-stage fallback: arboard → xclip → xsel → wl-copy → OSC 52
- `src/config.rs` — YAML via `serde_yaml_ng`
- `src/git/mod.rs` — git status queries for browser colouring and remote-ahead detection
- `src/update/` — GitHub Releases API, self-replace binary on disk
- `src/session.rs` — session persistence (currently returns defaults)
- `src/filter.rs` — file name filtering for the fuzzy finder
- `src/syntax_registry.rs` — syntect `SyntaxSet` singleton

## Key Abstractions

**`PseudoTerminal`** — wraps portable-pty + vt100 parser into one owned handle.
Fields: `parser`, `writer`, `master`, `exited`. Background threads share `Arc`
clones; the main thread accesses via `lock_or_recover()` (poison-safe). Up to 3
instances, keyed by `PaneId` in `App::terminals: HashMap<PaneId, PseudoTerminal>`.

**`JobState<T>`** — explicit lifecycle for single-shot async jobs, replacing
`Option<Receiver<T>>`. `poll()` returns `PollOutcome::{Pending, Ready(T),
Disconnected}` and auto-resets to `Idle`.

**`PaneRects`** — bundle of 6 `Rect` values recomputed per mouse event to
hit-test pane clicks: `files`, `preview`, `claude`, `lazygit`, `terminal`,
`footer`.

**`ClipboardOutcome`** — enumerates which fallback stage succeeded or why all
failed: `Arboard | Xclip | Xsel | WlCopy | Osc52 | Failed(String) | Submitted`.
`Submitted` is returned immediately when a copy is queued to the async worker;
the real outcome arrives later via `take_pending_outcome()`.

## Entry Points

- Normal start: `src/main.rs` → `async_main()` → `App::new()` + `App::run()`
- `--check-update` → `run_update_check_cli()`
- `--update-to <version>` → `run_update_to_version_cli()`
- `--clipboard-diag` → `run_clipboard_diag_cli()`
- `--ssh-paste-diag` → `run_ssh_paste_diag_cli()`

## Error Handling Paths

- PTY spawn failure: stored in `App::claude_error` / `lazygit_error` /
  `terminal_error`, shown as a pane overlay
- PTY exit: `Arc<AtomicBool>` set by the reader thread;
  `check_and_restart_exited_ptys()` respawns
- Clipboard failure: `ClipboardOutcome::Failed(msg)` triggers a
  `clipboard_error_flash` footer banner (3 s)
- Async job disconnect: `PollOutcome::Disconnected` resets the job to `Idle`;
  the UI silently returns to the previous state

## Known Anti-Patterns

- **Monolithic `App` struct** — ~65 fields, split only by `impl` blocks.
- **`lock_or_recover` poison suppression** — a poisoned mutex is recovered
  rather than propagated, which hides the original panic. The crash log
  (`src/crashlog.rs`) is the compensating control.

---

## Technology Stack

### Frameworks

| Crate | Version | Purpose |
|---|---|---|
| `ratatui` | 0.30.0 | Terminal UI framework (widgets, layout, rendering) |
| `crossterm` | 0.28.1 | Terminal I/O, keyboard/mouse events, raw mode — **pinned**, see CLAUDE.md |
| `portable-pty` | 0.8.1 | PTY creation and management |
| `vt100` | 0.16 | VT100/ANSI parser, 1000-line scrollback |
| `tokio` | 1.44.0 | `rt-multi-thread`, drives the outer `block_on` |
| `clap` | 4.5.37 | CLI flags (`derive`, `env`) |
| `tui-textarea` | git fork, branch `update-ratatui` | Inline editor, patched for ratatui 0.30 |
| `tui-markdown` | 0.3 | Markdown rendering in TUI panes |
| `pulldown-cmark` | 0.13 | Markdown parsing (CommonMark) |
| `syntect` | 5.2 | Syntax highlighting for the preview pane |
| `typst` / `typst-pdf` / `typst-library` / `typst-kit` | 0.14.2 | PDF export (feature `pdf-export`, on by default) |
| `comemo` / `ecow` | 0.4 / 0.2 | Required by the typst `World` trait |
| `serde` / `serde_yaml_ng` | 1.0.219 / 0.9 | Config serialization |
| `arboard` | 3.6 | Clipboard (`wayland-data-control`) |
| `self_update` | 0.42 | GitHub Release download, extraction, self-replace |
| `anyhow` | 1.0.98 | Error handling |
| `dirs` | 5.0 | Platform-aware directory resolution |
| `shlex` | 1.3 | Shell-style argument splitting |
| `regex` | 1.12 | Git status parsing, file filtering |
| `libc` | 0.2 | SIGTSTP suppression (one `unsafe` block in `main.rs`) |
| `tempfile` | 3 | Update staging |

Clipboard subprocess fallbacks need no crate: `xclip` → `xsel` →
`wl-copy`/`wl-paste` → OSC 52, controlled by `CLAUDE_WORKBENCH_CLIPBOARD`
(`osc52` | `arboard` | `subprocess`).

### Platform Requirements

- Rust stable (2021 edition); `cargo build` / `clippy` / `fmt` / `test`
- X11: `xclip` or `xsel` recommended (clipboard over XRDP)
- Wayland: `wl-clipboard` (`wl-copy`, `wl-paste`)
- `lazygit` on `$PATH` (LazyGit pane)
- `claude` CLI on `$PATH` (AI pane, unless `pty.claude_command` is set)
- `open` / `xdg-open` / `start` for browser and file opening
- Release targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`

---

## Conventions

### Naming

- `snake_case` module files: `job_state.rs`, `file_browser.rs`, `terminal_pane.rs`
- Descriptive compound names over abbreviations: `dependency_checker.rs`
- Module groups use `mod.rs`: `src/app/mod.rs`, `src/git/mod.rs`
- `CamelCase` types and enum variants: `PseudoTerminal`, `ClipboardOutcome::Arboard`
- Boolean predicates prefixed `is_` / `has_`: `is_running()`, `is_yolo()`
- Constructors: `new()` or descriptive (`JobState::running(rx)`)
- `SCREAMING_SNAKE_CASE` constants: `SUBPROCESS_TIMEOUT`, `STRATEGY_ENV`
- `pub(crate)` for cross-module constants: `REPO_OWNER`, `REPO_NAME`, `BIN_NAME`
- `_` prefix for intentionally unused params: `set_restrictive_permissions(_path: &Path)`
- Public API re-exported at module root via `pub use`

### Module and Function Design

- Fallible operations return `Result<T>` (anyhow) or `Option<T>`
- Infallible state mutation returns `()`
- Public API declared with `pub` at item level; crate-internal uses `pub(crate)`

### Logging

- `println!` only in CLI diagnostic modes in `src/main.rs`
- No stdout/stderr logging during TUI operation — it corrupts rendering
- Update operations → `dirs::cache_dir()/ai-workbench/update.log`
- Panics → `dirs::cache_dir()/ai-workbench/crash.log`
- Errors that cannot be surfaced are silently dropped (TUI constraint)

---

## UI Module Structure

| File | Purpose |
|---|---|
| `layout.rs` | Computes the 6-pane layout rectangles |
| `file_browser.rs` | File browser with git status colours |
| `preview.rs` | File preview, syntax highlighting, markdown |
| `terminal_pane.rs` | Renders PTY output from vt100 screen cells |
| `footer.rs` | Status bar: shortcuts, date/time, version |
| `help.rs` | Help overlay |
| `about.rs` | About dialog with license info |
| `settings.rs` | Settings menu |
| `wizard_ui.rs` | Setup wizard |
| `fuzzy_finder.rs` | Ctrl+P file finder |
| `syntax.rs` | Syntect integration |
| `drag_ghost.rs` | Drag & drop visual feedback |
| `claude_startup.rs` | AI startup prefix dialog |

**Browser module** (`src/browser/`): `opener.rs` (platform-specific file opening
via open/xdg-open/start), `markdown.rs` (Markdown → styled HTML).

---

## Update-System Testing

### CLI options

```bash
./ai-workbench --check-update                      # check without starting the TUI
./ai-workbench --check-update --fake-version 0.37.0 # simulate an older version
./ai-workbench --update-to v1.6.0                  # downgrade target (debug builds)
```

### Method 1 — downgrade and re-update (recommended)

```bash
./target/release/ai-workbench --check-update    # 1. current version
./target/release/ai-workbench --update-to v1.6.0 # 2. downgrade (signed, >= v1.6.0)
./target/release/ai-workbench                    # 3. should detect a newer version
# 4. In the Help screen (F12), press 'u' to trigger the update
```

### Method 2 — fake version

`--fake-version 0.37.0` exercises update detection without a real download.

### TUI triggers

- Automatic: update check at startup (errors are silent)
- Manual: `u` in the Help screen (F12)
- Dialog shows version and release notes when an update is available

### Log file

```bash
cat ~/Library/Caches/ai-workbench/update.log   # macOS
tail -f ~/.cache/ai-workbench/update.log       # Linux
```

### Troubleshooting

1. *No releases found* — the GitHub Release lacks assets for your platform
2. *Network errors* — check connectivity and GitHub API access
3. *Permission denied* — the binary must be writable for self-update
4. *Version mismatch* — verify with `--check-update`

### GitHub Release requirements

Tag format `vX.Y.Z`; assets named `ai-workbench-{target}.tar.gz` for the four
supported targets listed under *Platform Requirements*.
