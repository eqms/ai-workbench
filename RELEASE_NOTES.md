# Release Notes

## Version 1.13.0 (30.08.2026)

### Added

- **[ADD] Interactive AI CLI launcher and backend-specific welcome screens.**
  Starting without a positional backend now opens a chooser before any AI PTY
  is spawned. Claude keeps its structured permission/model/effort screen;
  Codex, OpenCode, Pi and the new Antigravity (`agy`) backend offer wide,
  Claude-style forms with Tab-selectable CLI-specific sections and editable
  model/agent fields. Runtime switching with F8 uses the same flow, dangerous
  options are visibly marked, command paths remain editable in Settings, and
  existing config files migrate through Serde defaults.

### Fixed

- **[FIX] Dismissing the initial CLI chooser with the mouse no longer leaves
  the AI pane without a terminal.** The chooser deliberately delays spawning
  the AI PTY until a backend is picked. Pressing `Esc` handled that correctly —
  it applied the configured backend and started the process — but clicking
  outside the menu only closed it, so the pane stayed empty for the rest of the
  session and no keystroke could bring it back. Both input paths now go through
  one shared cancel routine, so they cannot drift apart again; cancelling an F8
  switch still just closes the menu, since a terminal already exists in that
  case.

### Changed

- **[CHG] Dependencies refreshed; one advisory closed.** `h2` was two weeks
  behind a denial-of-service advisory (RUSTSEC-2026-0258), reachable only
  through the update check. The refresh also clears an unsound `lru` release
  and two yanked crates, and drops the vulnerable `quick-xml` copy that the
  clipboard stack pulled in. `cargo audit` reports no vulnerabilities. The
  ignore rationales in `.cargo/audit.toml` and `deny.toml` were corrected to
  match the one path that genuinely remains.

## Version 1.12.1 (27.08.2026)

### Fixed

- **[FIX] The click that brings the window forward no longer answers a
  prompt.** Since v1.11.0 mouse clicks are forwarded to applications that track
  the mouse, and Claude Code enables tracking for its question dialogs. Coming
  back from another application and clicking into the pane therefore submitted
  an answer instead of just activating the window — the click was meant for the
  window, not for the prompt. Focus reporting (DECSET 1004) is now enabled, so
  a button press within 500 ms of the window regaining focus only moves focus to
  the pane under the cursor; its drag and release events are dropped with it, so
  an application that never saw the press does not see a release either. The
  next click reaches the application normally. Wheel events are never
  suppressed, clicks inside the workbench are unaffected, and terminals without
  focus reporting behave exactly as before.

## Version 1.12.0 (27.08.2026)

Two defects that made the application look broken from the outside: a caught
rendering panic still tore the terminal down, and a multi-line paste into the
AI pane arrived truncated.

### Fixed

- **[FIX] A caught panic no longer destroys the terminal.** Markdown rendering
  runs inside `catch_unwind` precisely so a library panic degrades to raw text
  instead of taking the pane down. But `catch_unwind` only stops the unwind —
  the panic hook still runs first, and on the UI thread it restored the
  terminal: alternate screen left, raw mode off, while the event loop kept
  drawing over the shell's scrollback. The fallback worked; the display did
  not. `crashlog::expect_panic()` now marks such a scope, and a panic inside it
  is written to the crash log without touching the terminal and without
  flagging the session as crashed. v1.11.1 fixed the same corruption for
  background threads; this closes the caught-panic case on the UI thread.

- **[FIX] Markdown with a task list after a code block no longer panics.**
  tui-markdown 0.3.8 inserted a task-list marker at index 1 of a line that had
  no spans (`insertion index (is 1) should be <= len (is 0)`), which is what
  triggered the crash above when browsing plan documents — 70 files in a single
  local tree hit it. Raised to 0.3.9, which fixes the case; the `catch_unwind`
  guard stays, because a rendering panic must never take the pane down.

