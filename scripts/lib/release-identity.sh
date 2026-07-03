#!/usr/bin/env bash
# release-identity.sh — single source of truth for build/release identifiers
# that were previously hand-copied across shell scripts, CI, and the
# Homebrew cask (CopyPaste-8ebg.60).
#
# Usage: `source` this file (do NOT execute it directly), then use:
#   $REPO           — GitHub "owner/name" slug.
#   $DAEMON_LABEL   — launchd label / macOS Keychain service name for the daemon.
#   bundle_id_for X — echoes the codesign bundle identifier for binary X.
#
# Not plumbed everywhere: the Rust call sites below keep their own local
# consts because sourcing a shell file into a Cargo build isn't a low-risk
# change (would need a build.rs / include! indirection touching signing-
# adjacent code). They are documented here so the two stay in sync by hand:
#   - crates/copypaste-daemon/src/keychain/mod.rs            ::SERVICE
#   - crates/copypaste-cli/src/commands/daemon/platform.rs   ::LAUNCHD_LABEL
#   - crates/copypaste-ui/src-tauri/src/daemon_lifecycle.rs  ::LAUNCHD_LABEL
# packaging/macos/com.copypaste.daemon.plist and Casks/copypaste.rb also stay
# hand-kept in sync (plist XML and the Homebrew cask's Ruby DSL aren't
# shell-sourceable either).

# GitHub repo slug (owner/name). Overridable via COPYPASTE_REPO for forks.
REPO="${COPYPASTE_REPO:-dmytro-yevs/copypaste}"

# launchd label / macOS Keychain service name for the daemon.
DAEMON_LABEL="com.copypaste.daemon"

# Per-binary macOS codesign bundle identifier. macOS ships bash 3.2 (no
# associative arrays), so this is a function + case statement rather than a
# `declare -A` map — mirrors the pattern already used at each call site.
bundle_id_for() {
    case "$1" in
        copypaste-daemon) echo "com.copypaste.daemon" ;;
        copypaste)        echo "com.copypaste.cli" ;;
        copypaste-relay)  echo "com.copypaste.relay" ;;
        *)                echo "com.copypaste.$1" ;;
    esac
}
