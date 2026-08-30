# 🔍 Project Health Audit Report — Follow-up

Date: 30.08.2026
Scope: `/Users/picard/gitbase/ai-workbench` only

This is a second, independent audit run on the same day as
`PROJECT_AUDIT_2026-08-30.md`. It exists to close that report's stated evidence
gap — the RustSec advisory scan and the compile/test/Clippy gates were blocked
by the sandbox at the time — and it corrects one factual claim in it. Read both
together; this document does not restate findings it agrees with.

Unlike the first report, every finding here was executed and observed, not
inferred.

## 📋 Project Summary

- Languages detected: Rust 2021
- Package managers: Cargo (`Cargo.toml`, committed `Cargo.lock`)
- Direct dependencies: 28 runtime (6 of them optional, behind `pdf-export`) + 1 dev
- Locked packages: 645 (643 before the update described below)
- Toolchain used: rustc 1.94.1 / cargo 1.94.1; declared MSRV 1.85
- Last commit: `1829854` — `[CHG] Move architecture reference out of CLAUDE.md into docs/`
- Git state at audit start: 20 tracked files modified, 2 untracked. The audit
  covers that uncommitted startup/backend work as well as the committed baseline.

## 🔒 Security Findings

### 🟡 MEDIUM — `h2` denial-of-service advisory (RESOLVED during this audit)

`cargo audit` completed successfully and reported exactly one vulnerability:

- Crate: `h2 0.4.15`, advisory RUSTSEC-2026-0258 (dated 17.08.2026),
  "unbounded empty DATA frames", patched in `>= 0.4.16`.
- Dependency path, confirmed with `cargo tree -i h2`:
  `ai-workbench` → `self_update 0.44.0` → `reqwest 0.13.4` →
  `hyper 1.10.1` / `hyper-rustls 0.27.9` → `h2`.
- Reachability: only through the update check and download against the GitHub
  Releases API (`src/update/check.rs`, `src/update/install.rs`). Exploitation
  requires a malicious or compromised endpoint. Because update work runs on a
  background thread and the project's panic strategy keeps background failures
  from killing the process, the blast radius is a stalled update thread — not
  RCE and not a process crash. Hence Medium, not High.

**Resolved.** A plain `cargo update` was run on 30.08.2026 and lifts `h2` to
0.4.19. `cargo audit` afterwards reports **zero vulnerabilities**.

The same update cleared three further advisories that no ignore comment had
recorded:

| Crate | Was | Now | Advisory |
|---|---|---|---|
| `lru` (via `ratatui-core`) | 0.18.1 | 0.18.3 | RUSTSEC-2026-0253, unsound `pop()`, patched `>= 0.18.2` |
| `spin` | 0.9.8 | 0.9.9 | was yanked |
| `chacha20` | 0.10.1 | 0.10.2 | was yanked |
| `serial` (via `portable-pty`) | 0.4.0 | — | no longer in the tree at all |

Five unmaintained-crate warnings remain, all transitive through `syntect` and
`typst`, none with an exploitable path here: `bincode`, `paste`, `rustybuzz`,
`ttf-parser`, `yaml-rust`.

### 🟠 STRUCTURAL — the CI never runs

This is the finding that explains why the `h2` advisory sat unnoticed for two
weeks.

`.github/workflows/ci.yml` triggers on `pull_request` against `main` and nothing
else — no `push`, no `schedule`. But the repository has **zero merge commits**
across its entire history and only the `main` branch: work is pushed directly.

Six well-built jobs — Check (matrix), Test (matrix), Clippy, Format, Security
Audit via `rustsec/audit-check`, and `cargo-deny` over advisories/bans/licenses
— therefore never execute in practice.

A `push: branches: [main]` trigger matches how the project is actually
developed. It is not sufficient on its own, though: advisories appear
independently of commits, so the audit job additionally needs a `schedule`
entry (daily is reasonable) to catch a `h2`-class finding on a quiet week.

By contrast, `release.yml` is sound and needs no change: signing is mandatory
and a missing key aborts the run, signatures are verified against the committed
public key before publication, key material is removed after use, permissions
are `contents: read` globally with `contents: write` scoped to the release job
alone, and every action is pinned to a commit SHA.

### ✅ No findings in application code

A dedicated security pass over the source found nothing at the >80% confidence
threshold. The paths that would matter are already strongly controlled:

- **Process spawning** — every `Command` invocation uses argument vectors, never
  shell strings. `Command::new("git")` occurs exactly once, inside the hardened
  `git_command()` helper; no bypass exists anywhere in the tree.
  `src/browser/opener.rs` validates program names against shell metacharacters
  and quotes paths through `shlex`.
- **Paths into PTYs** — `quote_path_for_cd` / `insert_path_at_cursor` in
  `src/app/pty.rs` quote via `shlex::try_quote` and refuse paths that cannot be
  quoted safely.
- **Typst file access** — `resolve_within_base()` in `src/browser/typst_pdf.rs`
  canonicalises and checks `starts_with(canonical_base)`, with a regression test
  against `..` traversal; `typst_str_escape()` blocks string breakout from
  `#image(...)` / `#link(...)`, also tested.