- **[FIX] Multi-line paste into the AI pane is no longer truncated.** Cmd+V
  (and any terminal-side paste) reached the Claude/OpenCode/Pi/Codex pane as
  raw bytes, on the assumption that the CLI does not understand bracketed
  paste. Claude Code does request it (`ESC[?2004h`, verified against 2.x), so
  every newline in the pasted block acted as Enter: the first line was
  submitted and the rest was lost. Ctrl+V was unaffected because it forwards
  `0x16` and lets the CLI read the clipboard itself — which is why the two
  paths behaved differently. All three PTY panes now share
  `PseudoTerminal::send_paste()`, which wraps the text only when the inner
  application has announced bracketed paste — the same "the inner app decides"
  rule the mouse routing already follows. Paste markers inside the payload are
  stripped so they cannot end the block early and let the remainder run as
  typed input.

## Version 1.11.1 (26.08.2026)

A panic in a background thread no longer destroys the running UI, and every
panic is now written to a crash log.

### Fixed

- **[FIX] A background-thread panic no longer tears down the terminal.**
  `ratatui::init()` installs a panic hook that restores the terminal on *any*
  panic — including one on a PTY reader thread, the clipboard worker or a
  git-check thread, none of which end the process. The result: the alternate
  screen was left, raw mode and mouse capture were switched off, and the event
  loop kept drawing — painting the UI over the shell's scrollback while the app
  stopped responding to input. It looks exactly like a hard crash, but the
  process is still alive, and the panic message that would have explained it is
  overpainted by the next frame. The hook is now re-installed after
  `ratatui::init()` and only the UI thread may touch the terminal; a background
  panic takes down its own thread and nothing else.

### Added

- **[ADD] Crash log.** Every panic — foreground or background — is appended to
  `crash.log` in the platform cache directory (macOS
  `~/Library/Caches/ai-workbench/crash.log`, Linux
  `~/.cache/ai-workbench/crash.log`) with version, thread, location, message
  and a full backtrace. A TUI overpaints anything written to stderr, so without
  a file on disk a crash left no evidence at all.
- **[ADD] Footer banner for background failures.** When a background thread
  dies, the footer shows a persistent red `⚠ internal error — see crash.log`.
  Without it the loss is silent: the affected pane simply stops updating.

## Version 1.11.0 (15.08.2026)

Mouse clicks now reach the application running inside a pane.

### Fixed

- **[FIX] Clicks reach Claude Code and lazygit.** Claude Code's "Jump to bottom
  (click)" button did nothing in the workbench — and so did every other
  clickable element in Claude Code and lazygit. v0.96.0 taught the workbench to
  forward the mouse *wheel* to an inner application that enables mouse tracking,
  but buttons were never forwarded: a left click was consumed locally for focus
  and text selection, so the `ESC [ <0;col;row M` report the application waits
  for was never sent. Press, drag and release are now routed the same way the
  wheel already was — if the inner application requested mouse tracking
  (DECSET 1000/1002/1003), the event goes to it; otherwise nothing changes and
  the local selection keeps working as before. The press captures the pane, so
  a drag that leaves the pane still reports to the application that received it,
  and the protocol mode is respected (no release reports to an X10-mode app, no
  motion reports without 1002/1003).

### Changed

- **[CHG] In mouse-aware panes the mouse belongs to the application.** Where an
  inner application tracks the mouse, the workbench no longer starts its own
  text selection — Claude Code marks text itself, and that selection was what
  the workbench had been overriding all along. Panes without mouse tracking —
  a plain shell, the preview pane — are untouched: click and drag still selects
  and copies to the clipboard exactly as before.

### Security

- **[ADD] Self-update now verifies release signatures (SEC-01 Half 2 — finding
  closed).** Archives have been signed in CI since v1.6.0, but the client
  installed whatever GitHub served: a compromised release asset would have
  installed itself on every machine at the next auto-update.
  `src/update/install.rs` now embeds `signing/ai-workbench-pub.bin` as
  `RELEASE_PUBLIC_KEY` and passes it to both `Update::configure()` chains, so
  `self_update` rejects any archive that is unsigned or signed with another
  key. The published v1.10.1 asset was verified against the committed key
  before enabling this, confirming binary and CI use the same key. Two
  consequences: signing is now **mandatory** in the release workflow (missing
  `ZIPSIGN_PRIVATE_KEY` fails the job instead of publishing an uninstallable
  release), and `--update-to` targets must be **v1.6.0 or newer**, since
  earlier releases predate signing.
