#!/usr/bin/env bash
# Decide the file-size budget from scripts/check-file-size.sh's report.
#
# The checker is advisory by design: it prints overages and exits 0 whatever it
# finds. This gate is the part that fails, and it separates two outcomes the
# inline CI step could not tell apart because `|| true` discarded the status —
# a report that lists overages, and no report at all. A checker that died
# mid-run produced an empty report, matched no overage, and passed.
#
# It reads `--porcelain`, not the human table. Re-deriving the rows from the
# table cost four false greens at once: the regex knew `.rs` only, so `.ts` and
# `.tsx` overages passed; a Rust path containing a space passed; and a summary
# declaring overages with no row the gate recognised passed as well. Anything
# the porcelain parse cannot account for — an unknown record, a count that does
# not match the rows, a report with no `end` — fails the gate.
#
# --self-test runs the gate against a fake checker for each of those outcomes.
set -uo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SELF="$SELF_DIR/$(basename "${BASH_SOURCE[0]}")"

TARGET=500
REPORT_VERSION=1
CHECKER="$SELF_DIR/check-file-size.sh"

# Repository-relative paths, one per line. A file belongs here only once its
# module header names what was considered for extraction and why splitting it
# would make the code worse (AGENTS.md rule 5). Empty on purpose.
EXEMPT=""

reject() {
    printf '::error::%s\n' "$*"
    exit 2
}

is_uint() {
    [[ "$1" =~ ^(0|[1-9][0-9]{0,8})$ ]]
}

# Per actions/toolkit escapeProperty: %, CR, LF, colon, comma.
escape_property() {
    local v="$1"
    v="${v//%/%25}"
    v="${v//$'\r'/%0D}"
    v="${v//$'\n'/%0A}"
    v="${v//:/%3A}"
    v="${v//,/%2C}"
    printf '%s' "$v"
}

# Fills OVER_PATHS/OVER_LINES, or exits 2. Every record is accounted for: the
# gate must never treat a row it failed to understand as a row that was clean.
# Record sequence: header → over* → count → end.
read_report() {
    local raw kind lines path extra state=init declared="" seen=0
    OVER_PATHS=()
    OVER_LINES=()
    OVER_COUNT=0

    [[ -n "$1" ]] ||
        reject "${CHECKER##*/} printed nothing; the file-size budget was not measured."

    while IFS= read -r raw; do
        # IFS=$'\t' read strips leading tabs, collapses doubled tabs, and
        # discards trailing empty fields — all valid-looking to the parser.
        case "$raw" in
            $'\t'*|*$'\t'|*$'\t\t'*) reject "${CHECKER##*/} emitted a record with malformed tab fields." ;;
        esac
        IFS=$'\t' read -r kind lines path extra <<< "$raw"
        case "$state" in
            ended) reject "${CHECKER##*/} emitted '$kind' after its end record." ;;
        esac
        case "$kind" in
            file-size)
                [[ "$state" == "init" && "$lines" == "$REPORT_VERSION" && "$path" == "$TARGET" && -z "$extra" ]] ||
                    reject "${CHECKER##*/} report header is not version $REPORT_VERSION for $TARGET lines: '$kind $lines $path'."
                state=rows
                ;;
            over)
                [[ "$state" == "rows" ]] ||
                    reject "${CHECKER##*/} emitted an overage record out of sequence."
                is_uint "$lines" && [[ -n "$path" && -z "$extra" ]] ||
                    reject "${CHECKER##*/} emitted an unreadable overage record: '$kind $lines $path $extra'."
                [[ "$lines" -gt "$TARGET" ]] ||
                    reject "${CHECKER##*/} reported $lines source lines for $path, not over the $TARGET-line budget."
                OVER_LINES[$seen]="$lines"
                OVER_PATHS[$seen]="$path"
                seen=$((seen + 1))
                ;;
            count)
                [[ "$state" == "rows" ]] ||
                    reject "${CHECKER##*/} emitted a count record out of sequence."
                is_uint "$lines" && [[ -z "$path$extra" ]] ||
                    reject "${CHECKER##*/} emitted an unreadable count record: '$kind $lines $path'."
                declared="$lines"
                state=counted
                ;;
            end)
                [[ "$state" == "counted" ]] ||
                    reject "${CHECKER##*/} emitted an end record without a preceding count."
                [[ -z "$lines$path$extra" ]] || reject "${CHECKER##*/} emitted a malformed end record."
                state=ended
                ;;
            *)
                reject "${CHECKER##*/} emitted a record the gate does not recognise: '$kind'."
                ;;
        esac
    done <<< "$1"

    [[ "$state" == "ended" ]] ||
        reject "${CHECKER##*/} report is incomplete; it stopped before its end record."
    [[ "$declared" -eq "$seen" ]] ||
        reject "${CHECKER##*/} declared $declared overage(s) and emitted $seen; its report is inconsistent."
    OVER_COUNT="$seen"
}

