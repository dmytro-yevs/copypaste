#!/usr/bin/env bash
# check-macos-types.sh — type-check the macOS-only code, on a Linux host.
#
#   Usage: scripts/check-macos-types.sh [--self-test]
#
# `#[cfg(target_os = "macos")]` code is compiled by nothing on this project's
# development host and by nothing in CI until a `macos-14` runner picks it up.
# That gap cost two rounds and a Mac runner to surface one missing trait bound
# in a test module, which every other check in this repository passed over.
#
# Two mechanisms, because the crates differ:
#
#   * `copypaste-ipc` has no C in its dependency graph, so its macOS branches
#     are checked directly with `--target aarch64-apple-darwin`.
#   * `copypaste-core` and `copypaste-daemon` cannot be: SQLCipher and ring
#     compile C, and cross-compiling C for Darwin needs an SDK this host has
#     none of (`cc: error: unrecognized command-line option '-arch'`). So the
#     two macOS files are `#[path]`-included into a generated crate that
#     depends only on objc2 and security-framework, neither of which builds any
#     C. The include points at the files in this repository — never a copy,
#     which would pass while the tree rotted.
#
# ---------------------------------------------------------------------------
# The drift cost, stated
# ---------------------------------------------------------------------------
# The generated crate mirrors the surface those two files reach for outside
# themselves: `crypto::{CryptoError, KEY_LEN}`, `crypto::keys::random_secret`,
# and the slice of `AppState` that `notify.rs` reads. If `keystore/mod.rs`
# starts using something new from `super`, or `AppState` changes shape, this
# check breaks — loudly, here, in seconds. That is the trade: a stub that can
# go stale, against macOS-only code that nothing type-checks at all.
#
# It is not a substitute for the `macos-check` job. It proves the code compiles
# for Darwin; only a Mac proves it does anything.
#
# --self-test additionally perturbs the *generated* crate — never the tree —
# and asserts the check fails, and fails naming the real file. A check that
# cannot fail is worse than no check.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || exit 1

TARGET="aarch64-apple-darwin"
# Kept under target/ so a rerun is incremental and nothing lands in the tree.
WORK="$REPO_ROOT/target/macos-typecheck"
HARNESS="$WORK/harness"

# The four crates the generated crate pins. Each must satisfy what
# [workspace.dependencies] asks for, or the harness is checking code the
# workspace does not build.
PIN_OBJC2="0.5.2"
PIN_OBJC2_FOUNDATION="0.2.2"
PIN_OBJC2_APP_KIT="0.2.2"
PIN_SECURITY_FRAMEWORK="3.7.0"

PASS=0
FAIL=0
ok()  { PASS=$((PASS + 1)); printf '  ok    %s\n' "$1"; }
bad() {
    FAIL=$((FAIL + 1))
    printf '  FAIL  %s\n' "$1"
    if [[ -n "${2:-}" ]]; then printf '%s\n' "$2" | sed 's/^/        /'; fi
}
group() { printf '\n== %s\n' "$1"; }

replace_in_place() {
    local expression="$1" file="$2" replacement="$2.tmp"
    sed "$expression" "$file" > "$replacement" && mv "$replacement" "$file"
}

CARGO="cargo"
TOOLCHAIN=""
if cargo +1.96 --version >/dev/null 2>&1; then
    CARGO="cargo +1.96"
    TOOLCHAIN="--toolchain 1.96"
fi

# ---------------------------------------------------------------------------
group "Preflight"
# ---------------------------------------------------------------------------
if ! command -v cargo >/dev/null 2>&1; then
    bad "cargo is on PATH"
    exit 1
fi
ok "using $CARGO"

# TOOLCHAIN is a flag pair or empty, so it is deliberately unquoted.
# shellcheck disable=SC2086
if rustup target list --installed ${TOOLCHAIN} 2>/dev/null | grep -qx "$TARGET"; then
    ok "$TARGET std is installed"
elif rustup target add ${TOOLCHAIN} "$TARGET" >/dev/null 2>&1; then
    ok "$TARGET std installed"
else
    bad "$TARGET std is available" \
        "rustup target add ${TOOLCHAIN} ${TARGET} failed; this check needs the Darwin standard library"
    exit 1
fi