- **[FIX] `SECURITY-NOTES.md` no longer reports two fixed issues as open.**
  "Shell Fallback in Dependency Probe" and "Predictable Temp File Path" were
  still listed as `(MEDIUM — open)` although both were closed in v0.90.0 —
  the shell fallback is gone from `dependency_checker.rs`, and
  `pdf_export.rs:130` uses `tempfile::Builder…tempfile_in()` with `O_EXCL`.
  Both moved to "Closed Findings" with the verifying evidence.

### Internal

- **[CHG] `portable-pty` 0.8.1 → 0.9.0**, which drops the `serial` crate
  (unmaintained since 2017, RUSTSEC-2017-0008) in favour of `serial2` and
  pulls `nix` up to 0.28. No source change was required. Removes the only
  `cargo audit` warning that reached the project through a direct dependency.
- `encode_mouse_button_event()` and `mode_reports()` in `src/terminal.rs`
  (pure, unit-tested) next to the existing `encode_wheel_event()`;
  `PseudoTerminal::send_mouse_button()` writes them past `write_input()` so the
  scrollback position survives, as `send_mouse_wheel()` already did.
- `App::forward_mouse_button()` / `App::pty_pane_rect()` in `src/app/mouse.rs`,
  new `App::pty_mouse_capture: Option<PaneId>` field.
- `wheel_coords_in_pane()` renamed to `pane_cell_coords()` — it now serves
  buttons as well as the wheel.
- 267 unit + 3 CLI tests pass (+4).

## Version 1.10.1 (05.08.2026)

Follow-up review of the v1.10.0 security work. One functional regression, one
gap in the git hardening, and three parsing/robustness defects.

### Fixed

- **[FIX] Pulling is possible again.** v1.10.0 turned `git.auto_fetch` off by
  default, which also removed the only path to `git pull`: the pull dialog was
  raised solely by the automatic remote check, so with the new default nothing
  could reach it. The `GitConfig::auto_fetch` doc claimed "Manual pull (`Ctrl+G`)
  is unaffected", but no such binding exists — `g` in the file menu is "Go to
  path". There is now a real entry: **F9 → `p`**, which asks before pulling and
  works regardless of `auto_fetch` (that setting governs whether *navigating*
  into a repository talks to the network, not whether you may ask it to).
- **[FIX] `git pull` no longer runs the repository's hooks.** `git_command()`
  pinned six config keys but not `core.hooksPath`, so confirming the pull dialog
  in a tree you had only navigated into executed `.git/hooks/post-merge`. Clones
  do not carry hooks, but an unpacked archive containing `.git/` does. Now
  pointed at a path that is never a directory, verified against a real
  `post-merge` hook. Hooks are disabled for *our* git calls only — the Terminal
  and LazyGit panes run git unpinned.
- **[FIX] The trust check and the config load no longer read the file twice.**
  `local_config_status()` hashed one read of `./config.yaml` and
  `load_config_checked()` then parsed a second, so what was approved and what was
  loaded could differ. New `read_and_classify_local_config()` reads once and
  hands the bytes to the caller; `save_config()` likewise pins the bytes it just
  wrote instead of re-reading them.
- **[FIX] Paths with non-ASCII characters get their git color back.**
  `core.quotePath` was left at its default, so `git status --porcelain` returned
  `"\303\234bung.txt"` for `Übung.txt` — a path that never matches anything on
  disk, leaving the file rendered as clean. Now pinned to `false`.
