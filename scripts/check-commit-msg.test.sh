#!/usr/bin/env bash
# Tests for check-commit-msg.sh. Run it directly; CI runs it too.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK="$HERE/check-commit-msg.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

PASS=0
FAIL=0

ok()    { PASS=$((PASS + 1)); printf '  ok    %s\n' "$1"; }
bad()   { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; }
group() { printf '\n== %s\n' "$1"; }

# run <accept|reject> <name> <file> [substring the output must contain]
run() {
    local want="$1" name="$2" file="$3" needle="${4:-}" out status got
    out="$(bash "$CHECK" "$file" 2>&1)"
    status=$?
    got=accept
    (( status != 0 )) && got=reject

    if [[ "$got" != "$want" ]]; then
        bad "$name" "wanted $want, got $got: ${out:-(no output)}"
    elif [[ -n "$needle" && "$out" != *"$needle"* ]]; then
        bad "$name" "rejected, but no diagnostic mentioning '$needle': ${out:-(no output)}"
    else
        ok "$name"
    fi
}

# Message on stdin, written with a final newline.
expect() { local w="$1" n="$2" d="${3:-}"; cat > "$TMP/msg"; run "$w" "$n" "$TMP/msg" "$d"; }

# Message on stdin, written with NO final newline -- the shape git writes
# .git/MERGE_MSG in, and the shape that used to be read as an empty message.
expect_unterminated() {
    local w="$1" n="$2" d="${3:-}"
    printf '%s' "$(cat)" > "$TMP/msg"
    run "$w" "$n" "$TMP/msg" "$d"
}

AT_LIMIT="$(printf '%68s' '' | tr ' ' x)"   # "Add " + 68 == 72
OVER_LIMIT="${AT_LIMIT}x"

# Dropping the last line reported a single-line message as empty, and silently
# skipped whatever a multi-line one said last.
group "An unterminated final line is still read"

expect_unterminated accept 'stock merge subject, unterminated as git writes it' <<'EOF'
Merge branch 'feature'
EOF

expect_unterminated reject 'banned content on an unterminated last line' 'CI' <<'EOF'
Add a retry bound to the relay dialer

All tests pass
EOF

expect_unterminated reject 'over-long subject, unterminated' 'max 72' <<EOF
Add $OVER_LIMIT
EOF

expect_unterminated accept 'ordinary message, unterminated' <<'EOF'
Add a retry bound to the relay dialer
EOF

group "Rule 10, unchanged"

expect accept 'subject only' <<'EOF'
Add a retry bound to the relay dialer
EOF

expect accept 'subject at exactly 72 chars' <<EOF
Add $AT_LIMIT
EOF

expect accept 'full shape' <<'EOF'
Fix inaccessible selected-row state

Problem:
Selected and hovered rows were visually indistinguishable.

Change:
- Add an accent edge to selected rows
- Update contrast checks
EOF

expect reject 'empty message' 'empty subject' < /dev/null

expect reject 'whitespace-only subject' 'empty subject' <<'EOF'

EOF

expect reject 'subject at 73 chars' 'max 72' <<EOF
Add $OVER_LIMIT
EOF

expect reject 'subject ends in a period' 'period' <<'EOF'
Add a retry bound to the relay dialer.
EOF

expect reject 'past-tense subject' 'past tense' <<'EOF'
Added a retry bound to the relay dialer
EOF

expect reject 'gerund subject' 'gerund' <<'EOF'
Adding a retry bound to the relay dialer
EOF

expect reject 'no blank line after the subject' 'blank line' <<'EOF'
Add a retry bound to the relay dialer
Problem: the dialer spun.
EOF

expect reject 'body over 12 lines' 'max 12' <<'EOF'
Add a retry bound to the relay dialer

1
2
3
4
5
6
7
8
9
10
11
12
13
EOF

expect reject 'AI Co-Authored-By trailer' 'Co-Authored-By' <<'EOF'
Add a retry bound to the relay dialer

Co-Authored-By: Claude <noreply@anthropic.com>
EOF

expect reject 'session URL' 'session URL' <<'EOF'
Add a retry bound to the relay dialer

See https://claude.ai/code/session/abc123
EOF

expect reject 'test results' 'CI' <<'EOF'
Add a retry bound to the relay dialer

All tests pass locally.
EOF

expect reject 'unfinished work' 'issue' <<'EOF'
Add a retry bound to the relay dialer

TODO: bound the cloud dialer too.
EOF

expect reject 'in-flight subject' 'issue' <<'EOF'
Add a retry bound to the relay dialer (in flight)
EOF

printf '\n%s\n' "-----------------------------------------------"
printf 'passed %d, failed %d\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
