#!/usr/bin/env bash
# build-macos-app.sh — build CopyPaste.app, inject the daemon and CLI, ad-hoc sign.
#
#   Usage: scripts/release/build-macos-app.sh <version> [arch]
#          arch defaults to the host (arm64 on Apple Silicon).
#
# Output: dist/CopyPaste.app — staged, complete and ad-hoc signed.
#
# Must run on macOS: codesign and PlistBuddy have no Linux equivalent. Nothing
# in this file has been executed on a Mac by its author; see the header of
# release.yml for what that means.
#
# ---------------------------------------------------------------------------
# Why this signs by hand instead of letting Tauri do it
# ---------------------------------------------------------------------------
# The Tauri CLI signs the bundle itself when APPLE_SIGNING_IDENTITY is set, and
# the obvious thing to do is set it to "-". Two reasons not to:
#
#   1. tauri-apps/tauri#13804: with the identity "-", `tauri build` signs the
#      .app successfully and then fails intermittently in bundle_dmg.sh, on
#      both architectures, on CI. Re-running the identical build succeeds. A
#      release pipeline that fails one run in N for no reason is worse than no
#      pipeline.
#   2. We have to re-sign anyway. Tauri bundles only its own binary; the daemon
#      and the CLI are injected afterwards, and injecting an unsigned Mach-O
#      into a signed bundle breaks the seal. Signing last is the only order
#      that produces a valid bundle.
#
# So: `tauri build --bundles app` with no signing identity in the environment,
# then sign here, then build the DMG in make-dmg.sh.
#
# ---------------------------------------------------------------------------
# Why there is no hardened runtime and no entitlements file
# ---------------------------------------------------------------------------
# The hardened runtime is a *precondition for notarisation*, not a benefit on
# its own. ADR-0001 rules notarisation out, so `--options runtime` buys nothing
# — and it costs something specific. v1 shipped `--options runtime` on an
# ad-hoc signature and had to add a `codesign --force --deep --sign -` to the
# cask's postflight to make the app launchable at all; its recorded symptom was
# `RBSRequestErrorDomain Code=5` / POSIX 163, surfaced to users as "CopyPaste.app
# can't be opened."
#
# Dropping the hardened runtime should remove that failure mode along with the
# entitlements file. UNVERIFIED — reasoned from Apple's documented behaviour,
# not observed. If a quarantined ad-hoc build still refuses to launch on a real
# Mac, the fallback is v1's: re-sign in the cask postflight. Casks/copypaste.rb
# documents where that goes.
#
# ---------------------------------------------------------------------------
# Why the release artefact is still ad-hoc when the cask re-signs anyway
# ---------------------------------------------------------------------------
# The signature that matters for TCC is applied on the user's machine, by
# packaging/macos/selfsign.sh, with a certificate that only exists there. CI has
# no certificate and ADR-0001 says it never will. So this build signs ad-hoc —
# which is the floor macOS requires to execute a Mach-O at all — and stages the
# re-signing script into the bundle for the cask to run.
set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
    echo "ERROR: version required. Usage: $0 <version> [arm64|x86_64]" >&2
    exit 1
fi
# Accept a tag or a bare version; the rest of the pipeline uses the bare form
# and re-adds a single 'v' where it wants one. v1 shipped a DMG called
# CopyPaste-vv0.5.1-... exactly once because this normalisation was missing.
VERSION="${VERSION#v}"

ARCH="${2:-}"
if [[ -z "$ARCH" ]]; then
    case "$(uname -m)" in
        arm64)  ARCH="arm64"  ;;
        x86_64) ARCH="x86_64" ;;
        *) echo "ERROR: cannot infer arch from $(uname -m); pass it explicitly" >&2; exit 1 ;;
    esac
fi
case "$ARCH" in
    arm64)  TRIPLE="aarch64-apple-darwin" ;;
    x86_64) TRIPLE="x86_64-apple-darwin"  ;;
    *) echo "ERROR: arch must be arm64 or x86_64 (got: $ARCH)" >&2; exit 1 ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: must run on macOS (this host is $(uname -s))" >&2
    exit 1
fi

APP_NAME="CopyPaste"
DIST="dist"
APP="${DIST}/${APP_NAME}.app"
UI_DIR="crates/copypaste-ui"

