#!/usr/bin/env bash
# gen-changelog.sh — generate CHANGELOG.md from Conventional Commits via git-cliff.
#
# Examples:
#   scripts/gen-changelog.sh                              # full changelog -> CHANGELOG.md
#   scripts/gen-changelog.sh --since v0.1.0               # only commits since v0.1.0
#   scripts/gen-changelog.sh --tag v0.2.0                 # render unreleased section as v0.2.0
#   scripts/gen-changelog.sh --output RELEASE_NOTES.md    # write to a different file
#   scripts/gen-changelog.sh --since v0.1.0 --tag v0.2.0 --output RELEASE_NOTES.md
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CONFIG="${ROOT_DIR}/cliff.toml"

SINCE=""
OUTPUT="CHANGELOG.md"
TAG=""

usage() {
    cat <<EOF
${SCRIPT_NAME} — generate CHANGELOG.md via git-cliff.

Usage: ${SCRIPT_NAME} [options]

Options:
  --since <tag>     Only include commits after <tag> (e.g. v0.1.0).
  --output <file>   Output path (default: CHANGELOG.md).
  --tag <version>   Tag name for the unreleased section (e.g. v0.2.0).
  -h, --help        Show this help and exit.

Requires:
  git-cliff (https://git-cliff.org)
    macOS:   brew install git-cliff
    Cargo:   cargo install git-cliff
    Other:   see https://git-cliff.org/docs/installation
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --since)
            [[ $# -ge 2 ]] || { echo "error: --since requires a value" >&2; exit 2; }
            SINCE="$2"
            shift 2
            ;;
        --output)
            [[ $# -ge 2 ]] || { echo "error: --output requires a value" >&2; exit 2; }
            OUTPUT="$2"
            shift 2
            ;;
        --tag)
            [[ $# -ge 2 ]] || { echo "error: --tag requires a value" >&2; exit 2; }
            TAG="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if ! command -v git-cliff >/dev/null 2>&1; then
    cat >&2 <<EOF
error: git-cliff is not installed.

Install it with one of:
  brew install git-cliff
  cargo install git-cliff
  See https://git-cliff.org/docs/installation
EOF
    exit 127
fi

if [[ ! -f "${CONFIG}" ]]; then
    echo "error: ${CONFIG} not found" >&2
    exit 1
fi

CLIFF_ARGS=(--config "${CONFIG}" --output "${OUTPUT}")

if [[ -n "${TAG}" ]]; then
    CLIFF_ARGS+=(--tag "${TAG}")
fi

if [[ -n "${SINCE}" ]]; then
    # git-cliff accepts a revspec to narrow history.
    CLIFF_ARGS+=("${SINCE}..HEAD")
fi

echo "==> git-cliff ${CLIFF_ARGS[*]}"
( cd "${ROOT_DIR}" && git-cliff "${CLIFF_ARGS[@]}" )

echo "==> wrote ${OUTPUT}"
