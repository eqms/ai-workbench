# CLAUDE.md

Guidance for Claude Code working in this repository.

**Reference material lives in `docs/` — read on demand, not by default:**
- `docs/ARCHITECTURE.md` — component map, layers, data flow, key abstractions,
  tech stack, naming conventions, UI module list, update-system testing.
  *Read when touching pane layout, adding a module, or changing the event loop.*
- `README.md` / `USAGE.md` — user-facing features and the full shortcut tables.
- `RELEASE_NOTES.md` — version history. Feature lists belong there, not here.

## Project

A Rust TUI multiplexer: file browser with preview, three embedded PTY panes
(AI agent, LazyGit, system terminal), mouse + keyboard navigation, scrollback.
Built with Ratatui, Crossterm, portable-pty. Single binary via GitHub Releases
with self-update.

**Core value:** one terminal, all panes pointing at the same working directory.
If everything else fails, the three PTY panes must stay reliably interactive and
synchronized.

The primary (AI) pane is backend-selectable via a positional launch argument
(`ai-workbench claude|opencode|pi`, see `src/backend.rs`). **`PaneId::Claude` is
retained as the internal identifier for the AI pane regardless of backend** —
do not rename it. `AiBackend::supports_claude_flags()` gates Claude-only flags
and dialogs; all other panes are backend-agnostic.

## Constraints

- **Tech stack locked**: Rust 2021, Ratatui 0.30, Crossterm 0.28.1 (pinned),
  portable-pty, vt100, tokio multi-thread — no migration without an explicit
  phase for it.
- **Platform**: Linux + macOS only. XRDP and Kitty are first-class targets.
- **Compatibility**: the existing `config.yaml` format must be preserved or
  migrated transparently — users rely on persistent settings.
- **Performance**: 16 ms event-loop polling; PTY reader threads must never block
  the UI; clipboard work stays off the UI thread.

## Git Push Strategy

**Dual-remote — always push to both:**

```bash
git push origin main      # GitLab: gitlab.ownerp.io
git push upstream main    # GitHub: github.com/eqms/ai-workbench.git
```

Both must stay in sync; GitHub carries the Open Source distribution and the
pre-built binaries via GitHub Actions.

## Development Commands

```bash
cargo build / cargo run / cargo build --release
cargo run -- --config path/to/config.yaml
cargo test / cargo test test_name / cargo test -- --nocapture
cargo check / cargo clippy / cargo fmt / cargo fmt -- --check
```

Formatting is enforced by `rustfmt.toml` (edition 2021, reordered imports and
modules, Unix newlines) and `clippy.toml` (cognitive-complexity 30,
too-many-arguments 8, type-complexity 300 — deliberately relaxed).

## Rules That Are Easy To Get Wrong

These exist because each one has already cost a debugging session.

### Panic hooks are installed twice — do not "clean that up"

`ratatui::init()` registers its own hook that restores the terminal on *any*
panic, regardless of thread. That is wrong here: a PTY reader, the clipboard
worker and the git-check threads can panic without ending the process, and
restoring the terminal from them leaves the alternate screen and disables raw
mode *while the event loop keeps drawing* — the UI paints over the shell's
scrollback and goes deaf to input. It looks like a hard crash; the process is
still running.

Therefore, in `src/crashlog.rs` / `src/main.rs`:
- `crashlog::install_panic_hook()` is called **twice** — once early, and again
  **after `ratatui::init()`** to replace ratatui's hook. Do not drop the second
  call, and do not chain onto the previous hook (that keeps ratatui's
  unconditional restore alive and double-logs every panic).
- Only the UI thread may run `restore_terminal()` or write to stderr.
- A background panic surfaces as a red footer banner
  (`⚠ internal error — see crash.log`), because the failure is otherwise
  invisible — the affected pane just stops updating.

Crash log: `~/Library/Caches/ai-workbench/crash.log` (macOS),
`~/.cache/ai-workbench/crash.log` (Linux). stderr is worthless in a TUI, so this
file is the only post-mortem evidence.

### All git calls go through `git_command()`

