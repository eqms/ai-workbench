# 🔍 Project Health Audit Report

Date: 30.08.2026  
Scope: `/Users/picard/gitbase/ai-workbench` only

## 📋 Project Summary

- Languages detected: Rust 2021
- Package managers: Cargo (`Cargo.toml`, committed `Cargo.lock`)
- Direct dependencies: 29 (28 runtime, including 6 optional PDF dependencies; 1 dev)
- Locked packages: 643
- Last commit: `1829854` — `[CHG] Move architecture reference out of CLAUDE.md into docs/`
- Git state at audit start: `main...upstream/main`; 20 tracked files modified and
  2 files untracked. This audit evaluates that uncommitted startup/backend work,
  not only commit `1829854`.

## 🔒 Security Findings

### 🔴 HIGH Severity

No high-confidence High or Critical findings were detected.

### 🟡 MEDIUM Severity

No high-confidence Medium findings were detected.

### ✅ No Issues Found

The reviewed security-sensitive paths already use strong controls:

- Repository-local process-spawning configuration is canonical-path and
  SHA-256 trust-pinned, parsed from the exact bytes that were checked, and its
  trust store is created with mode `0600` (`src/config.rs`).
- All internal Git invocations use the centralized hardened command builder;
  prompts, hooks and the `ext` transport are disabled (`src/git/mod.rs`).
- Self-update archives are checked against the embedded Ed25519 release key
  before installation (`src/update/install.rs`).
- Paths sent into a PTY are shell-quoted and rejected if they cannot be quoted
  safely (`src/app/pty.rs`).
- The secret scan found only CI secret references and documentation examples,
  no embedded credentials.

The live RustSec advisory scan could not be completed because the nono sandbox
blocked the advisory database under `/Users/picard/.cargo`. Restart the session with
`nono run --allow /Users/picard/.cargo -- Codex` and rerun `cargo audit` to close
that evidence gap.

## 🚧 Current Worktree Functional Findings

These are release-blocking functional findings in the uncommitted CLI/startup
implementation. They are not classified as security vulnerabilities.

### 🔴 HIGH — Initial CLI chooser can leave the AI pane without a PTY

- `App::new` deliberately delays AI PTY creation while the initial backend
  chooser is visible (`src/app/mod.rs:235-259`).
- Keyboard `Esc` correctly applies the last backend and starts the flow
  (`src/app/keyboard/dialogs.rs:307-318`).
- A mouse press while the chooser is open only closes it
  (`src/app/mouse.rs:499-503`). In startup mode no backend is applied and no PTY
  is created, leaving an empty AI pane.
- **Recommendation:** centralize chooser cancellation/confirmation semantics and
  use the same path for keyboard and mouse.

### 🟡 MEDIUM — Agent startup overlay is not mouse-modal

`agent_startup.visible` is absent from the mouse-down handling and from the
drag, release and scroll modal blocklists (`src/app/mouse.rs:499-533`,
`1032-1045`, `1118-1131`, `1198-1208`, `1263-1273`). Mouse input can therefore
change panes or file-browser state behind the visible modal. Introduce one
central `has_blocking_modal()` predicate instead of maintaining divergent lists.

### 🟡 MEDIUM — Visible overlay and keyboard owner can disagree

The update dialog is rendered before `agent_startup`, so the startup form is
visually on top (`src/app/drawing.rs:204-250`). Keyboard dispatch gives the
update dialog exclusive priority (`src/app/keyboard/mod.rs:43-47`). When both
flags are true, keys control the hidden dialog. Render order and input priority
must be identical.

### 🟡 MEDIUM — Startup selections disappear on PTY restart

The initial non-Claude start correctly combines configured command and form
arguments (`src/app/pty.rs:87-147`). Auto-restart and manual restart reconstruct
the AI command only from persistent configuration (`src/app/pty.rs:330-450`).
Sandbox, approval, model, session and similar invocation choices are lost after
the first process exit. Store the effective argv vector in application state
and reuse it for both restart paths.

### 🟡 MEDIUM — Configured and form arguments can conflict

`build_agent_command` appends form arguments to the freely configurable command
without resolving existing flags (`src/app/pty.rs:87-97`). A configured
`codex --sandbox read-only` combined with form choice `Workspace` produces two
contradictory `--sandbox` values; the CLI may reject them or apply parser-specific
precedence. Known options should be merged deterministically and covered by
conflict tests.

### 🟢 LOW — Generalized startup state retains Claude-specific names

`init_claude_after_wizard` and `claude_pty_pending` now coordinate every backend,
while some comments still state that OpenCode/Pi start directly. Rename the
generic orchestration state, but retain `PaneId::Claude` as required by the
project architecture.

## 📦 Dependency Status

The table records direct dependencies and the versions resolved by the current
lockfile. „Current“ means no clearly deprecated, abandoned, or major-version-
behind dependency was established during this audit; it is not a substitute
for the blocked advisory scan.

