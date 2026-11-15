#!/usr/bin/env bash
set -euo pipefail
case "${1:?expected create or restore}" in
  create)
set -euo pipefail
state="$RUNNER_TEMP/copypaste-release-keychain-state"
keychain="$RUNNER_TEMP/copypaste-release.keychain-db"
password="$(openssl rand -base64 24)"
mkdir -p "$state"
security default-keychain -d user > "$state/default"
security list-keychains -d user > "$state/search-list"
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
security list-keychains -d user -s "$keychain"
security default-keychain -d user -s "$keychain"
echo "COPYPASTE_KEYCHAIN_TEST=1" >> "$GITHUB_ENV"
echo "COPYPASTE_KEYCHAIN_TEST_PATH=$keychain" >> "$GITHUB_ENV"
    ;;
  restore)
set -euo pipefail
state="$RUNNER_TEMP/copypaste-release-keychain-state"
keychain="$RUNNER_TEMP/copypaste-release.keychain-db"
python3 - "$state/default" "$state/search-list" <<'PY'
from pathlib import Path
import shlex
import subprocess
import sys
default = shlex.split(Path(sys.argv[1]).read_text(encoding="utf-8"), posix=True)
search = shlex.split(Path(sys.argv[2]).read_text(encoding="utf-8"), posix=True)
if len(default) != 1 or not search:
    raise SystemExit("saved Keychain state is invalid")
subprocess.run(["security", "default-keychain", "-d", "user", "-s", default[0]], check=True)
subprocess.run(["security", "list-keychains", "-d", "user", "-s", *search], check=True)
PY
security delete-keychain "$keychain"
    ;;
  *) echo "Unknown keychain operation" >&2; exit 1 ;;
esac