render() {
    local i
    printf '%-52s %7s\n' "FILE" "SOURCE"
    for (( i = 0; i < OVER_COUNT; i++ )); do
        printf '%-52s %7s\n' "${OVER_PATHS[$i]}" "${OVER_LINES[$i]}"
    done
    echo
    if [[ "$OVER_COUNT" -eq 0 ]]; then
        printf 'All files within the %d-line budget.\n' "$TARGET"
    else
        printf '%d file(s) over the %d-line budget (AGENTS.md rule 5).\n' \
            "$OVER_COUNT" "$TARGET"
    fi
}

gate() {
    local out status report fail=0 i path lines
    out="$("$CHECKER" --porcelain "$TARGET" 2>&1)"
    status=$?
    if [[ "$status" -ne 0 ]]; then
        printf '%s\n' "$out"
        reject "${CHECKER##*/} exited $status; the file-size budget was not measured."
    fi

    read_report "$out"
    report="$(render)"
    printf '%s\n' "$report"

    if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
        {
            echo '## File-size budget (AGENTS.md rule 5)'
            echo
            echo '```'
            printf '%s\n' "$report"
            echo '```'
        } >> "$GITHUB_STEP_SUMMARY"
    fi

    for (( i = 0; i < OVER_COUNT; i++ )); do
        path="${OVER_PATHS[$i]}"
        lines="${OVER_LINES[$i]}"
        escaped="$(escape_property "$path")"
        if printf '%s\n' "$EXEMPT" | grep -qxF "$path"; then
            printf '::notice file=%s::%s source lines, exempt (rule 5)\n' "$escaped" "$lines"
            continue
        fi
        printf '::error file=%s::%s source lines, over the %d-line budget (AGENTS.md rule 5). Split it, or claim the indivisible-responsibility exemption in the module header and add it to EXEMPT in %s.\n' \
            "$escaped" "$lines" "$TARGET" "${SELF##*/}"
        fail=1
    done

    return "$fail"
}

