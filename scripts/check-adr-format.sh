#!/usr/bin/env bash
# check-adr-format.sh — Lint Architecture Decision Records.
#
# Validates filename convention, H1 line, required sections, numbering gaps,
# and duplicate numbers in docs/adr/.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/check-adr-format.sh [--fix] [--dry-run] [--help]

Options:
  --fix       Append missing required sections (skeleton only) to violating
              ADRs. Existing content is never rewritten.
  --dry-run   Same as default mode (report only). Provided for symmetry with
              other repo scripts that document violations without changing
              files.
  --help, -h  Show this message.

Checks performed:
  1. Filename matches  ADR-NNN-kebab-case.md   (legacy NNN-slug.md tolerated
     with a warning; see docs/adr/README.md).
  2. H1 line matches   # ADR-NNN: <Title>      with NNN equal to filename.
  3. Required sections present: Status, Context, Decision, Consequences.
  4. No duplicate ADR numbers across the directory.
  5. No gaps in the ADR number sequence (warning only).

Exit codes:
  0 — no violations
  1 — violations found
  2 — usage error
EOF
}

ADR_DIR="docs/adr"
FIX_MODE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --fix)     FIX_MODE=1; shift ;;
        --dry-run) shift ;;
        --help|-h) usage; exit 0 ;;
        *)         echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ ! -d "$ADR_DIR" ]]; then
    echo "ERROR: $ADR_DIR not found (run from repo root)" >&2
    exit 2
fi

REQUIRED_SECTIONS=("Status" "Context" "Decision" "Consequences")
VIOLATIONS=0
WARNINGS=0
NUMBERS=()
# Parallel arrays for duplicate detection (bash 3.2 compatible — no -A).
SEEN_NUMS=()
SEEN_FILES=()

seen_lookup() {
    # $1 = number; echoes the first filename seen for that number, or empty.
    local target="$1" i
    for ((i = 0; i < ${#SEEN_NUMS[@]}; i++)); do
        if [[ "${SEEN_NUMS[$i]}" == "$target" ]]; then
            printf '%s' "${SEEN_FILES[$i]}"
            return 0
        fi
    done
    return 1
}

report() {
    # $1 = severity (ERROR|WARN), $2 = file, $3 = message
    printf '  [%s] %s: %s\n' "$1" "$2" "$3"
    if [[ "$1" == "ERROR" ]]; then
        VIOLATIONS=$((VIOLATIONS + 1))
    else
        WARNINGS=$((WARNINGS + 1))
    fi
}

append_missing_sections() {
    local file="$1"
    shift
    {
        printf '\n'
        for section in "$@"; do
            printf '## %s\n\n<!-- TODO: fill in -->\n\n' "$section"
        done
    } >> "$file"
}

shopt -s nullglob
# Collect candidate files. Match both new ADR-NNN-*.md and legacy NNN-*.md
# while explicitly excluding README.md and ADR-TEMPLATE.md.
declare -a FILES=()
for f in "$ADR_DIR"/ADR-*.md "$ADR_DIR"/[0-9][0-9][0-9]-*.md; do
    base="$(basename "$f")"
    [[ "$base" == "README.md" || "$base" == "ADR-TEMPLATE.md" ]] && continue
    FILES+=("$f")
done
shopt -u nullglob

if [[ ${#FILES[@]} -eq 0 ]]; then
    echo "No ADR files found in $ADR_DIR (expected ADR-NNN-*.md)."
    exit 0
fi

echo "Linting ${#FILES[@]} ADR file(s) in $ADR_DIR/"
echo

for file in "${FILES[@]}"; do
    base="$(basename "$file")"
    num=""
    legacy=0

    if [[ "$base" =~ ^ADR-([0-9]{3})-[a-z0-9]+(-[a-z0-9]+)*\.md$ ]]; then
        num="${BASH_REMATCH[1]}"
    elif [[ "$base" =~ ^([0-9]{3})-[a-z0-9]+(-[a-z0-9]+)*\.md$ ]]; then
        num="${BASH_REMATCH[1]}"
        legacy=1
        report "WARN" "$base" "legacy filename; prefer ADR-${num}-<slug>.md"
    else
        report "ERROR" "$base" "filename does not match ADR-NNN-kebab-case.md"
        continue
    fi

    # Duplicate number check
    if prev_file="$(seen_lookup "$num")"; then
        report "ERROR" "$base" "duplicate ADR number $num (also in $prev_file)"
    else
        SEEN_NUMS+=("$num")
        SEEN_FILES+=("$base")
        NUMBERS+=("$num")
    fi

    # H1 check
    h1="$(head -n 1 "$file" || true)"
    expected_h1_prefix="# ADR-${num}:"
    if [[ "$h1" != "$expected_h1_prefix"* ]]; then
        report "ERROR" "$base" "H1 must start with '$expected_h1_prefix' (got: '${h1:0:60}')"
    fi

    # Required sections
    missing=()
    for section in "${REQUIRED_SECTIONS[@]}"; do
        # Accept either "## Section" or "## Section — extra" / "## Section -- extra"
        if ! grep -qE "^##[[:space:]]+${section}([[:space:]]|$|[—-])" "$file"; then
            missing+=("$section")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        report "ERROR" "$base" "missing section(s): ${missing[*]}"
        if [[ $FIX_MODE -eq 1 ]]; then
            append_missing_sections "$file" "${missing[@]}"
            echo "    -> appended skeleton for: ${missing[*]}"
        fi
    fi

    # Silence shellcheck about $legacy being assigned-but-unused; keep for future
    : "$legacy"
done

# Gap detection (warning only)
if [[ ${#NUMBERS[@]} -gt 0 ]]; then
    sorted=()
    while IFS= read -r line; do sorted+=("$line"); done < <(printf '%s\n' "${NUMBERS[@]}" | sort -u)
    prev=0
    for n in "${sorted[@]}"; do
        cur=$((10#$n))
        if [[ $prev -ne 0 && $cur -gt $((prev + 1)) ]]; then
            missing_range=""
            for ((i = prev + 1; i < cur; i++)); do
                missing_range+="$(printf '%03d ' "$i")"
            done
            report "WARN" "(sequence)" "gap in numbering: missing ${missing_range%% }"
        fi
        prev=$cur
    done
fi

echo
echo "Summary: $VIOLATIONS error(s), $WARNINGS warning(s)"

if [[ $VIOLATIONS -gt 0 ]]; then
    exit 1
fi
exit 0
