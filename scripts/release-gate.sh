#!/usr/bin/env bash
# =============================================================================
# release-gate.sh — MANUAL-verification release gate (NOT an automated test).
# =============================================================================
#
# THIS SCRIPT NEVER DECLARES A BUILD "VERIFIED" OR "RELEASE-READY" BY ITSELF.
# THE LAST WORD ON A RELEASE BELONGS TO A HUMAN, ON REAL HARDWARE — NOT TO CI,
# NOT TO THIS SCRIPT, NOT TO ANY AGENT.
#
# What this script DOES:
#   1. Builds the REAL signed macOS artifact (.dmg) by calling the existing
#      release scripts — `cargo build --release`, the Tauri UI bundle, and
#      `scripts/release/build-dmg-ci.sh <version>` (ad-hoc codesign + hardened
#      runtime + entitlements + .sha256). If any required tool or input is
#      missing it FAILS LOUDLY, naming what is missing. It does NOT fake,
#      stub, or skip the build to look green.
#   2. Prints a NUMBERED checklist of MANUAL acceptance checks that a human
#      MUST perform on real devices before the release ships.
#
# What this script DELIBERATELY DOES NOT DO:
#   - It does NOT install the app, sync between devices, test Android on a
#     phone, open Settings, or probe the Keychain. Those are HUMAN steps.
#   - It does NOT claim any of those checks passed. It cannot observe them.
#
# EXIT CONTRACT (the gate):
#   - By default this script EXITS NON-ZERO after a successful build, because
#     the manual checklist has NOT been completed. A non-zero exit means
#     "build produced; human verification still owed."
#   - It exits 0 ONLY when a human explicitly asserts they completed the
#     checklist on real hardware, via EITHER:
#         --i-verified-on-hardware            (CLI flag), OR
#         RELEASE_GATE_HUMAN_CONFIRMED=1       (environment variable)
#     Passing this flag is a HUMAN ATTESTATION, not an automated result. Do
#     not wire it into CI to auto-pass releases — that defeats the gate.
#
# USAGE:
#   scripts/release-gate.sh [<version>] [--i-verified-on-hardware]
#
#   <version>   Version string for the artifact (e.g. 0.5.1). Defaults to the
#               workspace version parsed from the root Cargo.toml.
#
# EXAMPLES:
#   scripts/release-gate.sh 0.5.1
#       → builds the signed DMG, prints the checklist, exits 1 (verification owed).
#
#   scripts/release-gate.sh 0.5.1 --i-verified-on-hardware
#   RELEASE_GATE_HUMAN_CONFIRMED=1 scripts/release-gate.sh 0.5.1
#       → builds the signed DMG, prints the checklist, and (because a human has
#         attested) exits 0.
#
# See docs/release/RELEASE-CHECKLIST.md for the full release runbook. A
# dedicated docs/RELEASE-ACCEPTANCE.md may be added later; until then the
# acceptance checklist is inlined below and is the source of truth.
# =============================================================================
set -euo pipefail

# --- Argument parsing --------------------------------------------------------
HUMAN_CONFIRMED=0
if [[ "${RELEASE_GATE_HUMAN_CONFIRMED:-0}" == "1" ]]; then
    HUMAN_CONFIRMED=1
fi

VERSION=""
for arg in "$@"; do
    case "$arg" in
        --i-verified-on-hardware)
            HUMAN_CONFIRMED=1
            ;;
        -h|--help)
            sed -n '2,60p' "$0"
            exit 0
            ;;
        --*)
            echo "ERROR: unknown flag: $arg" >&2
            echo "Usage: $0 [<version>] [--i-verified-on-hardware]" >&2
            exit 2
            ;;
        *)
            if [[ -n "$VERSION" ]]; then
                echo "ERROR: version already set to '$VERSION'; unexpected extra arg '$arg'" >&2
                exit 2
            fi
            VERSION="$arg"
            ;;
    esac
done

# --- Locate repo root --------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# --- Resolve version from Cargo.toml if not supplied -------------------------
if [[ -z "$VERSION" ]]; then
    VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
    if [[ -z "$VERSION" ]]; then
        echo "ERROR: could not determine version. Pass it explicitly:" >&2
        echo "       $0 <version> [--i-verified-on-hardware]" >&2
        exit 1
    fi
    echo "==> No version arg; using workspace version from Cargo.toml: $VERSION"
