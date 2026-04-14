#!/usr/bin/env bash
set -e

REPO="/Users/davidandrews/PycharmProjects/claw-code"

echo "→ Stashing local changes..."
git -C "$REPO" stash

echo "→ Pulling latest..."
git -C "$REPO" pull

echo "→ Rebuilding..."
cargo build --manifest-path "$REPO/rust/Cargo.toml" --workspace

echo "→ Restoring stash..."
git -C "$REPO" stash pop || echo "(nothing to restore)"

echo "✓ Done. Binary at $REPO/rust/target/debug/claw"
