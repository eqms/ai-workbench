# Security Notes

Operational security playbook for ai-workbench. This file tracks the
findings of the project audit and the remediation plan for each.

## Self-Update Supply-Chain Hardening (HIGH — open)

**Finding** (audit 2026-05-11): `src/update/install.rs` uses
`self_update::backends::github::Update` with default configuration. Downloads
are over HTTPS but there is no checksum or signature verification on the
binary archive. A compromised GitHub release asset (compromised account,
account takeover, supply-chain attack on a maintainer) would install an
arbitrary binary on every user's machine on the next auto-update — silently
and with the privileges of the running user.

Tar-slip protection is fully delegated to the `self_update` crate; if a
malicious archive entry contains `../` or absolute paths, defense depends
entirely on whatever guard that crate ships.

### v1.0.1 Audit Re-Confirmation (10.07.2026)

The 10.07.2026 project audit re-confirmed this finding a third time and the
maintainer decided to **defer** the code change in this batch to avoid
bricking auto-update for existing users (enabling `.verifying_keys()` before
signed releases ship would break every in-flight update). No client or
workflow change is made here; the two-halves Remediation Plan below remains
the tracked path, blocked on the operator generating the keypair.

### v1.1.1 Security Audit Cross-Reference (02.07.2026)

The 02.07.2026 audit re-confirmed this finding independently. No code or
workflow changes are made for it in this batch — the CI-signing /
client-verification work is already tracked as **SEC-01 (HIGH)** in
`.planning/ROADMAP.md` / `.planning/STATE.md` Phase 1 backlog: plan
`01-05-PLAN.md` (CI release signing, blocked on the operator running
`cargo install zipsign && zipsign generate-keys` and adding the private key
as a GitHub Actions secret) and plan `01-06-PLAN.md` (client
`.verifying_keys()` wiring, blocked on 2+ signed releases shipping first).
Duplicating this work here would fork the tracking; the unchanged technical
steps remain in the Remediation Plan below.

### Remediation Plan

The fix has two halves, both required:

#### Half 1: Sign release archives in CI

In `.github/workflows/release.yml`, before the `gh release upload` step:

1. Generate an ed25519 signing keypair **once**, on a developer workstation:
   ```bash
   # Use zipsign (matches self_update's `signatures` feature) or minisign.
   cargo install zipsign
   zipsign generate-keys --pubkey ai-workbench-pub.bin --privkey ai-workbench-priv.bin
   ```
2. Store `ai-workbench-priv.bin` as a base64-encoded GitHub Actions
   secret (e.g. `ZIPSIGN_PRIVATE_KEY`). Never commit the private key.
3. Commit `ai-workbench-pub.bin` to the repository at
   `signing/ai-workbench-pub.bin`.
4. In the release workflow, after building the platform archives:
   ```yaml
   - name: Sign archives
     env:
       ZIPSIGN_KEY_B64: ${{ secrets.ZIPSIGN_PRIVATE_KEY }}
     run: |
       echo "$ZIPSIGN_KEY_B64" | base64 -d > /tmp/key.bin
       for f in dist/ai-workbench-*.tar.gz dist/ai-workbench-*.zip; do
         zipsign sign tar /tmp/key.bin "$f"
       done
       rm -f /tmp/key.bin
   ```
5. Upload both the archive **and** the corresponding `.sig` sidecar file to
   the release.

#### Half 2: Verify signatures in the client

The `signatures` feature is already enabled on `self_update` in `Cargo.toml`.
What remains:

1. Embed the public key at compile time in `src/update/install.rs`:
   ```rust
   const RELEASE_VERIFYING_KEY: &[u8] =
       include_bytes!("../../signing/ai-workbench-pub.bin");
   ```
2. Configure the updater to require a verified signature:
   ```rust
   Update::configure()
       .repo_owner(REPO_OWNER)
       .repo_name(REPO_NAME)
       .bin_name(BIN_NAME)
       .target(target)
       .current_version(CURRENT_VERSION)
       .verifying_keys([RELEASE_VERIFYING_KEY])  // <-- new
       .show_download_progress(false)
       .show_output(false)
       .no_confirm(true)
       .build()
   ```