- **Self-update** — Ed25519 signature verification against the embedded key,
  rustls for TLS, release selection by semver comparison rather than list
  position (deliberate hardening against reordering).
- **Repo-local config trust** — SHA-256 pinning against the canonical path,
  fail-closed on a non-canonicalisable path or a corrupt allowlist, no TOCTOU
  (the bytes that were hashed are the bytes that get parsed), trust store at
  mode `0600`.
- **`unsafe`** — 9 blocks, all libc FFI for local time (`localtime_r`,
  `mem::zeroed` on `struct tm`) plus one `geteuid()` check. No
  externally-influenced memory access.
- No embedded credentials, no weak cryptography (SHA-256 only, for config
  pinning), nothing sensitive in the crash log or update log, and no escape
  sequence injection in the iTerm transfer path (filename and payload are
  base64-encoded before entering OSC 1337).

### Correction to `PROJECT_AUDIT_2026-08-30.md`

That report states a third `quick-xml` path runs through `self_update 0.44.0`
and concludes the risk acceptance is incomplete until that path is traced.

That is not correct. The third path is
`syntect` → `plist` → `quick-xml 0.41.0`, and 0.41.0 is already the patched
version for both advisories (`patched = [">= 0.41.0"]`). There was nothing left
to accept on that path.

The ignore rationale did need correcting, but for a different reason — see
below.

## 🔧 Changes Applied During This Audit

1. **`cargo update`** — 126 packages updated; `Cargo.lock` now holds 645
   packages. Verified afterwards:
   - `cargo audit` → 0 vulnerabilities, 5 unmaintained warnings.
   - `cargo build --release` → clean, 2m 56s.
   - `cargo test` → 288 passed, 1 failed, 3 ignored. The single failure is
     `ui::settings::tests::test_paths_item_count`, which was **already failing
     before the update** — no regression was introduced. See below.
   - `cargo clippy --all-targets --all-features` → the same 3 warnings as
     before; no new lint.
   - `cargo fmt -- --check` → clean.

2. **`.cargo/audit.toml`** — the ignore rationale was rewritten. After the
   update only **one** affected `quick-xml` path remains:
   `typst-library` → `hayagriva` → `citationberg` → `quick-xml 0.38.4`, still
   unreachable (ai-workbench generates its own Typst source and never loads a
   user-supplied `.csl`). The formerly listed Wayland path now resolves to the
   patched 0.41.0, because `wayland-scanner` moved from 0.31.10 to 0.31.11 and
   off its `^0.39` pin. The resolved advisories are listed explicitly so they
   are not silently re-added.

3. **`deny.toml`** — the same correction to the ignore comment, plus the
   `[bans] skip` list regenerated from the new lockfile: the entire `0.48.x`
   Windows line dropped out of the tree and `0.53.x` / `0.61.x` came in, so
   eight entries pointed at versions that no longer exist while the new ones
   were uncovered. A note records the alternative — dropping the `version` field
   and skipping those crates wholesale — since the project never builds for
   Windows and this list will drift again on the next update.

Both files were validated as well-formed TOML. `cargo-deny` itself is not
installed locally, so the bans/licenses sections could not be executed.

## 🐛 Functional Findings in the Working Tree

Not security issues, but release-blocking. Both were verified by execution or by
reading the code, not inferred.

### 🔴 HIGH — A test is red

`ui::settings::tests::test_paths_item_count` (`src/ui/settings.rs:1821`) asserts
10 items but finds 11: the Paths category gained a field for the Antigravity
command and the test was not updated. Reproduced directly; 291 other tests pass.
Not a flake.

### 🔴 HIGH — The backend chooser can leave the AI pane without a PTY

Confirmed as described in the first audit. On `Esc`,
`src/app/keyboard/dialogs.rs:309` takes the startup branch and calls
`apply_ai_backend`, which spawns the PTY. The mouse path in
`src/app/mouse.rs:499-503` calls only `close()` — no startup branch, no PTY.

The permission-mode dialog immediately below it handles exactly this case
correctly (it checks `claude_pty_pending` and starts the PTY); the backend
chooser is missing that counterpart. A single shared cancel path for keyboard
and mouse is the fix.

### 🟡 MEDIUM — Visible overlay and keyboard owner can disagree (broader than reported)

The first audit describes this for `agent_startup`. It is more general than
that. The update dialog is drawn at `src/app/drawing.rs:205` but takes keyboard
priority at `src/app/keyboard/mod.rs:43`, ahead of *everything drawn after it*:
menu, dialog, wizard, settings, export chooser, `claude_startup`,
`agent_startup` and `backend_switch`. Only `permission_mode_dialog` carries an
explicit `&& !self.update_state.show_dialog` guard, in both the draw and the
mouse path.

Since the update dialog appears asynchronously after the version check, it can
surface on top of an already-open modal and swallow its keys. One
`has_blocking_modal()` predicate, used by drawing, keyboard and mouse alike,
resolves the whole class rather than one instance of it.

