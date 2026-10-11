#!/usr/bin/env bash
set -euo pipefail
artifacts=""; session=""; output=""
while [[ "$#" -gt 0 ]]; do case "$1" in --artifacts) artifacts="$2"; shift 2;; --session) session="$2"; shift 2;; --output) output="$2"; shift 2;; *) exit 2;; esac; done
[[ -d "$artifacts" && ( "$session" == x11 || "$session" == wayland ) && -n "$output" ]] || exit 2
appimage="$(find "$artifacts" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
[[ -n "$appimage" ]] || exit 1
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
(cd "$work" && APPIMAGE_EXTRACT_AND_RUN=1 "$appimage" --appimage-extract >/dev/null)
prefix="$work/squashfs-root/usr/lib/copypaste"
"$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/run-linux-clipboard-diagnostic.sh" --output "$output" -- \
  python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/linux-first-clipboard-diagnostic.py" --runtime-prefix "$prefix" --gui-launcher "$work/squashfs-root/AppRun" --session "$session" --output "${output%.json}.capture.json"