`git_command()` in `src/git/mod.rs` pins `core.fsmonitor`, `core.sshCommand`,
`core.gitProxy`, `core.pager`, `credential.helper` and `protocol.ext.allow` on
the command line so repository config cannot override them. Add new git calls
through this helper, **never** via a bare `Command::new("git")`.

The surface is reduced, not closed: `.gitattributes` + `filter.<name>.clean` can
still execute during `git status`. That is why `git.auto_fetch` defaults to off
— `git fetch` would execute the browsed repository's own config.

### Repo-local `config.yaml` requires explicit approval

`config.yaml` sets `pty.*_command` and `terminal.shell_path`, which are spawned
as processes at startup. Loading whatever file sits in the working directory
would make "start the tool in a cloned repo" equivalent to running its code.

A local config is ignored until approved once via
`ai-workbench --trust-local-config`. Approval pins the canonical path plus a
SHA-256 of the content in `~/.config/ai-workbench/trusted_configs.yaml`
(mode 0600); any later edit invalidates it. The decision lives in `config.rs` —
`local_config_status()` for the I/O wrapper, `classify_local_config()` for the
pure, testable core.

Config search order: `./config.yaml` (only when approved) →
`~/.config/ai-workbench/config.yaml` → built-in defaults.

### `crossterm` must stay at 0.28.x

The `tui-textarea` fork (branch `update-ratatui`) imports crossterm 0.28 event
types. Bumping to 0.29 breaks every `editor.input(Event::Key(...))` call site.
`tui-textarea` comes from a git fork, not crates.io — **`Cargo.lock` must stay
committed.**

### Paste is bracketed only when the inner app asks for it

All three PTY panes go through `PseudoTerminal::send_paste()`, which wraps text
in `\x1b[200~…\x1b[201~` **only** when the inner application requested bracketed
paste (DECSET 2004, read via `wants_bracketed_paste()`) — the same "the inner
app decides" rule as mouse routing. Claude Code does ask for it (verified
against 2.x); the earlier assumption that it does not made every newline in a
pasted block act as Enter, so a multi-line paste was submitted line by line and
looked truncated. Paste markers inside the payload are stripped so they cannot
end the block early.

### Self-update rejects releases older than v1.6.0

Since v1.11.0 every archive is verified against the embedded release key
(`signing/ai-workbench-pub.bin`). Releases before v1.6.0 predate signing and are
rejected with a signature error. **That rejection is the feature working, not a
bug** — relevant when testing downgrades via `--update-to`.

### Border accounting and PTY sizing

Terminal panes have 1 px borders on all sides. When resizing a PTY, subtract 2
from both width and height. Resize happens during every `draw()` call before
rendering, so dimensions stay correct across window resizes.

`vt100::Parser::new(rows, cols, 1000)` — the third argument is the scrollback
depth. Fish gets `fish_features=no-query-term` to suppress its DA query, which
otherwise causes rendering artifacts.

### Directory sync and scrollback reset

When the file browser changes directory, `App::sync_terminals()` sends
`cd "path"\r` to the Terminal and AI panes. Terminal panes reset scrollback to 0
when the user types (in `PseudoTerminal::write_input`), so typed input always
appears at the bottom.

## Configuration

```yaml
terminal:
  shell_path: "/bin/bash"
  shell_args: []
ui:
  theme: "default"
git:
  # Run `git fetch` when the file browser enters a repository.
  # Off by default: fetch executes the target repo's own config
  # (`remote.<n>.url = ext::…`, `core.sshCommand`, `credential.helper`),
  # so auto-fetch would let any browsed directory run code.
  auto_fetch: false
```

Session persistence (`src/session/mod.rs`) is stubbed and currently returns
default state.

## Conventions

- Commit prefixes: `[ADD]` new features, `[CHG]` modifications, `[FIX]` bug fixes
- Version headers: increment the version number, update the date (DD.MM.YYYY)
- UTF-8 everywhere; use Unicode-aware string functions, avoid byte-level
  operations on UTF-8 strings
- German for communication, English for code and documentation
- German typography in generated documents: quotation marks always as the proper
  pair „…“ — never an ASCII `"` as the closing quote (it breaks PDF export)
- Never commit automatically unless explicitly requested
