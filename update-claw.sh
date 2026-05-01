#!/usr/bin/env bash
# update-claw.sh — pull upstream changes and rebase local DeepSeek patches on top
#
# Strategy: local customisations live on branch "local/deepseek-patches" which is
# rebased onto origin/main after each upstream pull.  This survives upstream
# updates cleanly and avoids the fragility of git stash + stash pop.
#
# First-time setup (run once after cloning):
#   git checkout -b local/deepseek-patches
#   # … make your changes, then:
#   git add -p && git commit -m "feat: add DeepSeek provider support"
#   # Then run this script any time upstream updates.
#
# Subsequent usage: just run this script.

set -euo pipefail

REPO="$(cd "$(dirname "$0")" && pwd)"
LOCAL_BRANCH="local/deepseek-patches"
UPSTREAM_BRANCH="main"

cd "$REPO"

CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"

if [ "$CURRENT_BRANCH" != "$LOCAL_BRANCH" ]; then
    echo "ERROR: Expected to be on branch '$LOCAL_BRANCH', but on '$CURRENT_BRANCH'." >&2
    echo "       Run: git checkout $LOCAL_BRANCH" >&2
    exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "→ Uncommitted changes detected. Stashing them temporarily..."
    git stash push -m "update-claw: auto-stash before rebase"
    STASHED=1
else
    STASHED=0
fi

echo "→ Fetching upstream..."
git fetch origin

echo "→ Rebasing $LOCAL_BRANCH onto origin/$UPSTREAM_BRANCH..."
if ! git rebase "origin/$UPSTREAM_BRANCH"; then
    echo "" >&2
    echo "ERROR: Rebase conflict. Resolve it manually:" >&2
    echo "  1. Fix conflict markers in the listed files" >&2
    echo "  2. git add <resolved-files>" >&2
    echo "  3. git rebase --continue" >&2
    echo "  4. Re-run this script" >&2
    if [ "$STASHED" -eq 1 ]; then
        echo "  Note: your uncommitted changes are in git stash (run 'git stash pop' after resolving)" >&2
    fi
    exit 1
fi

if [ "$STASHED" -eq 1 ]; then
    echo "→ Restoring stashed changes..."
    git stash pop
fi

echo "→ Rebuilding..."
cargo build --manifest-path "$REPO/rust/Cargo.toml" --workspace

echo ""
echo "✓ Done. Binary at $REPO/rust/target/debug/claw"
echo "  Upstream: $(git log --oneline "origin/$UPSTREAM_BRANCH" -1)"
echo "  Local:    $(git log --oneline HEAD -1)"
