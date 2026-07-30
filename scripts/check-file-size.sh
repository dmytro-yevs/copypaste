#!/usr/bin/env bash
# Report source files over the CLAUDE.md rule 5 budget.
#
# Counts source lines only: everything before the first `#[cfg(test)]` module.
# A file may be 500 lines of code and 900 of tests and still be within budget.
#
# Advisory, not a gate. v1 rejected a hard CI line-count check because it
# forces artificial splits; the number is here to make the backlog visible.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

TARGET=${1:-500}
over=0

printf '%-52s %7s\n' "FILE" "SOURCE"
while read -r f; do
    total=$(wc -l < "$f")
    tl=$(grep -n '^#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1 || true)
    src=${tl:+$((tl - 1))}
    src=${src:-$total}
    if [ "$src" -gt "$TARGET" ]; then
        printf '%-52s %7s\n' "${f#crates/}" "$src"
        over=$((over + 1))
    fi
done < <(find crates -name '*.rs' -not -path '*/target/*' | sort)

echo
if [ "$over" -eq 0 ]; then
    echo "All files within the ${TARGET}-line budget."
else
    echo "$over file(s) over the ${TARGET}-line budget (CLAUDE.md rule 5)."
fi
