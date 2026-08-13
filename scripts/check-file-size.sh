#!/usr/bin/env bash
# Report source files over the AGENTS.md rule 5 budget.
#
# Counts source lines only: everything before the `#[cfg(test)]` module.
# A file may be 500 lines of code and 900 of tests and still be within budget.
#
# "The module", not "the first `#[cfg(test)]`". `daemon/src/main.rs` declares
# `#[cfg(test)] mod testutil;` at line 39 and its tests begin at 730, so cutting
# at the first attribute reported a 729-line file as 38 and this check passed it
# 229 lines over budget. A declaration ends in `;`; the module opens a block.
#
# Covers .rs, .ts, .tsx; excludes `.test.*`, `.spec.*`, `.d.ts`. Frontend was
# added after `lib/ipc.ts` at 499 lines read as within budget while 154 were
# comment — the rule had never been measured outside Rust.
#
# Advisory, not a gate. `--porcelain` emits tab-delimited records (header,
# over*, count, end) for check-file-size-gate.sh.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

PORCELAIN=0
TARGET=500
for arg in "$@"; do
    case "$arg" in
        --porcelain) PORCELAIN=1 ;;
        *)           TARGET=$arg ;;
    esac
done
over=0

if [ "$PORCELAIN" -eq 1 ]; then
    printf 'file-size\t1\t%s\n' "$TARGET"
else
    printf '%-52s %7s\n' "FILE" "SOURCE"
fi
while read -r f; do
    src=$(awk '
        /^#\[cfg\(test\)\]/ { pending = NR; next }
        pending && /^[[:space:]]*$/ { next }
        pending {
            if ($0 !~ /;[[:space:]]*$/) { print pending - 1; found = 1; exit }
            pending = 0
        }
        END { if (!found) print NR }
    ' "$f")
    if [ "$src" -gt "$TARGET" ]; then
        if [ "$PORCELAIN" -eq 1 ]; then
            printf 'over\t%s\t%s\n' "$src" "${f#crates/}"
        else
            printf '%-52s %7s\n' "${f#crates/}" "$src"
        fi
        over=$((over + 1))
    fi
done < <(find crates \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' \) \
             -not -path '*/target/*' \
             -not -path '*/node_modules/*' \
             -not -path '*/gen/*' \
             -not -path '*/dist/*' \
             -not -name '*.test.*' \
             -not -name '*.spec.*' \
             -not -name '*.d.ts' | sort)

if [ "$PORCELAIN" -eq 1 ]; then
    printf 'count\t%s\nend\n' "$over"
    exit 0
fi

echo
if [ "$over" -eq 0 ]; then
    echo "All files within the ${TARGET}-line budget."
else
    echo "$over file(s) over the ${TARGET}-line budget (AGENTS.md rule 5)."
fi