# ---------------------------------------------------------------------------
group "The harness pins what the workspace builds"
# ---------------------------------------------------------------------------
# Read the requirement out of [workspace.dependencies] rather than Cargo.lock:
# the lock holds two objc2 majors, because Tauri brings its own.
workspace_req() {
    awk -v name="$1" '
        $1 == name && $2 == "=" {
            if (match($0, /version = "[^"]+"/)) { print substr($0, RSTART + 11, RLENGTH - 12); exit }
            if (match($0, /= "[^"]+"/))         { print substr($0, RSTART + 3,  RLENGTH - 4);  exit }
        }' Cargo.toml
}

pin_matches() {
    local name="$1" pin="$2" req
    req="$(workspace_req "$name")"
    if [[ -z "$req" ]]; then
        bad "$name is in [workspace.dependencies]" "not found in Cargo.toml"
        return
    fi
    case "$pin" in
        "$req"|"$req".*) ok "$name pinned at $pin, workspace asks for $req" ;;
        *) bad "$name pin agrees with the workspace" \
               "this script pins $pin, Cargo.toml asks for $req — update the PIN_ above" ;;
    esac
}
pin_matches objc2            "$PIN_OBJC2"
pin_matches objc2-foundation "$PIN_OBJC2_FOUNDATION"
pin_matches objc2-app-kit    "$PIN_OBJC2_APP_KIT"
pin_matches security-framework "$PIN_SECURITY_FRAMEWORK"

# ---------------------------------------------------------------------------
# Generate the crate. Quoted heredocs, with @REPO@ substituted afterwards, so
# nothing in the Rust below is interpreted by the shell.
# ---------------------------------------------------------------------------
write_harness() {
    local dir="$1"
    rm -rf "$dir"
    mkdir -p "$dir/src" "$dir/copypaste-core-stub/src"

    cat > "$dir/Cargo.toml" <<EOF
[package]
name = "macos-typecheck"
version = "0.0.0"
edition = "2021"

# Its own workspace: it lives under target/ and must not be read as a member.
[workspace]

[dependencies]
copypaste-ipc = { path = "$REPO_ROOT/crates/copypaste-ipc" }
copypaste-core = { path = "copypaste-core-stub" }
objc2 = "=$PIN_OBJC2"
objc2-foundation = { version = "=$PIN_OBJC2_FOUNDATION", features = ["NSString", "NSData", "NSArray"] }
objc2-app-kit = { version = "=$PIN_OBJC2_APP_KIT", features = ["NSPasteboard", "NSPasteboardItem", "NSWorkspace", "NSRunningApplication"] }
security-framework = "=$PIN_SECURITY_FRAMEWORK"
zeroize = { version = "1.7", features = ["derive"] }
thiserror = "1"
anyhow = "1"
tracing = "0.1"
rustix = { version = "1", features = ["fs"] }
uuid = { version = "1", features = ["v4"] }
url = "2"
plist = "1.10"
mime_guess = "2"

[dev-dependencies]
tempfile = "3"

[features]
# Named so the cfg in keystore/mod.rs is not an unexpected one. Never enabled.
android-keystore-typecheck = []
EOF

    cat > "$dir/src/lib.rs" <<'EOF'
// Generated by scripts/check-macos-types.sh. Do not edit; edit the script.
//
// The modules below are the real files, included by path. Everything else in
// this crate is the smallest stub that reproduces what they reach for.
//
// `dead_code` is allowed because nothing here calls into them — the price is
// that an unused item in a macOS file is reported by the macos-14 job rather
// than by this one. `unused_imports` still fires.
#![allow(dead_code)]

#[path = "@REPO@/crates/copypaste-daemon/src/clipboard/mod.rs"]
pub mod clipboard;

const _: fn(&str) -> bool = clipboard::is_password_manager_app;

#[path = "@REPO@/crates/copypaste-daemon/src/notify.rs"]
pub mod notify;

pub mod crypto;

mod daemon_state;
pub use daemon_state::AppState;
EOF

    cat > "$dir/copypaste-core-stub/Cargo.toml" <<'EOF'
[package]
name = "copypaste-core"
version = "0.0.0"
edition = "2021"
EOF

    cat > "$dir/copypaste-core-stub/src/lib.rs" <<'EOF'
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    pub filename: String,
    pub mime_type: String,
}

impl FileMetadata {
    pub fn new(filename: impl Into<String>, mime_type: impl Into<String>) -> Option<Self> {
        let filename = filename.into();
        let mime_type = mime_type.into();
        (!filename.is_empty() && !mime_type.is_empty()).then_some(Self {
            filename,
            mime_type,
        })
    }

    pub fn is_valid(&self) -> bool {
        Self::new(self.filename.clone(), self.mime_type.clone()).is_some()
    }
}

pub fn binary_item_id(_bytes: &[u8]) -> String {
    "00000000-0000-0000-0000-000000000000".to_owned()
}

#[path = "@REPO@/crates/copypaste-core/src/sensitive/origin.rs"]
pub mod sensitive;
EOF

    cat > "$dir/src/crypto.rs" <<'EOF'
