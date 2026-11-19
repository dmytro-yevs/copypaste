#!/usr/bin/env bash
set -euo pipefail

ARTIFACT="${1:-}"
OUTPUT="${2:-${ARTIFACT}.sig}"
[[ -f "$ARTIFACT" ]] || {
    echo "ERROR: updater artifact is missing" >&2
    exit 1
}
: "${TAURI_SIGNING_PRIVATE_KEY:?missing updater signing key}"
: "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:?missing updater signing password}"
[[ -z "${TAURI_SIGNING_PRIVATE_KEY_PATH+x}" && -z "${TAURI_PRIVATE_KEY_PATH+x}" ]] || {
    echo "ERROR: updater signing accepts the repository secret only through the environment" >&2
    exit 1
}

artifact="$(cd "$(dirname "$ARTIFACT")" && pwd)/$(basename "$ARTIFACT")"
# Install the signer from the committed lockfile so every package is
# integrity-pinned, with lifecycle scripts disabled and the key withheld.
pinned="$(cd "$(dirname "${BASH_SOURCE[0]}")/updater-signer" && pwd)"
signer="$(mktemp -d)"
trap 'rm -rf "$signer"' EXIT
cp "$pinned/package.json" "$pinned/package-lock.json" "$signer/"
(cd "$signer" && unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD \
    && npm ci --ignore-scripts --no-audit --no-fund)
"$signer/node_modules/.bin/tauri" signer sign "$artifact"
[[ -s "${artifact}.sig" ]] || {
    echo "ERROR: updater signer did not create a signature" >&2
    exit 1
}
output="$(cd "$(dirname "$OUTPUT")" && pwd)/$(basename "$OUTPUT")"
if [[ "${artifact}.sig" != "$output" ]]; then
    cp "${artifact}.sig" "$output"
fi
[[ -s "$output" ]] || {
    echo "ERROR: updater signature is empty" >&2
    exit 1
}
