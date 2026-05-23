#!/usr/bin/env bash
# Copy version-controlled fuzz seeds (fuzz/seeds/) into the live fuzz
# corpus directory (fuzz/corpus/) before invoking `cargo fuzz run`.
#
# fuzz/corpus/ is in .gitignore — it accumulates libFuzzer-generated inputs
# and crash reproducers at runtime. fuzz/seeds/ holds the hand-crafted
# initial corpus that gives the fuzzer meaningful starting coverage.
#
# Usage:
#   scripts/fuzz-seed.sh                    # seed all targets
#   scripts/fuzz-seed.sh ipc_protocol_parse # seed a single target
#
# Idempotent: re-running just re-copies; existing libFuzzer-discovered
# inputs in fuzz/corpus/<target>/ are preserved (cp -n would skip them
# but we use cp -f for seeds since they are the source of truth).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SEEDS_DIR="$REPO_ROOT/fuzz/seeds"
CORPUS_DIR="$REPO_ROOT/fuzz/corpus"

if [[ ! -d "$SEEDS_DIR" ]]; then
    echo "error: $SEEDS_DIR does not exist" >&2
    exit 1
fi

targets=()
if [[ $# -gt 0 ]]; then
    targets=("$@")
else
    while IFS= read -r dir; do
        targets+=("$(basename "$dir")")
    done < <(find "$SEEDS_DIR" -mindepth 1 -maxdepth 1 -type d | sort)
fi

if [[ ${#targets[@]} -eq 0 ]]; then
    echo "error: no targets found under $SEEDS_DIR" >&2
    exit 1
fi

for target in "${targets[@]}"; do
    src="$SEEDS_DIR/$target"
    dst="$CORPUS_DIR/$target"

    if [[ ! -d "$src" ]]; then
        echo "warn: skipping $target (no seeds dir at $src)" >&2
        continue
    fi

    mkdir -p "$dst"
    count=0
    while IFS= read -r seed; do
        cp -f "$seed" "$dst/"
        count=$((count + 1))
    done < <(find "$src" -mindepth 1 -maxdepth 1 -type f)

    # Per-target generated (non-versioned) seeds. Built on demand so they
    # don't bloat the repo. The oversized IPC payload is ~16 MB and would
    # never round-trip through review if committed.
    case "$target" in
        ipc_protocol_parse)
            python3 - "$dst/seed_gen_oversized_field.json" <<'PY'
import json, sys
out = sys.argv[1]
big = "A" * (16 * 1024 * 1024 + 32)
payload = {"id": 7, "method": "insert",
           "params": {"content_type": "text", "content": big},
           "protocol_version": 1}
with open(out, "w") as f:
    json.dump(payload, f)
PY
            count=$((count + 1))
            ;;
    esac

    echo "seeded $target: $count file(s) -> $dst"
done
