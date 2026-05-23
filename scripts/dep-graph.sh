#!/usr/bin/env bash
# dep-graph.sh — render workspace dependency graph as SVG via cargo-depgraph + graphviz.
#
# Examples:
#   scripts/dep-graph.sh                              # workspace-only -> reports/depgraph.svg
#   scripts/dep-graph.sh --full                       # full graph    -> reports/depgraph-full.svg
#   scripts/dep-graph.sh --workspace-only             # explicit workspace-only mode
#   scripts/dep-graph.sh --output reports/deps.svg    # custom output path
#   scripts/dep-graph.sh --dry-run                    # print commands without running them
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

MODE="workspace-only"
OUTPUT=""
DRY_RUN=0

usage() {
    cat <<EOF
${SCRIPT_NAME} — render workspace dependency graph via cargo-depgraph + graphviz.

Usage: ${SCRIPT_NAME} [options]

Options:
  --workspace-only   Only workspace crates (default). Output: reports/depgraph.svg
  --full             Include all dependencies. Output: reports/depgraph-full.svg
  --output <path>    Custom output SVG path (overrides default).
  --dry-run          Print commands without invoking cargo / dot.
  -h, --help         Show this help and exit.

Requires:
  cargo-depgraph  (https://crates.io/crates/cargo-depgraph)
    Install with:  cargo install cargo-depgraph
  graphviz (dot)  (https://graphviz.org)
    macOS:  brew install graphviz
    Debian: sudo apt-get install graphviz
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --workspace-only)
            MODE="workspace-only"
            shift
            ;;
        --full)
            MODE="full"
            shift
            ;;
        --output)
            [[ $# -ge 2 ]] || { echo "error: --output requires a value" >&2; exit 2; }
            OUTPUT="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            shift
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

if [[ -z "${OUTPUT}" ]]; then
    if [[ "${MODE}" == "full" ]]; then
        OUTPUT="reports/depgraph-full.svg"
    else
        OUTPUT="reports/depgraph.svg"
    fi
fi

if [[ "${DRY_RUN}" -eq 0 ]]; then
    if ! cargo depgraph --help >/dev/null 2>&1; then
        cat >&2 <<EOF
error: cargo-depgraph is not installed.

Install it with:
  cargo install cargo-depgraph
EOF
        exit 127
    fi

    if ! command -v dot >/dev/null 2>&1; then
        cat >&2 <<EOF
error: graphviz 'dot' is not installed.

Install it with one of:
  brew install graphviz
  sudo apt-get install graphviz
  See https://graphviz.org/download/
EOF
        exit 127
    fi
fi

DEPGRAPH_ARGS=()
if [[ "${MODE}" == "workspace-only" ]]; then
    DEPGRAPH_ARGS+=(--workspace-only)
fi

OUTPUT_ABS="${OUTPUT}"
case "${OUTPUT}" in
    /*) ;;
    *)  OUTPUT_ABS="${ROOT_DIR}/${OUTPUT}" ;;
esac
OUTPUT_DIR="$(dirname "${OUTPUT_ABS}")"

echo "==> mode: ${MODE}"
echo "==> output: ${OUTPUT_ABS}"
echo "==> cargo depgraph ${DEPGRAPH_ARGS[*]:-} | dot -Tsvg -o ${OUTPUT_ABS}"

if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "==> dry-run: skipping execution"
    exit 0
fi

mkdir -p "${OUTPUT_DIR}"

( cd "${ROOT_DIR}" && cargo depgraph "${DEPGRAPH_ARGS[@]}" | dot -Tsvg -o "${OUTPUT_ABS}" )

echo "==> wrote ${OUTPUT_ABS}"
