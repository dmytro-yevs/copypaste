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
npx --yes @tauri-apps/cli@2.11.4 signer sign "$artifact"
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