fi

# --- Platform + tooling preconditions (fail loudly, name what's missing) -----
if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: release-gate.sh builds the macOS signed DMG and must run on macOS." >&2
    echo "       Current platform: $(uname -s). Run this on a real Mac." >&2
    exit 1
fi

require_tool() {
    local tool="$1"
    local hint="$2"
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "ERROR: required tool '$tool' not found on PATH." >&2
        echo "       $hint" >&2
        exit 1
    fi
}

echo "==> Checking required tooling"
require_tool cargo     "Install the Rust toolchain (https://rustup.rs)."
require_tool pnpm      "Install pnpm (https://pnpm.io) — needed for the Tauri UI build."
require_tool codesign  "Part of Xcode command line tools: xcode-select --install"
require_tool hdiutil   "Provided by macOS; if missing your install is broken."
require_tool shasum    "Provided by macOS; if missing your install is broken."

BUILD_DMG_SCRIPT="scripts/release/build-dmg-ci.sh"
if [[ ! -x "$BUILD_DMG_SCRIPT" && ! -f "$BUILD_DMG_SCRIPT" ]]; then
    echo "ERROR: real release script missing: $BUILD_DMG_SCRIPT" >&2
    echo "       Cannot build the signed artifact without it. Aborting." >&2
    exit 1
fi

# =============================================================================
# STEP 1 — Build the REAL signed artifact via the existing release scripts.
# =============================================================================
# build-dmg-ci.sh requires (per its own header):
#   - cargo --release build of copypaste-cli / -daemon / -relay
#   - the Tauri .app bundle (cd crates/copypaste-ui && pnpm install && pnpm tauri build)
# We run those prerequisites here, then hand off to build-dmg-ci.sh which does
# the ad-hoc codesign (hardened runtime + entitlements), DMG packaging, and
# .sha256. We do NOT re-implement signing — we call the real script.

echo
echo "========================================================================"
echo " STEP 1/2  BUILD SIGNED ARTIFACT (version: $VERSION)"
echo "========================================================================"

echo "==> [1a] cargo build --release -p copypaste-cli -p copypaste-daemon -p copypaste-relay (daemon: cloud-sync,relay-sync)"
cargo build --release -p copypaste-cli -p copypaste-daemon -p copypaste-relay \
    --features copypaste-daemon/cloud-sync,copypaste-daemon/relay-sync

echo "==> [1b] Building Tauri UI bundle (pnpm install && pnpm tauri build)"
(
    cd crates/copypaste-ui
    pnpm install
    pnpm tauri build
)

echo "==> [1c] Packaging + ad-hoc signing the DMG via $BUILD_DMG_SCRIPT $VERSION"
bash "$BUILD_DMG_SCRIPT" "$VERSION"