- **[FIX] A localized git no longer turns a missing remote into an error.** The
  "no remote configured" branch matches on English stderr (`Could not resolve`);
  under a German locale it fell through to the error path. `git_command()` now
  sets `LC_ALL=C`.
- **[FIX] An unreadable-but-present `config.yaml` is reported.** When
  `canonicalize()` failed, `local_config_status()` returned `Absent`, which
  suppressed the "your config is being ignored" warning. It now returns
  `Untrusted` — still fail-closed, but visible. Trusting and loading also agree
  on UTF-8 handling now, so a file with invalid UTF-8 can no longer be approved
  and then abort startup.

### Changed

- `MenuBar` indices are derived from a single `ITEM_COUNT` constant with a
  `debug_assert` against the rendered list; the wrap-around bounds were
  duplicated magic numbers. 4 new unit tests (263 total).

## Version 1.10.0 (29.07.2026)

Security release. Two issues let a directory you merely *open* decide what code
ai-workbench runs. Both are fixed by making the dangerous behavior opt-in, so
this release changes defaults — see "Changed" for what you may need to re-enable.

### Security

- **[FIX] A repository-local `config.yaml` is no longer trusted automatically.**
  `load_config()` read `./config.yaml` from the working directory with the
  highest priority and no provenance check. That file sets `pty.claude_command`,
  `pty.lazygit_command` and `terminal.shell_path`, which are spawned as processes
  at startup — so cloning an untrusted repository and starting the workbench in it
  was enough to run attacker-chosen binaries in all three panes, silently.

  A repo-local config is now ignored until you approve it once:

  ```bash
  ai-workbench --trust-local-config    # review the file first, then approve
  ```

  The approval is pinned to the file's exact content (SHA-256) and its
  canonical path, recorded in `~/.config/ai-workbench/trusted_configs.yaml`
  (mode 0600). Editing the file — or a `git pull` that rewrites it — drops the
  approval, so a config cannot be swapped out under an existing trust. When a
  local config is skipped, startup prints a warning to stderr naming the file and
  the command to approve it. Saving settings writes to the repo-local file only
  while it is trusted, and re-pins the hash afterwards; otherwise settings go to
  the XDG config as before.

- **[FIX] `git fetch` no longer runs automatically when browsing into a repository.**
  Entering a directory in the file browser triggered an immediate background
  `git fetch`, which executes the *target repository's own* configuration.
  `remote.<name>.url = ext::sh -c '…'`, `core.sshCommand`, `core.gitProxy` and
  `credential.helper` are all command-execution vectors, so navigation alone was
  enough to run code chosen by the browsed tree. Git's `safe.directory` guard does
  not cover this: it only rejects repositories owned by a *different* user, and an
  unpacked archive or fresh clone is owned by you.

  Auto-fetch is now off by default and opt-in per config:

  ```yaml
  git:
    auto_fetch: true   # only for trees you control
  ```

  Local git status colors and the branch indicator are unaffected — they never
  needed the network. Manual pull (`Ctrl+G`) is unchanged.

- **[FIX] All git invocations are hardened against repository-supplied config.**
  Every `git` call now pins `core.fsmonitor=false`, `core.sshCommand=ssh`,
  `core.gitProxy=`, `core.pager=cat`, `credential.helper=` and
  `protocol.ext.allow=never` on the command line, where repo config cannot
  override them, plus `GIT_TERMINAL_PROMPT=0` / `GIT_ASKPASS=` / `GIT_PAGER=cat`
  to keep git non-interactive. Verified against a repository configured to run a
  `core.fsmonitor` helper on `git status` and an `ext::sh -c` remote: both execute
  without the flags and are blocked with them.

  This shrinks the surface but does not close it — `.gitattributes` plus a
  `filter.<name>.clean` entry can still run a command during `git status`, and git
  offers no single switch to disable all filters. That residual risk is why
  `git.auto_fetch` defaults to off rather than relying on hardening alone.

### Changed

