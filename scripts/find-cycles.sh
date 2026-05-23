#!/usr/bin/env bash
# find-cycles.sh — detect cyclic dependencies in the Cargo workspace via cargo-depgraph.
#
# Uses `cargo depgraph --workspace-only` DOT output and parses edges to detect
# directed cycles. Exits 1 if any cycle is found, 0 otherwise.
#
# Examples:
#   scripts/find-cycles.sh                # scan workspace for cycles
#   scripts/find-cycles.sh --full         # also include external dependencies
#   scripts/find-cycles.sh --dry-run      # print commands without invoking cargo
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

MODE="workspace-only"
DRY_RUN=0

usage() {
    cat <<EOF
${SCRIPT_NAME} — detect cyclic dependencies in the Cargo workspace.

Usage: ${SCRIPT_NAME} [options]

Options:
  --workspace-only   Only workspace crates (default).
  --full             Include external dependencies in cycle scan.
  --dry-run          Print commands without invoking cargo.
  -h, --help         Show this help and exit.

Exit codes:
  0   No cycles found.
  1   One or more cycles detected.
  127 cargo-depgraph not installed.

Requires:
  cargo-depgraph (https://crates.io/crates/cargo-depgraph)
    cargo install cargo-depgraph
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

DEPGRAPH_ARGS=()
if [[ "${MODE}" == "workspace-only" ]]; then
    DEPGRAPH_ARGS+=(--workspace-only)
fi

echo "==> cargo depgraph ${DEPGRAPH_ARGS[*]:-}"

if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "==> dry-run: skipping execution"
    exit 0
fi

if ! cargo depgraph --help >/dev/null 2>&1; then
    cat >&2 <<EOF
error: cargo-depgraph is not installed.

Install it with:
  cargo install cargo-depgraph
EOF
    exit 127
fi

DOT_OUTPUT="$(cd "${ROOT_DIR}" && cargo depgraph "${DEPGRAPH_ARGS[@]}")"

# Extract (id, label) for nodes and (from, to) for edges using portable
# grep/sed (works on BSD and GNU). DOT lines look like:
#   0 [ label="copypaste-core" ... ]
#   0 -> 1 [ ... ]
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

NODES_FILE="${TMP_DIR}/nodes.tsv"
EDGES_FILE="${TMP_DIR}/edges.tsv"

# Node label lines: capture id and label.
printf '%s\n' "${DOT_OUTPUT}" \
    | grep -E '^[[:space:]]*[0-9]+[[:space:]]*\[' \
    | sed -E 's/^[[:space:]]*([0-9]+)[[:space:]]*\[[^"]*label="([^"]*)".*/\1	\2/' \
    | grep -E '^[0-9]+	' > "${NODES_FILE}" || true

# Edge lines: capture from-id and to-id.
printf '%s\n' "${DOT_OUTPUT}" \
    | grep -E '[0-9]+[[:space:]]*->[[:space:]]*[0-9]+' \
    | sed -E 's/^[[:space:]]*([0-9]+)[[:space:]]*->[[:space:]]*([0-9]+).*/\1	\2/' \
    | grep -E '^[0-9]+	[0-9]+$' > "${EDGES_FILE}" || true

# Portable awk: iterative DFS over the parsed edges. White=0, Gray=1, Black=2.
CYCLE_RESULT="$(awk -F'\t' '
    FNR == NR {
        labels[$1] = $2
        next
    }
    {
        from = $1; to = $2
        edges[from] = edges[from] " " to
        nodes[from] = 1
        nodes[to] = 1
    }
    END {
        cycle_found = 0
        for (n in nodes) {
            if (color[n] == 2) continue
            top = 0
            stack[top] = n "|0"
            color[n] = 1
            path_top = 0
            path[0] = n
            while (top >= 0) {
                split(stack[top], parts, "|")
                cur = parts[1]
                idx = parts[2] + 0
                ec = ""
                if (cur in edges) ec = edges[cur]
                nch = split(ec, ch, " ")
                advanced = 0
                while (idx <= nch) {
                    c = ch[idx]
                    idx++
                    if (c == "") continue
                    if (color[c] == 1) {
                        start = -1
                        for (i = 0; i <= path_top; i++) {
                            if (path[i] == c) { start = i; break }
                        }
                        if (start < 0) start = path_top
                        cyc = ""
                        for (i = start; i <= path_top; i++) {
                            lab = (path[i] in labels) ? labels[path[i]] : path[i]
                            cyc = cyc lab " -> "
                        }
                        lab = (c in labels) ? labels[c] : c
                        cyc = cyc lab
                        print "CYCLE: " cyc
                        cycle_found = 1
                        idx = nch + 1
                        break
                    }
                    if (color[c] == 0) {
                        stack[top] = cur "|" idx
                        top++
                        stack[top] = c "|0"
                        color[c] = 1
                        path_top++
                        path[path_top] = c
                        advanced = 1
                        break
                    }
                }
                if (!advanced) {
                    color[cur] = 2
                    if (path_top >= 0 && path[path_top] == cur) {
                        delete path[path_top]
                        path_top--
                    }
                    top--
                }
            }
        }
        if (cycle_found) exit 1
        exit 0
    }
' "${NODES_FILE}" "${EDGES_FILE}")"
STATUS=$?

if [[ -n "${CYCLE_RESULT}" ]]; then
    printf '%s\n' "${CYCLE_RESULT}"
fi

if [[ ${STATUS} -ne 0 ]]; then
    echo "==> cyclic dependencies detected" >&2
    exit 1
fi

echo "==> no cyclic dependencies found"
exit 0
