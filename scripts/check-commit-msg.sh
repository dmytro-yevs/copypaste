#!/usr/bin/env bash
# Enforce CLAUDE.md rule 10 on a commit message.
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

# A merge is not one logical change and its message is not authored: git writes
# the subject from the ref names, and under merge.log the body too. Holding
# generated text to rule 10 bans merging rather than enforcing anything: the
# subject git writes for a merge into dmytro-yevs/commit-msg-hook is 98 chars,
# and no rewording is available short of --no-ff -m.
#
# The quote after the ref kind is what makes this git's output rather than
# prose. A hand-written subject that opens with "Merge" is still held to the
# rule, which is why history has "Merge the hardware SHA-2 backend ..." (59
# chars, imperative) and it passes on its own merits.
MERGE_GENERATED="^Merge (branch|branches|remote-tracking branch|remote-tracking branches|tag|commit) '"
MERGE_GENERATED+="|^Merge pull request #[0-9]+ from "

if [[ "$SUBJECT" =~ $MERGE_GENERATED ]]; then
    # Nothing about the shape is the author's to fix. The two bans below are:
    # git never emits either, so their presence is always deliberate.
    MERGE_TEXT="$(printf '%s\n' "${LINES[@]}")"
    if grep -qiE '^Co-Authored-By:.*(claude|copilot|gpt|ai)' <<< "$MERGE_TEXT"; then
        echo "  ✗ AI Co-Authored-By trailer — not used in this repository" >&2
        exit 1
    fi
    if grep -qiE 'claude\.ai/code/session|Claude-Session:' <<< "$MERGE_TEXT"; then
        echo "  ✗ session URL — not used in this repository" >&2
        exit 1
    fi
    exit 0
fi

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