| Dependency | Current | Status | Action Needed |
|---|---:|---|---|
| anyhow | 1.0.103 | ✅ Current | None |
| arboard | 3.6.1 | ✅ Current | None |
| clap | 4.6.1 | ✅ Current | None |
| crossterm | 0.28.1 | ✅ Pinned | Keep at 0.28.x until the tui-textarea event types converge |
| dirs | 6.0.0 | ✅ Current | None |
| libc | 0.2.186 | ✅ Current | None |
| portable-pty | 0.9.0 | ✅ Current | None |
| pulldown-cmark | 0.13.4 | ✅ Current | None |
| ratatui | 0.30.2 | ✅ Current | None |
| regex | 1.13.0 | ✅ Current | None |
| self_update | 0.44.0 | ⚠️ Major available | 1.0 requires API migration and Rust 1.88; project MSRV is 1.85, so upgrade only in an explicit MSRV phase |
| semver | 1.0.28 | ✅ Current | None |
| serde | 1.0.228 | ✅ Current | None |
| serde_yaml_ng | 0.9.36 | ✅ Current | None |
| sha2 | 0.10.9 | ✅ Current | None |
| shlex | 1.3.0 | ✅ Current | None |
| syntect | 5.3.0 | ✅ Current | Benchmark `fancy-regex` before considering removal of native Oniguruma |
| tempfile | 3.27.0 | ✅ Current | None |
| tokio | 1.52.3 | ✅ Current | None |
| tui-markdown | 0.3.9 | ✅ Current | Keep panic fallback around third-party rendering |
| tui-textarea | 0.7.0 git rev `b6bf812d` | ✅ Pinned | Track upstream compatibility; keep exact revision |
| vt100 | 0.16.2 | ✅ Current | None |
| comemo (optional) | 0.4.0 | ✅ Current | Intentional Typst API dependency |
| ecow (optional) | 0.2.6 | ✅ Current | Intentional Typst API dependency |
| typst (optional) | 0.14.2 | ✅ Current | Update the Typst cluster together |
| typst-kit (optional) | 0.14.2 | ✅ Current | Update the Typst cluster together |
| typst-library (optional) | 0.14.2 | ✅ Current | Update the Typst cluster together |
| typst-pdf (optional) | 0.14.2 | ✅ Current | Update the Typst cluster together |
| ttf-parser (dev) | 0.25.1 | ⚠️ Unmaintained | RustSec RUSTSEC-2026-0192; evaluate `skrifa` for the font-coverage test |

Intentional duplicate transitive versions, including crossterm 0.28/0.29 and
comemo 0.4/0.5, are documented in `deny.toml`; consolidation should wait for
compatible upstream releases and full terminal regression testing.

Two High-severity quick-xml advisories are explicitly ignored in
`.cargo/audit.toml` and `deny.toml`: RUSTSEC-2026-0194 and RUSTSEC-2026-0195.
The documented reachability analysis covers Typst bibliography parsing and a
Wayland build-time path, but `cargo tree` also shows `quick-xml 0.38.4` through
`self_update 0.44.0`. The application uses self_update's GitHub backend, so this
audit did not establish an exploitable XML input path; nevertheless the ignore
rationale is incomplete and must trace this third dependency path before the
risk acceptance is considered current. RustSec fixes both classes in
quick-xml 0.41 or later.

## 💡 Library Recommendations

| Current | Suggested Alternative | Reason |
|---|---|---|
| syntect `regex-onig` | syntect `fancy-regex` feature | Potentially simpler cross-platform builds without native Oniguruma; adopt only after controlled highlighting and startup benchmarks |
| self_update 0.44 | self_update 1.x | Stable API and newer dependency graph; requires a planned Rust 1.88 MSRV bump and documented API migration |
| ttf-parser | skrifa | RustSec identifies ttf-parser as unmaintained; skrifa is the stated maintained alternative |
| Manual dependency review | Dependabot or Renovate | Automated, reviewable update PRs would complement the existing Cargo lockfile, RustSec and cargo-deny gates |
| Monolithic `App` state | Focused state aggregates over time | Reduces cross-module coupling as new startup and async states are added; perform incrementally, not as a broad rewrite |

## 🛠️ Code Quality Notes

- CI is strong: formatting, Clippy with warnings denied, tests, RustSec and
  cargo-deny are already configured across features.
- Error handling is generally defensive on security-sensitive paths, and the
  TUI-specific panic strategy correctly separates UI-thread restoration from
  background failures.
- `App` remains a large shared state object used through many `impl App`
  modules. New modal state should stay self-contained and pure where possible.
- `src/ui/agent_startup.rs` is already about 900 lines and combines backend
  capability data, text editing, argv generation and rendering. Split data from
  state/rendering before adding more CLIs.
- The new startup UI has state-level tests but no Ratatui `TestBackend` coverage
  for 80×24/small terminals, no mouse-flow tests, and no complete per-backend
  argv matrix. These tests are especially important because layout clipping and
  modal ownership are the regressions found in this audit.
- Add automated dependency-update PRs; keep the deliberate crossterm and
  git-revision pins protected from blind upgrades.
- Static gates passed: `cargo fmt --all -- --check`, `git diff --check`, Cargo
  metadata and dependency-tree inspection. Compilation, tests and Clippy remain
  unverified because nono blocks `/Users/picard/.cargo/shared-target`.
- Complete a live `cargo audit` after restarting nono with Cargo advisory-db
  access, and correct the quick-xml ignore's incomplete dependency-path record.

## 📊 Overall Health Score: 7.5/10

The project has unusually mature terminal-failure handling, configuration
trust boundaries, signed self-updates, and dependency policy for a TUI tool.
The committed baseline remains healthy, but the current uncommitted startup
feature is not release-ready because it contains one High-impact interaction
bug and several Medium reliability defects.
Top priorities:

1. Fix mouse cancellation/modal ownership and align overlay render/input order.
2. Preserve effective startup argv across restarts and resolve duplicate flags.
3. Run the blocked build/test/Clippy/audit gates, then add terminal-size,
   keyboard and mouse regression tests before release.