# ---------------------------------------------------------------------------
# 1. Version agreement
# ---------------------------------------------------------------------------
# tauri.conf.json carries no "version" key, so Tauri reads the Cargo package
# version — which comes from [workspace.package]. That is the right design
# (one source of truth) and it means this script must never rewrite it. It
# checks instead: a mismatch is a release-preparation mistake, and failing here
# is much cheaper than shipping a DMG whose Info.plist disagrees with its
# filename and with the cask that points at it.
WS_VERSION="$(awk '/^\[workspace\.package\]/{f=1} f && /^version *=/{gsub(/[" ]/,"",$3); print $3; exit}' Cargo.toml)"
if [[ "$WS_VERSION" != "$VERSION" ]]; then
    echo "ERROR: version mismatch." >&2
    echo "       requested       : $VERSION" >&2
    echo "       Cargo.toml says : $WS_VERSION" >&2
    echo "       Bump [workspace.package] version to match the tag, or tag the version that is there." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# 2. Build
# ---------------------------------------------------------------------------
echo "==> Building the daemon and CLI for $TRIPLE"
rustup target add "$TRIPLE" >/dev/null 2>&1 || true
cargo build --release --locked --target "$TRIPLE" -p copypaste-daemon -p copypaste-cli

echo "==> Building the frontend and the .app bundle"
(
    cd "$UI_DIR"
    npm ci
    # --bundles app: produce only the .app. The DMG is made by make-dmg.sh,
    # for the reasons in the header.
    #
    # APPLE_SIGNING_IDENTITY is explicitly cleared rather than merely unset, so
    # an identity leaking in from the runner environment cannot silently change
    # what this produces. There is no credential anywhere in this pipeline and
    # this is one of the places that has to stay true.
    APPLE_SIGNING_IDENTITY='' npm run tauri -- build --target "$TRIPLE" --bundles app
)

BUILT_APP="target/${TRIPLE}/release/bundle/macos/${APP_NAME}.app"
if [[ ! -d "$BUILT_APP" ]]; then
    echo "ERROR: $BUILT_APP not found after 'tauri build'." >&2
    echo "       Bundles present:" >&2
    ls -1 "target/${TRIPLE}/release/bundle" 2>/dev/null >&2 || echo "       (no bundle directory)" >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# 3. Stage and inject
# ---------------------------------------------------------------------------
echo "==> Staging $APP"
rm -rf "$APP"
mkdir -p "$DIST"
cp -R "$BUILT_APP" "$APP"

BIN_DIR="${APP}/Contents/MacOS"
for bin in copypaste copypaste-daemon; do
    src="target/${TRIPLE}/release/${bin}"
    [[ -f "$src" ]] || { echo "ERROR: $src missing" >&2; exit 1; }
    cp "$src" "$BIN_DIR/"
    echo "    injected $bin"
done

# The bundle's own executable must exist and be named what Info.plist claims.
# A mismatch here is the leading cause of the "App source is not there" class
# of Homebrew upgrade failures: brew copies the bundle, macOS refuses to open
# it, postflight fails, brew rolls back, and the error names a file it has
# already removed.
PLIST="${APP}/Contents/Info.plist"
EXEC="$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$PLIST" 2>/dev/null || true)"
[[ -n "$EXEC" ]] || { echo "ERROR: CFBundleExecutable unset in $PLIST" >&2; exit 1; }
[[ -f "${BIN_DIR}/${EXEC}" ]] || {
    echo "ERROR: CFBundleExecutable '$EXEC' is not present at ${BIN_DIR}/${EXEC}" >&2
    exit 1
}

# ---------------------------------------------------------------------------
# CFBundleShortVersionString, and the pre-release suffix
# ---------------------------------------------------------------------------
# Apple's documented format for CFBundleShortVersionString is one to three
# period-separated integers. A tag like v2.0.0-alpha.1 is not that, and nobody
# on this project has yet watched Tauri's macOS bundler decide what to do with
# one — it may pass the string through, or strip the suffix.
#
# Both outcomes are survivable and they are not equally survivable, so they are
# distinguished rather than lumped together:
#
#   * a different numeric core means the bundle claims to be a different release
#     from the DMG that contains it and the cask that points at the DMG. That is
#     a broken release and it stops here.
#   * a dropped "-alpha.1" is cosmetic. The cask keys on the DMG filename, so
#     the install still works; the user sees "2.0.0" in About. Loud warning, and
#     it goes into the CI step summary so the first real run answers the
#     question instead of burying it.
SHORT_VERSION="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$PLIST" 2>/dev/null || true)"
BUNDLE_VERSION="$(/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$PLIST" 2>/dev/null || true)"
echo "    CFBundleExecutable=$EXEC"
echo "    CFBundleShortVersionString=${SHORT_VERSION:-<unset>}  CFBundleVersion=${BUNDLE_VERSION:-<unset>}"

