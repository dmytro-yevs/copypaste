#!/usr/bin/env bash
# Exercise the release signing script end to end with a throwaway key, so a
# stale lockfile hash or a platform fault fails here instead of in a release.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp scripts/release/updater-signer/package.json scripts/release/updater-signer/package-lock.json "$work/"
(cd "$work" && npm ci --ignore-scripts --no-audit --no-fund)
password="$(openssl rand -hex 16)"
"$work/node_modules/.bin/tauri" signer generate --ci -p "$password" -w "$work/throwaway.key" >/dev/null
printf 'updater signer probe\n' > "$work/probe.bin"
TAURI_SIGNING_PRIVATE_KEY="$(cat "$work/throwaway.key")" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$password" \
    bash scripts/release/sign-update-artifact.sh "$work/probe.bin"

python3 - "$work/probe.bin" "$work/throwaway.key.pub" <<'PY'
import base64
import runpy
import sys
from pathlib import Path

artifact, public = Path(sys.argv[1]), Path(sys.argv[2])
verify = runpy.run_path("scripts/release/stage-compositor-runtime-release.py")["verify_package_signature"]
key = base64.b64decode(public.read_text(encoding="utf-8").strip()).decode("utf-8").splitlines()[1]
signature = artifact.with_name(artifact.name + ".sig")
verify(artifact, signature, public_key=key)
artifact.write_bytes(b"changed after signing\n")
try:
    verify(artifact, signature, public_key=key)
except ValueError:
    pass
else:
    raise SystemExit("ERROR: the verifier accepted bytes changed after signing")
PY
echo "The pinned updater signer produced a signature the repository verifier accepts."