// The slice of `copypaste_core::crypto` that the keystore reaches for.
pub const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("keystore unavailable: {0}")]
    KeystoreUnavailable(&'static str),
    #[error("keystore entry unusable: {0}")]
    KeystoreEntryUnusable(&'static str),
}

pub mod keys {
    use super::KEY_LEN;

    pub(super) fn random_secret() -> [u8; KEY_LEN] {
        [0u8; KEY_LEN]
    }
}

#[path = "@REPO@/crates/copypaste-core/src/crypto/keystore/mod.rs"]
pub mod keystore;
EOF

    cat > "$dir/src/daemon_state.rs" <<'EOF'
// The slice of `copypaste_daemon::AppState` that `notify.rs` reads.
use std::sync::{RwLock, RwLockReadGuard};

use copypaste_ipc::ConfigData;

pub struct AppState {
    pub settings: Settings,
}

impl AppState {
    pub fn backend_name(&self) -> &'static str {
        "fake-memory"
    }
}

pub struct Settings {
    current: RwLock<ConfigData>,
}

impl Settings {
    pub fn get(&self) -> RwLockReadGuard<'_, ConfigData> {
        self.current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
EOF

    replace_in_place "s|@REPO@|$REPO_ROOT|g" "$dir/src/lib.rs"
    replace_in_place "s|@REPO@|$REPO_ROOT|g" "$dir/src/crypto.rs"
}

# Prints the compiler output; returns cargo's status.
run_harness() {
    local dir="$1"
    ( cd "$dir" && CARGO_TARGET_DIR="$WORK/target" \
        $CARGO clippy --target "$TARGET" --all-targets -- -D warnings 2>&1 )
}

# ---------------------------------------------------------------------------
group "copypaste-ipc, checked directly (no stub)"
# ---------------------------------------------------------------------------
# Pure Rust all the way down, so the real crate compiles for Darwin as it is.
# This verifies the shared IPC crate for Darwin.
if out="$($CARGO check -p copypaste-ipc --all-targets --target "$TARGET" --locked 2>&1)"; then
    ok "copypaste-ipc compiles for $TARGET"
else
    bad "copypaste-ipc compiles for $TARGET" "$out"
fi

# ---------------------------------------------------------------------------
group "clipboard/macos.rs, keystore/macos.rs and notify.rs"
# ---------------------------------------------------------------------------
write_harness "$HARNESS"
if out="$(run_harness "$HARNESS")"; then
    ok "the macOS-only files compile for $TARGET, tests included, clippy-clean"
else
    bad "the macOS-only files compile for $TARGET, tests included, clippy-clean" "$out"
fi

# ---------------------------------------------------------------------------
if [[ "${1:-}" == "--self-test" ]]; then
    group "self-test: the check can still fail"

    # 1. Perturb the stub so a *real* file stops compiling. This is the one
    #    that proves the include reaches the repository: the error has to be
    #    reported against the path in crates/, not against the generated crate.
    probe="$WORK/selftest-stub"
    write_harness "$probe"
    replace_in_place \
        's/KeystoreEntryUnusable(/KeystoreEntryUnusableZZZ(/' \
        "$probe/src/crypto.rs"
    out="$(run_harness "$probe")"
    status=$?
    if [[ "$status" -eq 0 ]]; then
        bad "a broken stub fails the check" "it passed"
    elif printf '%s' "$out" | grep -q "crates/copypaste-core/src/crypto/keystore/macos.rs"; then
        ok "a broken stub fails, naming the real file in crates/"
    else
        bad "a broken stub fails, naming the real file in crates/" "$out"
    fi

    # 2. Prove the daemon's clipboard module is genuinely in the compilation and
    #    carries its real types — a `#[path]` that resolved to nothing would
    #    fail differently, so the assertion is on the mismatch, not on failure.
    probe="$WORK/selftest-clipboard"
    write_harness "$probe"
    printf '\nmod selftest {\n    fn probe(c: crate::clipboard::Capture) -> u8 { c.content }\n}\n' \
        >> "$probe/src/lib.rs"
    out="$(run_harness "$probe")"
    status=$?
    if [[ "$status" -eq 0 ]]; then
        bad "clipboard::Capture is the real type" "the probe compiled"
    elif printf '%s' "$out" | grep -q 'expected `u8`, found `String`'; then
        ok "clipboard::Capture is the real type, with its real fields"
    else
        bad "clipboard::Capture is the real type, with its real fields" "$out"
    fi

    rm -rf "$WORK/selftest-stub" "$WORK/selftest-clipboard"
fi

# ---------------------------------------------------------------------------
printf '\n%s\n' "-----------------------------------------------"
printf 'passed %d, failed %d\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