- **[CHG] Dependency advisories are now triaged in-repo.** `.cargo/audit.toml` and
  the `[advisories] ignore` list in `deny.toml` document two `quick-xml` DoS
  advisories (RUSTSEC-2026-0194 / -0195) as unreachable, with the dependency paths
  that make them so: typst's CSL bibliography parser, which never sees user input
  because the Typst source is generated internally, and the build-time
  `wayland-scanner` proc-macro. Neither is resolvable via `cargo update`
  (`citationberg` pins `^0.38`, `wayland-scanner` pins `^0.39`). Re-verify with
  `cargo tree -i quick-xml` whenever the lockfile changes.
- **[CHG] `timeout-minutes` added to the `audit` and `deny` CI jobs**, the two that
  were missed in the v1.9.1 sweep.
- New dependency: `sha2` 0.10, used only for content-pinning the config trust
  allowlist.

### Fixed

- **[FIX] The SSH image-paste setup instructions no longer assume a macOS client.**
  Wizard step "SSH Image Paste", `--ssh-paste-diag` and the USAGE.md sections told
  every user to run `brew install shunmeicho/tap/cc-clip` "on your Mac". The wizard
  runs on the *remote* host and cannot know the client's OS, so Linux users were
  handed a command that does not exist for them. The instructions now say "on your
  local machine" and list both installs (`brew` for macOS, `cargo install cc-clip`
  for Linux). Text only — no behavior change; the helper detection, the port-9998
  reachability check and the `[m] mark as configured` flag are untouched.
- **[FIX] The remote export/preview transfer no longer calls the SSH client a Mac
  either.** The iTerm2 OSC 1337 file transfer works from any host running iTerm2
  or WezTerm — WezTerm ships for Linux — but the footer flash, `--open-diag`
  output and USAGE.md all said "your Mac". Now "your local machine" / "local
  `~/Downloads`". Text only.

## Version 1.9.4 (28.07.2026)

### Added

- **[ADD] Two new AI backends: `ollama-opencode` and `ollama-pi`.** You can now launch OpenCode and Pi through Ollama directly from the workbench. Both appear as separate entries in the F8 backend menu ("OllamaOC" / "OllamaPi") and have their own dedicated Settings fields under Shift+F8 → Paths: "Ollama OpenCode Command" and "Ollama Pi Command". Defaults are `ollama launch opencode` and `ollama launch pi`; add `--model ...` in Settings or `config.yaml` to target a specific model, e.g.:
  ```yaml
  pty:
    ollama_opencode_command: ["ollama", "launch", "opencode", "--model", "kimi-k2.7-code:cloud"]
    ollama_pi_command: ["ollama", "launch", "pi", "--model", "qwen3.5:cloud"]
  ```
- The CLI accepts `ai-workbench ollama-opencode` and `ai-workbench ollama-pi`; session persistence stores them as `ollama-opencode` / `ollama-pi`.
- The setup wizard now detects `ollama`, lets you configure the Ollama path, and picks the new backends with keys `5` and `6`.

## Version 1.9.3 (28.07.2026)

### Added

- **[ADD] Settings text fields now support a real cursor, arrow navigation, and Command/Ctrl+V paste.** Previously the Settings dialog (Shift+F8) used a plain append-only input buffer: Backspace always removed the last character, there was no cursor position, and paste only worked with `Ctrl+V`. The editor now tracks a char-based cursor, renders the character at the cursor with a reversed-video block (or a trailing block cursor when at the end), and supports `←`/`→`, `Home`, `End`, `Delete`, `Backspace`, plus `Ctrl+V` and `Cmd+V` on macOS. This makes editing long command lines like `opencode --model kimi-k2.7-code:cloud` or `pi --model qwen3.5:cloud` in F8 → Settings → Paths much less frustrating.

### Changed

## Version 1.9.2 (28.07.2026)

### Changed

