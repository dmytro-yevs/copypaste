#!/usr/bin/env bash
# Report source files over the CLAUDE.md rule 5 budget.
#
# Counts source lines only: everything before the `#[cfg(test)]` module.
# A file may be 500 lines of code and 900 of tests and still be within budget.
#
# "The module", not "the first `#[cfg(test)]`". `daemon/src/main.rs` declares
# `#[cfg(test)] mod testutil;` at line 39 and its tests begin at 730, so cutting
# at the first attribute reported a 729-line file as 38 and this check passed it
# 229 lines over budget. A declaration ends in `;`; the module opens a block.
#
# Covers the frontend too. It did not until a review found `lib/ipc.ts` at 499
# lines reading as a file at its limit when 154 of them are comment — the rule
# had never been measured outside Rust, so "it passes" said nothing about it.
# `.test.` and `.spec.` files are excluded the way Rust test modules are.
#
# Advisory, not a gate. v1 rejected a hard CI line-count check because it
# forces artificial splits; the number is here to make the backlog visible.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET=${1:-500}
over=0

printf '%-52s %7s\n' "FILE" "SOURCE"
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
        printf '%-52s %7s\n' "${f#crates/}" "$src"
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

echo
if [ "$over" -eq 0 ]; then
    echo "All files within the ${TARGET}-line budget."
else
    echo "$over file(s) over the ${TARGET}-line budget (CLAUDE.md rule 5)."
fi