self_test() {
    local pass=0 bad=0
    # Deliberately not local: the EXIT trap runs after this function returns.
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/scripts"

    checker() {
        {
            echo '#!/usr/bin/env bash'
            cat
        } > "$tmp/scripts/check-file-size.sh"
        chmod +x "$tmp/scripts/check-file-size.sh"
    }
    # No in-place edit: BSD sed rejects GNU `sed -i 's|x|y|' file`, and the
    # release checker runs this self-test on macOS.
    install_gate() {
        awk -v exempt="${1:-}" '
            index($0, "case \"${1:-}\" in") == 1 && !hit {
                printf "EXEMPT=\"%s\"\n", exempt
                hit = 1
            }
            { print }
            END { if (!hit) exit 1 }
        ' "$SELF" > "$tmp/scripts/gate.sh" || return 1
        chmod +x "$tmp/scripts/gate.sh"
    }
    assert() {
        local desc="$1" want="$2" needle="$3" out got
        out="$(cd "$tmp" && GITHUB_STEP_SUMMARY='' ./scripts/gate.sh 2>&1)"
        got=$?
        if [[ "$got" == "$want" ]] && printf '%s' "$out" | grep -qF "$needle"; then
            printf '  ok    %s\n' "$desc"
            pass=$((pass + 1))
        else
            printf '  FAIL  %s\n        wanted status %s and %s, got status %s:\n%s\n' \
                "$desc" "$want" "$needle" "$got" "$out"
            bad=$((bad + 1))
        fi
    }

    printf '\n== file-size gate self-test\n'

    if ! install_gate; then
        printf '  FAIL  the self-test could not build a gate copy from %s\n' "${SELF##*/}"
        printf '  passed 0, failed 1\n'
        return 1
    fi

    checker <<'EOF'
echo "simulated internal failure" >&2
exit 37
EOF
    assert "a checker that exits 37 fails the gate" 2 "exited 37"

    checker <<'EOF'
printf 'file-size\t1\t500\n'
EOF
    assert "a report that stops after its header fails the gate" 2 "stopped before its end record"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\t812\tcrates/daemon/src/huge.rs\n'
EOF
    assert "a report truncated mid-overage fails the gate" 2 "stopped before its end record"

    checker <<'EOF'
printf 'file-size\t1\t500\ncount\t0\nend\n'
EOF
    assert "a clean report passes" 0 "All files within"

    for fixture in "rs:crates/daemon/src/huge.rs" "ts:crates/copypaste-ui/src/lib/ipc.ts" \
                   "tsx:crates/copypaste-ui/src/routes/History.tsx" "spaced:crates/daemon/src/has space.rs"; do
        checker <<EOF
printf 'file-size\t1\t500\nover\t812\t${fixture#*:}\ncount\t1\nend\n'
EOF
        assert "an overage in ${fixture%%:*} fails the gate" 1 "${fixture#*:}"
    done

    checker <<'EOF'
printf 'file-size\t1\t500\ncount\t1\nend\n'
EOF
    assert "a declared overage with no row fails the gate" 2 "declared 1 overage(s) and emitted 0"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\t812\tcrates/daemon/src/huge.rs\ncount\t2\nend\n'
EOF
    assert "a count that overstates the rows fails the gate" 2 "declared 2 overage(s) and emitted 1"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\tmany\tcrates/daemon/src/huge.rs\ncount\t1\nend\n'
EOF
    assert "an overage without a line count fails the gate" 2 "unreadable overage record"

    checker <<'EOF'
printf 'file-size\t1\t500\n1 file(s) over the 500-line budget.\ncount\t0\nend\n'
EOF
    assert "a record the gate cannot read fails the gate" 2 "does not recognise"

    checker <<'EOF'
printf 'file-size\t1\t300\ncount\t0\nend\n'
EOF
    assert "a report measured against another budget fails the gate" 2 "report header is not version"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\t812\tcrates/daemon/src/huge.rs\ncount\t1\n'
EOF
    assert "a report missing only its end record fails" 2 "stopped before its end record"

    checker <<'EOF'
[[ "$1" == "--porcelain" ]] || { echo "not --porcelain" >&2; exit 99; }
printf 'file-size\t1\t500\ncount\t0\nend\n'
EOF
    assert "the gate passes --porcelain to the checker" 0 "All files within"

    checker <<'EOF'
printf 'file-size\t1\t500\ncount\t18446744073709551616\nend\n'
EOF
    assert "an overflowing count fails the gate" 2 "unreadable count"

    checker <<'EOF'
printf 'file-size\t1\t500\ncount\t1\nover\t812\tcrates/daemon/src/huge.rs\nend\n'
EOF
    assert "an overage after the count fails the gate" 2 "out of sequence"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\t1\tcrates/daemon/src/huge.rs\ncount\t1\nend\n'
EOF
    assert "an overage not over the budget fails the gate" 2 "not over"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\t812\tcrates/ui/src/has,comma.ts\ncount\t1\nend\n'
EOF
    assert "a comma in a path is escaped in annotations" 1 "%2C"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\t812\tcrates/daemon/src/huge.rs\ncount\t1\nend\n'
EOF
    if ! install_gate "crates/daemon/src/huge.rs"; then
        printf '  FAIL  the self-test could not register an exemption\n'
        bad=$((bad + 1))
    fi
    assert "a registered exemption passes" 0 "exempt (rule 5)"

    checker <<'EOF'
printf 'file-size\t1\t500\ncount\t1\nover\t812\tcrates/daemon/src/huge.rs\nend\n'
EOF
    assert "an exempt overage after the count still fails" 2 "out of sequence"

    checker <<'EOF'
printf 'file-size\t1\t500\nover\t1\tcrates/daemon/src/huge.rs\ncount\t1\nend\n'
EOF
    assert "an exempt under-budget overage still fails" 2 "not over"

    checker <<'EOF'
printf '\tfile-size\t1\t500\ncount\t0\nend\n'
EOF
    assert "a leading tab on the header fails the gate" 2 "malformed tab"

    checker <<'EOF'
printf 'file-size\t\t1\t500\ncount\t0\nend\n'
EOF
    assert "a doubled tab in the header fails the gate" 2 "malformed tab"

    checker <<'EOF'
printf 'file-size\t1\t500\t\ncount\t0\nend\n'
EOF
    assert "a trailing tab on file-size fails the gate" 2 "malformed tab"

    checker <<'EOF'
printf 'file-size\t1\t500\ncount\t0\t\nend\n'
EOF
    assert "a trailing tab on the count fails the gate" 2 "malformed tab"

    checker <<'EOF'
printf 'file-size\t1\t500\ncount\t0\nend\t\n'
EOF
    assert "a trailing tab on the end fails the gate" 2 "malformed tab"

    printf '  passed %d, failed %d\n' "$pass" "$bad"
    [[ "$bad" -eq 0 ]]
}

case "${1:-}" in
    --self-test) self_test ;;
    "")          gate ;;
    *)           printf 'usage: %s [--self-test]\n' "${SELF##*/}" >&2; exit 2 ;;
esac
