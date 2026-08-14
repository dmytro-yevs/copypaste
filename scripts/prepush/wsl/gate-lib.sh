#!/usr/bin/env bash
# Shared plumbing for the WSL gate scripts, and the reason they have any.
#
# Each of these scripts used to open with `cd "$HOME/copypaste"`. That is the
# coordinator's verification checkout, so a worker that ran one gated somebody
# else's tree and reported the result for its own branch. The tree is now an
# argument with no default: a missing one is an error, never ~/copypaste.

COPYPASTE_GATE_STATUS=0
COPYPASTE_GATE_RESULTS=()

copypaste_gate_tree() {
    local caller="${COPYPASTE_GATE_NAME:-gate}"
    local tree="${1:-${COPYPASTE_TREE:-}}"
    if [ -z "$tree" ]; then
        printf '%s\n' \
            "$caller: refusing to run without an explicit tree." \
            "" \
            "    $caller <checkout>" \
            "    COPYPASTE_TREE=<checkout> $caller" \
            "" \
            "There is no default. Gating the wrong tree is indistinguishable" \
            "from gating yours, and green on somebody else's branch is worse" \
            "than no result at all." >&2
        return 2
    fi
    if [ ! -d "$tree" ]; then
        echo "$caller: $tree is not a directory" >&2
        return 2
    fi
    tree="$(cd "$tree" && pwd -P)" || return 2
    local marker
    for marker in Cargo.toml crates scripts; do
        if [ ! -e "$tree/$marker" ]; then
            echo "$caller: $tree has no $marker, so it is not a CopyPaste checkout" >&2
            return 2
        fi
    done
    if ! git -C "$tree" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        echo "$caller: $tree is not inside a git work tree" >&2
        return 2
    fi
    printf '%s\n' "$tree"
}

copypaste_gate_banner() {
    local tree="$1"
    echo "gate:    ${COPYPASTE_GATE_NAME:-gate}"
    echo "tree:    $tree"
    echo "branch:  $(git -C "$tree" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
    echo "head:    $(git -C "$tree" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    echo "dirty:   $(git -C "$tree" status --porcelain 2>/dev/null | wc -l) path(s)"
    # Printed because a CARGO_TARGET_DIR inherited from elsewhere is the other
    # way a run silently stops being about this tree.
    echo "target:  ${CARGO_TARGET_DIR:-$tree/target}"
}

copypaste_gate_run() {
    local name="$1"
    shift
    echo ""
    echo "################ $name"
    if "$@"; then
        COPYPASTE_GATE_RESULTS+=("PASS  $name")
    else
        COPYPASTE_GATE_RESULTS+=("FAIL  $name")
        COPYPASTE_GATE_STATUS=1
    fi
}

copypaste_gate_summary() {
    echo ""
    echo "================ SUMMARY  ${COPYPASTE_GATE_NAME:-gate}  ${1:-}"
    printf '%s\n' "${COPYPASTE_GATE_RESULTS[@]}"
    return "$COPYPASTE_GATE_STATUS"
}
