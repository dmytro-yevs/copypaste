#!/usr/bin/env bash
# gen-cask.sh — update Casks/copypaste.rb with new version + sha256.
#
# Usage (CI — auto mode, called from release.yml after GitHub Release is created):
#   scripts/release/gen-cask.sh
#
# Usage (manual mode — maintainer supplies values explicitly):
#   scripts/release/gen-cask.sh <version> <sha256>
#
# Auto mode: discovers version from the latest GitHub Release tag, downloads
# the DMG, computes its sha256, and updates Casks/copypaste.rb in place.
# Requires: gh CLI authenticated, curl, shasum.
#
# Manual mode: <version> must be bare (no leading 'v'), <sha256> must be
# 64 lowercase hex chars.
#
# After updating the cask the script prints a git diff. In CI it also
# commits and pushes the change directly (GITHUB_ACTIONS=true).
set -euo pipefail

REPO="dmytro-yevs/copypaste"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

CASK="Casks/copypaste.rb"
if [[ ! -f "$CASK" ]]; then
    echo "ERROR: cask not found at $CASK" >&2
    exit 1
fi

# ── Resolve VERSION and SHA256 ────────────────────────────────────────────────

if [[ $# -ge 2 ]]; then
    # Manual mode: caller supplies version and sha256.
    VERSION="${1}"
    SHA256="${2}"

    # Reject leading 'v' on version to keep cask string clean.
    if [[ "$VERSION" == v* ]]; then
        echo "ERROR: pass version without leading 'v' (got: $VERSION)" >&2
        exit 1
    fi

    # Validate sha256 shape: 64 lowercase hex chars.
    if [[ ! "$SHA256" =~ ^[a-f0-9]{64}$ ]]; then
        echo "ERROR: sha256 must be 64 lowercase hex chars (got: $SHA256)" >&2
        exit 1
    fi

else
    # Auto mode: derive everything from the latest GitHub Release.
    echo "==> Auto mode: fetching release info from github.com/${REPO}"

    if ! command -v gh &>/dev/null; then
        echo "ERROR: gh CLI not found — cannot auto-fetch release info" >&2
        exit 1
    fi

    TAG="$(gh release view --repo "$REPO" --json tagName --jq '.tagName')"
    if [[ -z "$TAG" ]]; then
        echo "ERROR: could not determine latest release tag" >&2
        exit 1
    fi

    # Strip leading 'v' for the cask version field.
    VERSION="${TAG#v}"

    # The CI DMG filename pattern:
    #   build-dmg-ci.sh produces CopyPaste-v<tag>-macos-arm64.dmg
    #   where <tag> already has the leading 'v', giving the double-vv prefix.
    DMG_NAME="CopyPaste-v${TAG}-macos-arm64.dmg"
    # gh's --jq is a single-string filter (no --arg). Use bash to embed the name.
    DMG_URL="$(gh release view --repo "$REPO" --json assets \
        --jq ".assets[] | select(.name==\"${DMG_NAME}\") | .url")"

    if [[ -z "$DMG_URL" ]]; then
        echo "ERROR: asset '${DMG_NAME}' not found in release ${TAG}" >&2
        echo "       Available assets:" >&2
        gh release view --repo "$REPO" --json assets --jq '.assets[].name' >&2
        exit 1
    fi

    echo "    tag     → ${TAG}"
    echo "    version → ${VERSION}"
    echo "    dmg url → ${DMG_URL}"
    echo "==> Downloading DMG to compute sha256 ..."

    TMP_DMG="$(mktemp /tmp/copypaste-XXXXXX.dmg)"
    trap 'rm -f "$TMP_DMG"' EXIT
    curl -fsSL --output "$TMP_DMG" "$DMG_URL"
    SHA256="$(shasum -a 256 "$TMP_DMG" | awk '{print $1}')"
    echo "    sha256  → ${SHA256}"
fi

# ── Update Tauri productVersion so Info.plist matches the cask version ──────
# tauri.conf.json carries a hardcoded "version" field that becomes
# CFBundleShortVersionString / CFBundleVersion in the .app bundle's Info.plist.
# Keeping it in sync with the cask version avoids Homebrew --adopt mismatches
# (Homebrew compares source bundle version to target when adopting an existing
# app) and makes `sw_vers` / bundle version queries return the correct value.

TAURI_CONF="crates/copypaste-ui/src-tauri/tauri.conf.json"
if [[ -f "$TAURI_CONF" ]]; then
    echo "==> Updating $TAURI_CONF version → $VERSION"
    TMP_TAURI="$(mktemp)"
    # Replace the top-level "version" field only (not nested version strings).
    # Use a simple sed that matches the exact JSON key pattern emitted by Tauri.
    sed 's/"version": "[^"]*"/"version": "'"$VERSION"'"/' "$TAURI_CONF" > "$TMP_TAURI"
    # Verify the change landed.
    if grep -qF "\"version\": \"${VERSION}\"" "$TMP_TAURI"; then
        mv "$TMP_TAURI" "$TAURI_CONF"
    else
        echo "WARNING: could not update version in $TAURI_CONF — continuing anyway" >&2
        rm -f "$TMP_TAURI"
    fi
else
    echo "WARNING: $TAURI_CONF not found — skipping Tauri version bump" >&2
fi

# ── Apply changes to cask ─────────────────────────────────────────────────────

echo "==> Updating $CASK"
echo "    version → $VERSION"
echo "    sha256  → $SHA256"

TMP="$(mktemp)"
awk -v ver="$VERSION" -v sha="$SHA256" '
    {
        if (match($0, /^([[:space:]]*)version[[:space:]]+"[^"]*"/, m)) {
            print m[1] "version \"" ver "\""
            next
        }
        # Match sha256 literal string or sha256 :no_check (with optional comment).
        if (match($0, /^([[:space:]]*)sha256[[:space:]]+/, m)) {
            print m[1] "sha256 \"" sha "\""
            next
        }
        print
    }
' "$CASK" > "$TMP"

# Verify both fields actually changed.
if ! grep -qE "^[[:space:]]*version[[:space:]]+\"$VERSION\"" "$TMP"; then
    echo "ERROR: failed to update version line in $CASK" >&2
    rm -f "$TMP"
    exit 1
fi
if ! grep -qE "^[[:space:]]*sha256[[:space:]]+\"$SHA256\"" "$TMP"; then
    echo "ERROR: failed to update sha256 line in $CASK" >&2
    rm -f "$TMP"
    exit 1
fi

mv "$TMP" "$CASK"

echo
echo "==> Diff:"
git --no-pager diff -- "$CASK" || true

# ── CI auto-commit ────────────────────────────────────────────────────────────

if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    echo
    echo "==> CI mode: committing and pushing cask update ..."
    git config user.name  "github-actions[bot]"
    git config user.email "github-actions[bot]@users.noreply.github.com"

    # Embed GH_TOKEN into remote URL — release.yml checks out at tag ref,
    # so HEAD is detached and the default credential helper has no token.
    if [[ -n "${GH_TOKEN:-}" ]]; then
        git remote set-url origin "https://x-access-token:${GH_TOKEN}@github.com/${REPO}.git"
    fi

    # Save the freshly-generated cask content before any branch switching.
    NEW_CASK_CONTENT="$(cat "$CASK")"

    # Overwrite-on-main strategy (no cherry-pick):
    #   1) Fetch + reset to remote main (avoids detached-HEAD / cherry-pick conflicts).
    #   2) Drop the new cask file in — overwriting whatever stale content is there.
    #   3) If no diff → already up to date, exit 0.
    #   4) Commit + push. Retry up to 3 times on push race.
    git fetch origin main
    git checkout -B main origin/main

    printf '%s\n' "$NEW_CASK_CONTENT" > "$CASK"
    git add "$CASK"
    # Also stage tauri.conf.json if it was updated.
    [[ -f "$TAURI_CONF" ]] && git add "$TAURI_CONF" || true

    if git diff --cached --quiet; then
        echo "Cask already up to date on main — nothing to push."
        echo "Done."
        exit 0
    fi

    git commit -m "chore(cask): bump to ${VERSION} [skip ci]"

    PUSH_ATTEMPTS=0
    MAX_ATTEMPTS=3
    until git push origin main; do
        PUSH_ATTEMPTS=$(( PUSH_ATTEMPTS + 1 ))
        if [[ $PUSH_ATTEMPTS -ge $MAX_ATTEMPTS ]]; then
            echo "ERROR: push failed after ${MAX_ATTEMPTS} attempts" >&2
            exit 1
        fi
        echo "Push rejected (race); retrying (attempt $((PUSH_ATTEMPTS + 1))/${MAX_ATTEMPTS}) ..."
        git fetch origin main
        git reset --hard origin/main
        printf '%s\n' "$NEW_CASK_CONTENT" > "$CASK"
        git add "$CASK"
        [[ -f "$TAURI_CONF" ]] && git add "$TAURI_CONF" || true
        if git diff --cached --quiet; then
            echo "Cask already up to date after re-fetch — nothing to push."
            echo "Done."
            exit 0
        fi
        git commit -m "chore(cask): bump to ${VERSION} [skip ci]"
    done
    echo "Done."
else
    echo
    echo "Done. Review then commit:"
    echo "  git add $CASK ${TAURI_CONF}"
    echo "  git commit -m \"chore(cask): bump to $VERSION\""
fi
