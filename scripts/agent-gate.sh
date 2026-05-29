#!/usr/bin/env bash
# agent-gate.sh — Fast pre-handoff self-check an agent runs BEFORE handing off.
#
# Runs, in order and halting on the FIRST failure:
#   1) cargo fmt --all --check
#   2) cargo clippy -p <crate> -- -D warnings   (per detected crate, or --workspace)
#   3) cargo check                              (same scope)
#
# Crate scope:
#   - Explicit crate names may be passed as args:
#         scripts/agent-gate.sh copypaste-core copypaste-daemon
#   - With no args, touched crates are detected from the branch diff against
#     main PLUS all uncommitted/untracked changes, mapped to `-p <name>`.
#   - If nothing is detected, falls back to the whole workspace (--workspace).
#
# NOTE: This gate deliberately does NOT run the test suite. Tests are owned by a
# separate single test runner — do not add cargo test / nextest here.
set -euo pipefail

# --- helpers ----------------------------------------------------------------

step_ok()  { printf '\xe2\x9c\x93 %s\n' "$1"; }   # ✓ <step>
step_bad() { printf '\xe2\x9c\x97 %s\n' "$1"; }   # ✗ <step>

# fail <step-name> — print failure marker, final line, and exit non-zero.
fail() {
    step_bad "$1"
    echo "GATE FAILED: $1"
    exit 1
}

# --- determine crate scope --------------------------------------------------

# Collect crate-scoping args for cargo: either "-p name -p name…" or "--workspace".
SCOPE_ARGS=()
SCOPE_DESC=""

if [[ $# -gt 0 ]]; then
    # Explicit crate list from CLI args.
    for crate in "$@"; do
        SCOPE_ARGS+=("-p" "$crate")
    done
    SCOPE_DESC="crates: $*"
else
    # Detect touched crates from branch diff + working-tree changes.
    MERGE_BASE=""
    if MERGE_BASE="$(git merge-base HEAD main 2>/dev/null)"; then
        :
    else
        MERGE_BASE=""
    fi

    CHANGED_PATHS=""
    {
        if [[ -n "$MERGE_BASE" ]]; then
            git diff --name-only "$MERGE_BASE"...HEAD 2>/dev/null || true
        fi
        git diff --name-only 2>/dev/null || true            # unstaged
        git diff --name-only --cached 2>/dev/null || true   # staged
        git ls-files --others --exclude-standard 2>/dev/null || true  # untracked
    } >/tmp/.agent-gate-paths.$$ 2>/dev/null || true
    CHANGED_PATHS="$(cat /tmp/.agent-gate-paths.$$ 2>/dev/null || true)"
    rm -f /tmp/.agent-gate-paths.$$ 2>/dev/null || true

    # Map crates/<name>/... -> <name>, dedup preserving uniqueness.
    DETECTED=()
    while IFS= read -r path; do
        [[ -z "$path" ]] && continue
        if [[ "$path" =~ ^crates/([^/]+)/ ]]; then
            name="${BASH_REMATCH[1]}"
            already=0
            for d in "${DETECTED[@]:-}"; do
                [[ "$d" == "$name" ]] && already=1 && break
            done
            [[ $already -eq 0 ]] && DETECTED+=("$name")
        fi
    done <<< "$CHANGED_PATHS"

    if [[ ${#DETECTED[@]} -gt 0 ]]; then
        for crate in "${DETECTED[@]}"; do
            SCOPE_ARGS+=("-p" "$crate")
        done
        SCOPE_DESC="detected crates: ${DETECTED[*]}"
    else
        SCOPE_ARGS=("--workspace")
        SCOPE_DESC="whole workspace (no changes detected)"
    fi
fi

echo "agent-gate: scope = ${SCOPE_DESC}"
echo

# --- step 1: fmt ------------------------------------------------------------

FMT_STEP="cargo fmt --all --check"
if cargo fmt --all --check; then
    step_ok "$FMT_STEP"
else
    fail "$FMT_STEP"
fi

# --- step 2: clippy ---------------------------------------------------------

CLIPPY_STEP="cargo clippy ${SCOPE_ARGS[*]} -- -D warnings"
if cargo clippy "${SCOPE_ARGS[@]}" -- -D warnings; then
    step_ok "$CLIPPY_STEP"
else
    fail "$CLIPPY_STEP"
fi

# --- step 3: check ----------------------------------------------------------

CHECK_STEP="cargo check ${SCOPE_ARGS[*]}"
if cargo check "${SCOPE_ARGS[@]}"; then
    step_ok "$CHECK_STEP"
else
    fail "$CHECK_STEP"
fi

# --- done -------------------------------------------------------------------

echo
echo "GATE GREEN"
exit 0