[[ -n "$SHORT_VERSION" ]] || {
    echo "ERROR: CFBundleShortVersionString is unset in the built bundle." >&2
    echo "       Tauri normally writes it from the Cargo package version. An empty" >&2
    echo "       value means the bundler did something unexpected; do not ship it." >&2
    exit 1
}

if [[ "$SHORT_VERSION" != "$VERSION" ]]; then
    if [[ "$SHORT_VERSION" == "${VERSION%%-*}" ]]; then
        MSG="Tauri wrote CFBundleShortVersionString=$SHORT_VERSION for version $VERSION — the pre-release suffix was dropped"
        echo "WARNING: $MSG" >&2
        # Plain `[[ ... ]] && cmd` as a statement would trip `set -e` whenever the
        # test is false, which is the common case. Keep it an `if`.
        if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
            echo "> **Note:** $MSG" >> "$GITHUB_STEP_SUMMARY"
        fi
        if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
            echo "::warning::$MSG"
        fi
    else
        echo "ERROR: CFBundleShortVersionString is '$SHORT_VERSION' but this build is '$VERSION'." >&2
        echo "       The bundle would disagree with its own DMG filename and with the cask." >&2
        exit 1
    fi
fi

# ---------------------------------------------------------------------------
# Stage the on-machine signing script
# ---------------------------------------------------------------------------
# packaging/macos/selfsign.sh runs on the user's machine, from the cask's
# postflight, and does the de-quarantine and the re-sign with a per-machine
# certificate. It travels inside the bundle rather than being pasted into the
# cask so that it is reviewable in a diff, testable on its own, and identical in
# the tap and in this repository.
#
# It is copied in BEFORE signing, so the signature seals it. A script the cask
# executes must be covered by the seal like any other resource.
SELFSIGN_SRC="packaging/macos/selfsign.sh"
[[ -f "$SELFSIGN_SRC" ]] || { echo "ERROR: $SELFSIGN_SRC missing" >&2; exit 1; }
bash -n "$SELFSIGN_SRC" || { echo "ERROR: $SELFSIGN_SRC does not parse" >&2; exit 1; }
mkdir -p "${APP}/Contents/Resources"
cp "$SELFSIGN_SRC" "${APP}/Contents/Resources/selfsign.sh"
chmod 755 "${APP}/Contents/Resources/selfsign.sh"
echo "    staged Contents/Resources/selfsign.sh"

# ---------------------------------------------------------------------------
# 4. Sign, ad-hoc, inside out
# ---------------------------------------------------------------------------
# Inner Mach-O binaries first, then the bundle. NOT `--deep`: Apple deprecated
# it, and it is the wrong tool anyway — it re-signs nested code with the outer
# identity and no per-binary identifier, which is how you get identifiers that
# rotate on every rebuild.
#
# `--identifier` pins a stable identifier per binary. Under an ad-hoc signature
# the cdhash still changes every build (that is ADR-0001's whole problem, and
# why this app needs no TCC grant), but a stable identifier keeps everything
# that is *not* cdhash-derived deterministic.
#
# --timestamp=none: a secure timestamp needs Apple's timestamp server and is
# only meaningful for a real identity. Requesting one here would make the build
# depend on network reachability for no benefit.
echo "==> Ad-hoc signing"
for bin in copypaste copypaste-daemon; do
    codesign --force --sign - \
        --identifier "com.copypaste.${bin}" \
        --timestamp=none \
        "${BIN_DIR}/${bin}"
done

codesign --force --sign - --timestamp=none "$APP"

echo "==> Verifying"
codesign --verify --strict --verbose=2 "$APP"

# Print the designated requirement. On an ad-hoc signature this is a bare cdhash
# and nothing else, which is exactly why a TCC grant against this bundle would
# die on the next build — and why the cask re-signs on the user's machine. Worth
# having in the release log the first time anyone reads one.
echo "==> Designated requirement (ad-hoc: expect a bare cdhash)"
codesign --display --requirements - "$APP" 2>&1 || true

# Reported, not enforced. `spctl` will reject an ad-hoc bundle — that is the
# expected outcome and the entire subject of ADR-0001. Printing it keeps the
# release log honest about what was produced.
echo "==> Gatekeeper assessment (expected to be rejected — see ADR-0001)"
spctl --assess --type execute --verbose=4 "$APP" 2>&1 || true

echo
echo "Built: $APP"