# Confirm the artifact + checksum actually landed in dist/ (build-dmg-ci.sh
# names it CopyPaste-v<version>-macos-<arch>.dmg). We don't assume the arch.
echo "==> [1d] Confirming artifact + checksum exist in dist/"
shopt -s nullglob
DMGS=( dist/CopyPaste-v"${VERSION}"-macos-*.dmg )
shopt -u nullglob
if [[ ${#DMGS[@]} -eq 0 ]]; then
    echo "ERROR: no DMG matching dist/CopyPaste-v${VERSION}-macos-*.dmg was produced." >&2
    echo "       The build step did not yield the expected artifact. Aborting." >&2
    exit 1
fi
for dmg in "${DMGS[@]}"; do
    if [[ ! -f "${dmg}.sha256" ]]; then
        echo "ERROR: checksum missing for $dmg (expected ${dmg}.sha256)." >&2
        exit 1
    fi
    echo "    artifact: $dmg"
    echo "    checksum: ${dmg}.sha256"
done

echo
echo "==> Signed artifact(s) built. THIS IS NOT A RELEASE APPROVAL."

# =============================================================================
# STEP 2 — Print the MANUAL acceptance checklist (human-only).
# =============================================================================
cat <<'CHECKLIST'

========================================================================
 STEP 2/2  MANUAL ACCEPTANCE CHECKLIST  (a human MUST do all of these)
========================================================================

This script CANNOT perform or observe any of the checks below. Run each one
yourself on REAL hardware. The release is NOT approved until every box is
genuinely ticked by a person.

  CLEAN-MACHINE INSTALL
   1. On a CLEAN macOS system (or a fresh user account that has never run
      CopyPaste), mount the built DMG and drag CopyPaste.app to /Applications.
      It launches without a Gatekeeper hard-block. If Gatekeeper flags it,
      `xattr -dr com.apple.quarantine /Applications/CopyPaste.app` clears it.
   2. Quit and relaunch the app: it starts cleanly, no crash, no zombie
      daemon processes left behind (`pgrep -fl copypaste`).

  HOMEBREW PATH
   3. Fresh `brew install --cask copypaste/tap/copypaste` on a clean host
      succeeds and launches.
   4. `brew upgrade --cask copypaste` from the PREVIOUS released version
      succeeds (no "App source ... is not there" rollback), and the upgraded
      app launches.
   5. `brew uninstall --cask copypaste` removes the app cleanly; reinstall is
      idempotent.

  CORE FUNCTION
   6. Copy text on this Mac → it appears in the CopyPaste history/UI.
   7. Copy an image and a file → both captured and pasteable.
   8. A sensitive value (e.g. a password-shaped string) is detected/flagged
      per the sensitive-content policy.

  SYNC — REQUIRES A REAL SECOND DEVICE
   9. P2P/LAN sync: copy on Device A → it arrives on a REAL second device on
      the same network. mDNS discovery pairs the devices without manual IPs.
  10. Cloud sync (Supabase relay): with the two devices on DIFFERENT networks,
      copy on Device A → it arrives on Device B via the relay.
  11. Conflict / ordering: copy rapidly on both devices → no data loss, no
      duplicate-storm, ordering is sane.

  SETTINGS & SECURITY
  12. Settings window opens and renders fully — no blank pane, no crash.
  13. Toggling a setting persists across an app restart.
  14. Keychain: after first grant, normal use does NOT repeatedly re-prompt
      for Keychain access on every launch/copy.

  ANDROID — REQUIRES A REAL PHONE
  15. Install the Android build (APK) on a REAL Android phone; it launches
      without crashing.
  16. Pair the phone with a desktop device; copy on desktop → appears on the
      phone (and the reverse) over both LAN and cloud paths.

  REGRESSION SWEEP
  17. Run the desktop app for >=10 minutes of normal clipboard activity:
      no panic, no runaway memory, no zombie processes.
  18. Re-read docs/release/RELEASE-CHECKLIST.md and confirm every release-cut
      step there (tag, checksum verify, cask, tap push) is satisfied.

------------------------------------------------------------------------
Full release runbook:  docs/release/RELEASE-CHECKLIST.md
(If docs/RELEASE-ACCEPTANCE.md exists in your checkout, treat it as the
 authoritative acceptance list and reconcile it with the above.)
========================================================================

CHECKLIST

# =============================================================================
# GATE — exit non-zero unless a human has explicitly attested.
# =============================================================================
if [[ "$HUMAN_CONFIRMED" == "1" ]]; then
    echo "==> HUMAN ATTESTATION RECEIVED (--i-verified-on-hardware /"
    echo "    RELEASE_GATE_HUMAN_CONFIRMED=1). You have asserted the checklist"
    echo "    above was completed on real hardware. Gate PASSES (exit 0)."
    echo "    On your head be it: this script trusted your word, it verified nothing."
    exit 0
fi

echo "========================================================================"
echo " GATE: NOT PASSED."
echo
echo " The signed artifact was built, but NO human has confirmed the manual"
echo " checklist on real hardware. This script will not, and cannot, approve"
echo " the release on its own."
echo
echo " When (and ONLY when) you have personally completed every check above"
echo " on real devices, re-run with your explicit attestation:"
echo
echo "     $0 $VERSION --i-verified-on-hardware"
echo "   or"
echo "     RELEASE_GATE_HUMAN_CONFIRMED=1 $0 $VERSION"
echo "========================================================================"
exit 1
