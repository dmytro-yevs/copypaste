#!/usr/bin/env bash
# check-license-headers.sh — verify SPDX license headers on .rs and .kt sources
#
# Scans crates/ and android/ for *.rs and *.kt files. Each file must contain a
# `SPDX-License-Identifier: MIT OR Apache-2.0` comment within the first few lines.
#
# Modes:
#   --dry-run   (default) list violations, exit non-zero if any are found
#   --fix       insert the SPDX header at the top of any offending file
#   --help      show usage
set -euo pipefail

SPDX_LINE="SPDX-License-Identifier: MIT OR Apache-2.0"
RS_HEADER="// ${SPDX_LINE}"
KT_HEADER="// ${SPDX_LINE}"
HEAD_LINES=5

MODE="dry-run"

usage() {
    cat <<EOF
Usage: $(basename "$0") [--dry-run|--fix|--help]

Scans .rs and .kt files under crates/ and android/ for an SPDX license header
("${SPDX_LINE}") within the first ${HEAD_LINES} lines.

Options:
  --dry-run   List files missing the header (default). Exit 1 if any missing.
  --fix       Prepend the header to any file missing it.
  --help      Show this message.

Exit codes:
  0   All files have headers (or --fix succeeded).
  1   Violations found in --dry-run.
  2   Invalid arguments or missing source roots.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) MODE="dry-run"; shift ;;
        --fix)     MODE="fix";     shift ;;
        --help|-h) usage; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ROOTS=()
[[ -d crates  ]] && ROOTS+=("crates")
[[ -d android ]] && ROOTS+=("android")

if [[ ${#ROOTS[@]} -eq 0 ]]; then
    echo "error: no crates/ or android/ directories found in ${REPO_ROOT}" >&2
    exit 2
fi

violations=()

while IFS= read -r -d '' file; do
    # Skip generated dirs that may live inside the roots
    case "$file" in
        */target/*|*/build/*|*/.gradle/*|*/generated/*) continue ;;
    esac
    if ! head -n "$HEAD_LINES" "$file" | grep -qF "$SPDX_LINE"; then
        violations+=("$file")
    fi
done < <(find "${ROOTS[@]}" \( -name '*.rs' -o -name '*.kt' \) -type f -print0)

if [[ ${#violations[@]} -eq 0 ]]; then
    echo "ok: all scanned files contain '${SPDX_LINE}'"
    exit 0
fi

if [[ "$MODE" == "dry-run" ]]; then
    echo "Missing SPDX header in ${#violations[@]} file(s):"
    for f in "${violations[@]}"; do
        echo "  $f"
    done
    exit 1
fi

# --fix
for f in "${violations[@]}"; do
    case "$f" in
        *.rs) header="$RS_HEADER" ;;
        *.kt) header="$KT_HEADER" ;;
        *)    continue ;;
    esac
    tmp="$(mktemp)"
    {
        printf '%s\n' "$header"
        cat "$f"
    } > "$tmp"
    mv "$tmp" "$f"
    echo "fixed: $f"
done

echo "ok: inserted SPDX header into ${#violations[@]} file(s)"