- **[CHG] Resolve all outstanding `cargo clippy -- -D warnings` failures.** The project no longer builds with the strict clippy profile due to 11 lint errors in test modules and initialization patterns: `items_after_test_module` (`src/browser/opener.rs`, `src/update/check.rs`), `field_reassign_with_default` in five test helpers (`src/config.rs`, `src/setup/wizard.rs`, `src/ui/settings.rs`), and `useless_vec` in semver-selection tests (`src/update/check.rs`). All affected code now uses struct-literal initialization with `..Default::default()`, places test modules at the end of their files, and replaces single-use `vec!` literals with arrays. No runtime behavior changed; this is a maintenance/CI-hardening release.

## Version 1.9.1 (17.07.2026)

### Changed

- **[CHG] CI hardening: `timeout-minutes` on all workflow jobs** (CI-only, no release tag — takes effect from the next `v*` tag). The v1.9.0 release run lost 45 minutes to a hung `rustup` install on the `aarch64-pc-windows-msvc` runner before GitHub reaped the job (no logs uploaded, Create Release + Homebrew jobs skipped). Build jobs in `release.yml` now time out after 25 minutes (~2× the longest observed build, 14 min), Create Release and Update Homebrew Formula after 10; `ci.yml` check/test/clippy get 25, fmt 10. A stuck runner now fails fast and can be retried immediately via `gh run rerun --failed`.

## Version 1.9.0 (17.07.2026)

### Added

- **[ADD] Immediate boot screen — no more black screen at startup.** Previously the terminal went black for 3–5 seconds between launch and the intro animation: `ratatui::init()` blanks the screen (alternate screen), and only afterwards ran the kitty-keyboard probe (up to ~2 s on unresponsive terminals) and `App::new()` synchronously before the first frame. The new `ui::boot_screen` module paints an instant frame right after terminal init — the "AI WORKBENCH" block wordmark in the intro's stabilized cyan style, version, and a status line ("probing terminal..." → "initializing panes...") — so startup shows immediate feedback and flows seamlessly into the intro animation. `App::new` duration is now logged to `update.log` for startup profiling.

### Changed

- **[CHG] Startup dependency check moved off the critical path.** `DependencyReport::check()` (~12–20 sequential subprocess spawns probing git/claude/opencode/pi/codex/lazygit and shells) ran synchronously — and, due to `WizardState`'s eager `Default`, effectively **twice** on every launch (three times on first run). It now runs once on a background thread via the existing `JobState` pattern (`check_async()`, polled in the event loop); `WizardState::default()` no longer spawns any subprocesses, and only the first-run wizard performs a synchronous check when it actually opens. The Linux clipboard-helper warning banner is seeded when the background check completes.

### Fixed

- **[FIX] Intro animation no longer starts before it is visible.** `IntroState`'s clock began mid-`App::new()`, so construction time silently consumed part of the ~4.5 s animation budget — the intro appeared to start partway through. `App::run` now re-anchors the clock (`IntroState::restart()`) right before the first frame, so the full glitch → sweep → stabilize sequence plays from the moment it becomes visible.

## Version 1.8.0 (16.07.2026)

### Added

- **[ADD] Codex (OpenAI) as fourth AI backend.** The AI pane can now run OpenAI's `codex` CLI alongside Claude Code, OpenCode, and Pi: launch with `ai-workbench codex`, or switch at runtime via the F8 backend menu (now 4 entries). Codex starts directly (like OpenCode/Pi — no startup dialog); its own flags (`-s` sandbox mode, `-a` approval policy, `-m` model, `--search`) are configurable via the new `pty.codex_command` config field, editable under F8-Settings → Paths → "Codex Command". The setup wizard gained a Codex CLI detection line, path field, backend choice `4`, and confirmation entry. The selected backend persists in `session.yaml` as before (`last_backend: codex`). New `AiBackend::Codex` variant flows through the existing data-driven F8 menu/footer/pane-title/respawn plumbing; Claude-specific paths (permission dialog, daily `claude update`) stay Claude-only via `supports_claude_flags()`. New config field is serde-defaulted (`["codex"]`) — existing config.yaml files load unchanged. 1 new unit test plus extended backend/menu/wizard tests (240 total).

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
