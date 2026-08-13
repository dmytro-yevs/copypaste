#!/usr/bin/env bash
# Decide the file-size budget from scripts/check-file-size.sh's report.
#
# The checker is advisory by design: it prints overages and exits 0 whatever it
# finds. This gate is the part that fails, and it separates two outcomes the
# inline CI step could not tell apart because `|| true` discarded the status —
# a report that lists overages, and no report at all. A checker that died
# mid-run produced an empty report, matched no overage, and passed.
#
# --self-test mutates the checker (exit 37, a truncated report, an overage, an
# exempt overage) and asserts each one lands on its own outcome.
set -uo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SELF="$SELF_DIR/$(basename "${BASH_SOURCE[0]}")"

TARGET=500
CHECKER="$SELF_DIR/check-file-size.sh"

# Paths relative to crates/, one per line. A file belongs here only once its
# module header names what was considered for extraction and why splitting it
# would make the code worse (AGENTS.md rule 5). Empty on purpose.
EXEMPT=""

gate() {
    local out status fail=0 file lines
    out="$("$CHECKER" "$TARGET" 2>&1)"
    status=$?
    printf '%s\n' "$out"

    if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
        {
            echo '## File-size budget (CLAUDE.md rule 5)'
            echo
            echo '```'
            printf '%s\n' "$out"
            echo '```'
        } >> "$GITHUB_STEP_SUMMARY"
    fi

    if [[ "$status" -ne 0 ]]; then
        printf '::error::%s exited %d; the file-size budget was not measured.\n' \
            "${CHECKER##*/}" "$status"
        return 2
    fi
    # Exit 0 is not proof of a complete run: a checker killed after its header
    # still reports nothing to enforce against.
    if ! printf '%s\n' "$out" | grep -qE '^(All files within|[0-9]+ file\(s\) over)'; then
        printf '::error::%s exited 0 with no summary line; its report is incomplete.\n' \
            "${CHECKER##*/}"
        return 2
    fi

    while read -r file lines; do
        [[ -n "$file" ]] || continue
        if printf '%s\n' "$EXEMPT" | grep -qxF "$file"; then
            printf '::notice file=crates/%s::%s source lines, exempt (rule 5)\n' "$file" "$lines"
            continue
        fi
        printf '::error file=crates/%s::%s source lines, over the %d-line budget (CLAUDE.md rule 5). Split it, or claim the indivisible-responsibility exemption in the module header and add it to EXEMPT in %s.\n' \
            "$file" "$lines" "$TARGET" "${SELF##*/}"
        fail=1
    done < <(printf '%s\n' "$out" | awk '/^[^ ]+\.rs[[:space:]]+[0-9]+$/ { print $1, $2 }')

    return "$fail"
}

self_test() {
    local pass=0 bad=0
    # Deliberately not local: the EXIT trap runs after this function returns.
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/scripts"

    checker() {
        cat > "$tmp/scripts/check-file-size.sh"
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
#!/usr/bin/env bash
echo "simulated internal failure" >&2
exit 37
EOF
    assert "a checker that exits 37 fails the gate" 2 "exited 37"

    checker <<'EOF'
#!/usr/bin/env bash
printf '%-52s %7s\n' "FILE" "SOURCE"
EOF
    assert "a report with no summary line fails the gate" 2 "report is incomplete"

    checker <<'EOF'
#!/usr/bin/env bash
printf '%-52s %7s\n' "FILE" "SOURCE"
echo
echo "All files within the 500-line budget."
EOF
    assert "a clean report passes" 0 "All files within"

    checker <<'EOF'
#!/usr/bin/env bash
printf '%-52s %7s\n' "FILE" "SOURCE"
printf '%-52s %7s\n' "daemon/src/huge.rs" "812"
echo
echo "1 file(s) over the 500-line budget (CLAUDE.md rule 5)."
EOF
    assert "an overage fails the gate" 1 "over the 500-line budget"

    # Same report, with the file registered as exempt.
    if ! install_gate "daemon/src/huge.rs"; then
        printf '  FAIL  the self-test could not register an exemption\n'
        bad=$((bad + 1))
    fi
    assert "a registered exemption passes" 0 "exempt (rule 5)"

    printf '  passed %d, failed %d\n' "$pass" "$bad"
    [[ "$bad" -eq 0 ]]
}

case "${1:-}" in
    --self-test) self_test ;;
    "")          gate ;;
    *)           printf 'usage: %s [--self-test]\n' "${SELF##*/}" >&2; exit 2 ;;
esac