3. Bump the **major** binary version when this lands. Older binaries that
   self-update will still receive the new signed archive successfully (they
   simply won't verify the signature). Newer binaries will refuse any
   unsigned or wrongly-signed archive.

#### Rollout Order

The two halves must land **in this order**, never reversed:

1. Sign the next 2-3 releases first. Verification is _not_ enabled in the
   client, so older clients keep updating normally.
2. Once releases reliably ship `.sig` sidecars, ship a client release that
   enables `verifying_keys`. Document in RELEASE_NOTES that older signed
   archives are required from this version on.

If verification is enabled in the client before the release workflow signs,
**every existing user's auto-update will break** until they manually replace
the binary.

### Effort Estimate

- Half 1 (CI workflow): ~30 minutes once a keypair exists.
- Half 2 (client code): ~10 minutes plus regression test.
- Keypair generation + secret rotation policy: ~1 hour planning + setup.

---

## Browser/Editor Command Construction (MEDIUM — open)

**Finding**: `src/browser/opener.rs:83-106` (`open_file_with_browser`,
`open_file_with_editor`) reads `config.ui.browser` / equivalent as a free
string, splits on whitespace via a hand-rolled `split_command()`, and uses
the first token as the program. No allow-list, no path validation. Currently
low-risk because config is user-owned, but if any future code path derives
the field from PTY output, a URL, or a remote source, it becomes command
injection on every file-open.

**Mitigation** (cheap, do it):

- After splitting, validate that the first token resolves to either an
  absolute path or a basename in `$PATH` whose name matches
  `^[A-Za-z0-9_./-]+$`. Reject anything else.
- Document explicitly in the config schema that `browser` and `editor` must
  not be sourced from untrusted input.

**Update (02.07.2026):** a related but distinct vector was found and closed
in the same audit — `open_file()`/`open_in_file_manager()` in
`src/browser/opener.rs` passed the target path to `open`/`xdg-open` without
canonicalizing it, so a filename starting with `-` could be misread as a
flag; fixed via the new `resolve_for_arg()` helper (see Closed Findings
below). This does not close the broader item above — the
`validate_program()` allow-list for the configured `browser`/`editor`
strings already existed before this audit and remains the mitigation for
that separate vector.

---

## Shell Fallback in Dependency Probe (MEDIUM — open)

**Finding**: `src/setup/dependency_checker.rs:172-186` builds a shell command
string with `shlex::try_quote` (good, just migrated from `shell-escape`)
then passes it to `$SHELL -i -c "<cmd>"`. Static call sites today are safe.
The pattern is fragile — one careless caller passing PTY-derived text via
`args` and a `shlex` bug becomes shell injection.

**Mitigation**:

- Replace the `-i -c` shell string with a direct `Command::new(name).args(args)`
  invocation. The only reason to go through a shell is to resolve aliases or
  shell functions; for binary lookups (which is the entire purpose of this
  module), that is unnecessary.
- If shell-resolved binaries are required, gate the call behind an explicit
  allow-list of known dependency names.

---

## Predictable Temp File Path (MEDIUM — open)

**Finding**: `src/browser/pdf_export.rs:119` writes preview HTML to
`$TMPDIR/<stem>-<dd.mm.yyyy>.html`. The path is guessable. On a multi-user
system, a local attacker can pre-create the path as a symlink to a target
file and the write redirects to it.

**Mitigation**: Use `tempfile::Builder::new().prefix(stem).suffix(".html")
.tempfile_in(env::temp_dir())?`. The crate opens with `O_EXCL` and an
unpredictable suffix.

---

## Windows Config File Permissions (LOW — accepted)

**Finding**: `src/config.rs:763-767`'s `set_restrictive_permissions()` is a
`#[cfg(not(unix))]` no-op on Windows — config file permissions are left at
the OS default rather than restricted to the owner.

**Disposition**: accepted. The current config schema holds no secrets, so
there is nothing sensitive to protect on Windows today. Must be revisited
the moment a secret-bearing field is added to `Config`.

---

## Claude Pane Paste Without Bracketing (LOW — accepted, by design)

**Finding**: `src/app/keyboard/mod.rs:165-173`'s `handle_paste_event()` sends
raw paste bytes to the Claude pane without the bracketed-paste wrapping
(`\x1b[200~...\x1b[201~`) used for the LazyGit/Terminal panes.

**Disposition**: accepted, by design. The Claude CLI does not understand
bracketed-paste sequences — wrapping would corrupt input rather than
protect it. LazyGit and the User Terminal keep bracketed-paste wrapping.

---

## Closed Findings

- **Typst PDF-export injection + path traversal** (audit 10.07.2026, v1.0.1):
  `src/browser/typst_pdf.rs` interpolated untrusted markdown link/image URLs
  into Typst string literals without escaping `"`, and fenced code content
  into fixed ` ``` ` fences — letting a crafted `.md` break out of the string
  and inject arbitrary Typst code, which via the custom `World::file()` (that
  joined paths without `..` sanitization) could read arbitrary local files
  into the exported PDF. Closed by: a `typst_str_escape()` helper (escapes
  `\`/`"`, drops newlines) applied to all `#link(...)`/`#image(...)` URLs; a
  `longest_backtick_run()`-based dynamic code fence; a language-tag whitelist;
  and a `resolve_within_base()` canonicalize + `starts_with` traversal guard
  in `World::file()` (mirrors the guard already in `markdown.rs`). Six
  regression tests added (T-typst-inj).
- **Markdown preview HTML injection** (audit 02.07.2026, v1.1.2): raw
  `Event::Html`/`Event::InlineHtml` from pulldown-cmark neutralized to
  escaped `Event::Text` in `src/browser/markdown.rs`, closing the
  raw-HTML/script execution vector in the `file://` preview while leaving
  `Tag::Image`/`Tag::Link`-rendered `<img>`/`<a>` tags (and
  `fix_image_paths()`/`fix_md_links()`) unaffected.
- **GitHub Actions supply-chain hardening** (audit 02.07.2026, v1.1.2): both
  workflow files now declare top-level `permissions: contents: read` and
  every action reference is pinned to a full commit SHA with a trailing
  tag-name comment.
- **shell-escape unmaintained** (audit 2026-05-11): replaced with `shlex` 1.x
  in `src/app/pty.rs` and `src/setup/dependency_checker.rs`. Tracked in
  commit history for v0.89 release.
