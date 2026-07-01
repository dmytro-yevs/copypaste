#!/usr/bin/env bash
# check-version-consistency.sh — CopyPaste-velj release gate.
#
# Asserts the version is consistent across every release surface, so a tagged
# release can never ship a Homebrew cask whose version/sha256 disagree with the
# built binary, nor an undocumented CHANGELOG. Intended to run in CI on a release
# tag (AFTER scripts/release/gen-cask.sh has rewritten the cask), and locally
# before cutting a release.
#
# Sources of truth checked:
#   1. Cargo workspace version   (Cargo.toml  [workspace.package] version)
#   2. Homebrew cask version     (Casks/copypaste.rb  version "X")
#   3. CHANGELOG has a section    (CHANGELOG.md  ## [X])
#
# Usage: scripts/release/check-version-consistency.sh
# Exit 0 = consistent; non-zero + message on any mismatch.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

fail() { echo "❌ version-consistency: $*" >&2; exit 1; }

# 1. workspace version from Cargo.toml (first `version = "X"` under [workspace.package]).
cargo_ver="$(rg -N '^\s*version\s*=\s*"([^"]+)"' Cargo.toml -or '$1' | head -1)"
[ -n "$cargo_ver" ] || fail "could not read workspace version from Cargo.toml"

# 2. cask version.
cask_ver="$(rg -N 'version\s+"([^"]+)"' Casks/copypaste.rb -or '$1' | head -1)"
[ -n "$cask_ver" ] || fail "could not read version from Casks/copypaste.rb"

# 3. CHANGELOG section presence.
changelog_has="$(rg -N "^## \[${cargo_ver}\]" CHANGELOG.md || true)"

echo "workspace=$cargo_ver  cask=$cask_ver"

[ "$cargo_ver" = "$cask_ver" ] || fail "cask version ($cask_ver) != workspace version ($cargo_ver) — run scripts/release/gen-cask.sh"
[ -n "$changelog_has" ] || fail "CHANGELOG.md has no '## [$cargo_ver]' section"

echo "✅ version consistent across Cargo, Cask, CHANGELOG ($cargo_ver)"
