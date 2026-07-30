#!/usr/bin/env bash
# Reclaim disk from target/ without a full rebuild.
#
# Removes the incremental cache and stale test binaries; cargo rebuilds what it
# still needs and leaves compiled dependencies alone, so this costs seconds
# rather than a SQLCipher compile.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

before=$(du -sm target 2>/dev/null | cut -f1 || echo 0)

rm -rf target/debug/incremental
find target/debug/deps -maxdepth 1 -type f -size +50M -delete 2>/dev/null || true

after=$(du -sm target 2>/dev/null | cut -f1 || echo 0)
printf 'target/: %s MB -> %s MB (freed %s MB)\n' "$before" "$after" "$((before - after))"
df -h . | tail -1
