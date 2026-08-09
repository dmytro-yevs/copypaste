#!/usr/bin/env bash
# Enforce AGENTS.md rule 10 on a commit message.
#
# One implementation, two callers: .githooks/commit-msg and CI. A rule with no
# check is how the previous guidance failed — twelve commits averaged 25 body
# lines against a 12-line budget.
#
# Usage: check-commit-msg.sh <file-with-message>
set -euo pipefail

MSG_FILE="${1:?usage: check-commit-msg.sh <file>}"
MAX_SUBJECT=72
MAX_BODY=12

fail() { printf '  ✗ %s\n' "$1" >&2; FAILED=1; }
FAILED=0

# Strip comments and the diff git appends under --verbose.
#
# `read` returns non-zero on a final line with no newline, so without the `||`
# the loop body never runs for it. git writes .git/MERGE_MSG unterminated and
# it is a single line, which made every merge arrive here as "empty subject".
LINES=()
while IFS= read -r line || [[ -n "$line" ]]; do
    LINES[${#LINES[@]}]="$line"
done < <(sed -e '/^#/d' -e '/^diff --git /,$d' "$MSG_FILE")

SUBJECT="${LINES[0]:-}"
[[ -z "${SUBJECT// }" ]] && { echo "  ✗ empty subject" >&2; exit 1; }

# --- subject -----------------------------------------------------------------

if (( ${#SUBJECT} > MAX_SUBJECT )); then
       fail "subject is ${#SUBJECT} chars, max $MAX_SUBJECT"
fi

if [[ "$SUBJECT" == *. ]]; then
    fail "subject ends in a period"
fi

# Past tense and gerunds are the two common non-imperative openings. This is a
# heuristic on the first word only — it cannot parse English, and a false
# positive is cheaper than the alternative.
FIRST="${SUBJECT%% *}"
case "$FIRST" in
    Added|Fixed|Removed|Updated|Changed|Wired|Moved|Renamed|Deleted|Landed)
        fail "subject starts with past tense '$FIRST' — use the imperative" ;;
    Adding|Fixing|Removing|Updating|Changing|Wiring|Moving)
        fail "subject starts with a gerund '$FIRST' — use the imperative" ;;
esac

# --- body --------------------------------------------------------------------

if (( ${#LINES[@]} > 1 )); then
    if [[ -n "${LINES[1]// }" ]]; then
        fail "no blank line between subject and body"
    fi
fi

# `(( n++ ))` returns 1 when n was 0, and `set -e` would take that as a
# failure and exit — silently skipping every check below.
BODY_LINES=0
for (( i = 1; i < ${#LINES[@]}; i++ )); do
    [[ -n "${LINES[i]// }" ]] && BODY_LINES=$(( BODY_LINES + 1 ))
done

if (( BODY_LINES > MAX_BODY )); then
       fail "body is $BODY_LINES lines, max $MAX_BODY — split the commit, or move the reasoning to an ADR or a document"
fi

# --- forbidden content -------------------------------------------------------

BODY="$(printf '%s\n' "${LINES[@]:1}")"

if grep -qiE '^Co-Authored-By:.*(claude|copilot|gpt|ai)' <<< "$BODY"; then
    fail "AI Co-Authored-By trailer — not used in this repository"
fi

if grep -qiE 'claude\.ai/code/session|Claude-Session:' <<< "$BODY"; then
    fail "session URL — not used in this repository"
fi

if grep -qiE '\ball tests pass\b|\btests? (all )?(pass|passing|green)\b' <<< "$BODY"; then
    fail "test results belong to CI, not the message"
fi

if grep -qiE '\b(in flight|WIP|work in progress|TODO)\b' <<< "$BODY$SUBJECT"; then
    fail "unfinished work belongs in an issue, not on main"
fi

if (( FAILED )); then
    cat >&2 <<'EOF'

CLAUDE.md rule 10. Allowed shape:

  <imperative subject, <=72 chars, no period>

  Problem:
  One or two sentences.

  Change:
  - One to four concrete points

  Risk:
  One sentence, only when there is a real one.
EOF
    exit 1
fi