The remaining Medium and Low findings of `PROJECT_AUDIT_2026-08-30.md` — mouse
modality of the startup overlay, startup selections lost on PTY restart,
conflicting configured/form arguments, Claude-specific naming in generalised
state — were not re-examined here and stand as written.

## 📦 Dependency Status

Only rows that call for action. The remaining direct dependencies resolve to
current versions with nothing notable against them.

| Dependency | Locked | Status | Action Needed |
|---|---|---|---|
| `h2` (transitive) | 0.4.19 | ✅ Fixed 30.08.2026 | none — was 0.4.15, RUSTSEC-2026-0258 |
| `lru` (transitive) | 0.18.3 | ✅ Fixed 30.08.2026 | none — was 0.18.1, RUSTSEC-2026-0253 |
| `crossterm` | 0.28.1 | ✅ Deliberately pinned | keep at 0.28.x while the tui-textarea fork targets it |
| `tui-textarea` | git rev `b6bf812d` | ✅ Pinned to an exact rev | keep; `Cargo.lock` must stay committed |
| `self_update` | 0.44.0 | ⚠️ 1.x available | needs Rust 1.88; only in a dedicated MSRV phase |
| `ttf-parser` (dev) | 0.25.1 | ⚠️ Unmaintained | RUSTSEC-2026-0192; evaluate `skrifa` for the font-coverage test |
| `bincode`, `paste`, `rustybuzz`, `yaml-rust` | — | ⚠️ Unmaintained | transitive via syntect/typst, no reachable path — watch only |
| `quick-xml` (transitive) | 0.38.4 + 0.41.0 | ⚠️ One affected path | 0.38.4 via citationberg stays until upstream moves off `^0.38` |

## 💡 Library Recommendations

| Current | Suggested Alternative | Reason |
|---|---|---|
| Manual dependency review | Dependabot or Renovate | Closes exactly the gap this audit found. Must be configured to respect the deliberate `crossterm` pin and the tui-textarea git revision — both tools support that. |
| `cargo-deny` not installed locally | install it | `deny.toml` is maintained but its bans/licenses sections can currently only be exercised in a CI job that does not run. |
| `syntect` with `regex-onig` | `syntect` with `fancy-regex` | Simpler cross-platform builds without native Oniguruma — but only after measuring highlighting and startup time, not on suspicion. |
| `ttf-parser` (dev) | `skrifa` | RustSec names it as the maintained successor. |

## 🛠️ Code Quality Notes

The committed state is in unusually good shape: `cargo fmt --check` clean,
Clippy across all targets and features reports only three cosmetic warnings
(`src/app/file_ops.rs:956` nonminimal_bool, `src/ui/agent_startup.rs:351`
derivable_impls, `src/setup/wizard.rs:449` field_reassign_with_default), no dead
code, no unused imports, and **zero** TODO/FIXME/HACK markers anywhere in
`src/`. 292 tests across 37 files.

Three things stand out for future work:

- **Two error-handling styles without a shared type.** Ten files use
  `anyhow::Result`; `src/clipboard.rs`, `src/update/install.rs`,
  `src/browser/remote_open.rs` and `src/git/mod.rs` return
  `Result<T, String>`. There is no project error enum and no `thiserror`.
- **Silently swallowed writes in directory sync.** `src/app/pty.rs:215, 233,
  469` discard the result of `write_input` when sending `cd` to a pane; a dead
  shell goes unnoticed. The neighbouring branch of the same function does log
  its failure case, so the file is inconsistent with itself. Similar cases at
  `src/app/file_ops.rs:440` and `:739`, where a failed `create_dir_all` surfaces
  later as a confusing "file not found".
- **`src/ui/settings.rs` is 2000 lines**, the largest file in the project, and
  still growing — the eleventh Paths field that just broke a test is a symptom.
  State, field navigation and several category-specific `render_*` functions all
  live in one file; splitting state from rendering is warranted. Seven further
  files exceed 1000 lines (`ui/preview.rs`, `app/mouse.rs`,
  `browser/typst_pdf.rs`, `config.rs`, `types.rs`, `app/file_ops.rs`,
  `ui/help.rs`).
- Module-level `//!` docs cover 46 of 72 files (64%); roughly 56% of public
  items carry a `///`.

## 📊 Overall Health Score: 7.5/10

The committed baseline is exceptionally mature for a TUI tool — signed updates,
a considered configuration trust boundary, hardened git invocation, and a panic
strategy built deliberately around the alternate screen. Points come off not for
the code but for the safeguards failing to engage: the quality gates never run,
and the working tree is not shippable with a red test and a mouse path that
leaves a pane without a terminal.

Top priorities:

1. Commit the updated `Cargo.lock` together with the corrected `.cargo/audit.toml`
   and `deny.toml` rationales.
2. Add `push: branches: [main]` to the CI trigger and a daily `schedule` to the
   audit job — otherwise the next advisory goes unnoticed just as this one did.
3. Before the next release: fix the test at `src/ui/settings.rs:1821`, route the
   backend chooser's mouse cancel through the same path as `Esc`, and unify
   render order with input priority behind one modal predicate.
