#!/usr/bin/env bash
# submit-winget.sh — open a WinGet community repository update for a release.
#
#   scripts/release/submit-winget.sh <version> <installer-sha256>
#
# WinGet marks installers from its community source as trusted after their hash
# matches, so `winget install` does not raise the SmartScreen prompt that a
# browser download of the self-signed installer does.
#
# GITHUB_TOKEN must be a classic token with the public_repo scope whose account
# owns a fork of microsoft/winget-pkgs. GITHUB_REPOSITORY names the release.
set -euo pipefail

PACKAGE_IDENTIFIER="DmytroYevs.CopyPaste"
KOMAC_VERSION="2.17.1"
KOMAC_SHA256="e694e79cd451afabf47bead66ed34586fef4814f7b94e4b61a098f419978d5d7"

usage() {
    echo "Usage: $0 <version> <installer-sha256>" >&2
    exit 1
}

[[ $# -eq 2 ]] || usage
VERSION="$1"
SHA256="$2"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "ERROR: version must be bare MAJOR.MINOR.PATCH (got: $VERSION)" >&2
    exit 1
fi
if [[ ! "$SHA256" =~ ^[a-f0-9]{64}$ ]]; then
    echo "ERROR: sha256 must be 64 lowercase hex characters (got: $SHA256)" >&2
    exit 1
fi
: "${GITHUB_TOKEN:?missing WinGet submission token}"
: "${GITHUB_REPOSITORY:?missing release repository}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# WinGet records the hash of whatever this URL serves, so it must still serve
# the qualified installer.
URL="https://github.com/${GITHUB_REPOSITORY}/releases/download/v${VERSION}/CopyPaste-v${VERSION}-windows-x86_64-setup.exe"
curl --fail --location --silent --show-error --output "$WORK/installer.exe" "$URL"
PUBLISHED="$(sha256sum "$WORK/installer.exe" | cut -d' ' -f1)"
if [[ "$PUBLISHED" != "$SHA256" ]]; then
    echo "ERROR: published installer $PUBLISHED does not match qualified $SHA256" >&2
    exit 1
fi

curl --fail --location --silent --show-error --output "$WORK/komac.tar.gz" \
    "https://github.com/russellbanks/Komac/releases/download/v${KOMAC_VERSION}/komac-${KOMAC_VERSION}-x86_64-unknown-linux-gnu.tar.gz"
echo "${KOMAC_SHA256}  $WORK/komac.tar.gz" | sha256sum --check --strict --quiet
tar -xzf "$WORK/komac.tar.gz" -C "$WORK" komac

"$WORK/komac" update "$PACKAGE_IDENTIFIER" --version "$VERSION" --urls "$URL" --submit
