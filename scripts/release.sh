#!/usr/bin/env bash
#
# release.sh — cut a new AI Workbench release.
#
# Bumps the version in Cargo.toml + Cargo.lock, drafts a RELEASE_NOTES.md
# section from the commit log (grouped by [ADD]/[CHG]/[FIX] prefixes), lets you
# refine the notes in $EDITOR, then commits, tags and pushes to BOTH remotes
# (origin = GitLab, upstream = GitHub). The pushed v* tag triggers the GitHub
# Actions release workflow, which builds the 6-platform binaries, creates the
# GitHub Release (body taken from the RELEASE_NOTES.md section this script
# writes) and updates the Homebrew tap.
#
# Usage:
#   scripts/release.sh <patch|minor|major|X.Y.Z> [--dry-run] [--no-push]
#
#   --dry-run   Show the computed version and drafted notes; change nothing.
#   --no-push   Commit and tag locally, but do not push (inspect first).
#
# Note: the claude-workbench Claude Code skill tracks the active version in its
# frontmatter — update that separately (it lives outside this repo).

set -euo pipefail

DRY_RUN=false
NO_PUSH=false
BUMP=""

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=true ;;
    --no-push) NO_PUSH=true ;;
    patch | minor | major) BUMP="$arg" ;;
    [0-9]*.[0-9]*.[0-9]*) BUMP="$arg" ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 1
      ;;
  esac
done

if [ -z "$BUMP" ]; then
  echo "Usage: scripts/release.sh <patch|minor|major|X.Y.Z> [--dry-run] [--no-push]" >&2
  exit 1
fi

cd "$(git rev-parse --show-toplevel)"

# ── Current + next version ────────────────────────────────────────────────
CURRENT=$(awk -F'"' '/^\[package\]/{p=1} p && /^version *=/{print $2; exit}' Cargo.toml)
if [ -z "$CURRENT" ]; then
  echo "ERROR: could not read [package] version from Cargo.toml" >&2
  exit 1
fi

IFS='.' read -r MA MI PA <<<"$CURRENT"
case "$BUMP" in
  patch) NEW="$MA.$MI.$((PA + 1))" ;;
  minor) NEW="$MA.$((MI + 1)).0" ;;
  major) NEW="$((MA + 1)).0.0" ;;
  *) NEW="$BUMP" ;;
esac

TODAY=$(date +%d.%m.%Y)
echo "Current version: $CURRENT"
echo "New version:     $NEW ($TODAY)"

# ── Draft release notes from the commit log since the last tag ─────────────
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
RANGE=""
[ -n "$LAST_TAG" ] && RANGE="${LAST_TAG}..HEAD"
echo "Commit range:    ${RANGE:-<all history>}"

collect() { git log $RANGE --pretty=format:'%s' --no-merges 2>/dev/null | grep -E "^\[$1\]" | sed -E "s/^\[$1\] */- /" || true; }
ADDED=$(collect ADD)
CHANGED=$(collect CHG)
FIXED=$(collect FIX)

build_section() {
  printf '## Version %s (%s)\n\n' "$NEW" "$TODAY"
  if [ -n "$ADDED" ]; then printf '### Added\n\n%s\n\n' "$ADDED"; fi
  if [ -n "$CHANGED" ]; then printf '### Changed\n\n%s\n\n' "$CHANGED"; fi
  if [ -n "$FIXED" ]; then printf '### Fixed\n\n%s\n\n' "$FIXED"; fi
  if [ -z "$ADDED$CHANGED$FIXED" ]; then printf '### Changed\n\n- _TODO: describe this release_\n\n'; fi
}
SECTION=$(build_section)

if $DRY_RUN; then
  echo
  echo "----- DRY RUN: RELEASE_NOTES.md section that would be inserted -----"
  echo "$SECTION"
  echo "----- No files changed. -----"
  exit 0
fi

# ── Apply Cargo.toml version (package section only) ────────────────────────
awk -v new="$NEW" '
  /^\[package\]/ { p = 1 }
  p && /^version *=/ { sub(/"[^"]*"/, "\"" new "\""); p = 0 }
  { print }
' Cargo.toml >Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

# ── Apply Cargo.lock version (the ai-workbench package entry only) ──────────
awk -v new="$NEW" '
  /^name = "ai-workbench"$/ { hit = 1 }
  hit && /^version = / { sub(/"[^"]*"/, "\"" new "\""); hit = 0 }
  { print }
' Cargo.lock >Cargo.lock.tmp && mv Cargo.lock.tmp Cargo.lock

# ── Prepend the drafted section right after the "# Release Notes" title ─────
awk -v section="$SECTION" '
  NR == 1 { print; print ""; print section; next }
  NR == 2 && $0 == "" { next }   # collapse the original blank after the title
  { print }
' RELEASE_NOTES.md >RELEASE_NOTES.md.tmp && mv RELEASE_NOTES.md.tmp RELEASE_NOTES.md

echo
echo "Bumped Cargo.toml, Cargo.lock and drafted RELEASE_NOTES.md for v$NEW."

# ── Let the operator refine the notes ──────────────────────────────────────
if [ -n "${EDITOR:-}" ] && [ -t 0 ]; then
  echo "Opening RELEASE_NOTES.md in \$EDITOR to finalize the notes…"
  "$EDITOR" RELEASE_NOTES.md
else
  echo "Review/edit RELEASE_NOTES.md now (\$EDITOR not set or non-interactive)."
fi

# ── Confirm, commit, tag, push ─────────────────────────────────────────────
git --no-pager diff --stat Cargo.toml Cargo.lock RELEASE_NOTES.md || true
echo
printf 'Commit, tag v%s and push to both remotes? [y/N] ' "$NEW"
read -r REPLY </dev/tty || REPLY="n"
case "$REPLY" in
  y | Y) ;;
  *)
    echo "Aborted. Version files are bumped but nothing was committed."
    exit 0
    ;;
esac

git add Cargo.toml Cargo.lock RELEASE_NOTES.md
git commit -m "[CHG] Release v$NEW"
git tag "v$NEW"

if $NO_PUSH; then
  echo "Committed and tagged v$NEW locally (--no-push). Push manually:"
  echo "  git push origin main && git push upstream main"
  echo "  git push origin v$NEW && git push upstream v$NEW"
  exit 0
fi

BRANCH=$(git rev-parse --abbrev-ref HEAD)
git push origin "$BRANCH"
git push upstream "$BRANCH"
git push origin "v$NEW"
git push upstream "v$NEW"

echo "Released v$NEW. GitHub Actions will build binaries and publish the release."
