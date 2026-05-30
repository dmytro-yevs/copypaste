#!/usr/bin/env bash
#
# merge-train.sh — serialize many feature branches into one integration branch
# so an agent only ever faces ONE conflict set at a time, not a 30-branch pileup.
#
# Why this exists: per-crate `bl-*` branches touch disjoint files and merge clean;
# the `android-*` branches all hammer the same handful of files. Merging them in a
# deterministic, disjoint-first order (with `git rerere` replaying past conflict
# resolutions) turns a boiling N-way mess into a calm one-at-a-time queue.
#
# Usage:
#   scripts/merge-train.sh <integration-branch> [branch ...]
#   scripts/merge-train.sh <integration-branch> --from-file branches.txt
#   BUILD=1 scripts/merge-train.sh <integration-branch> ...   # build-gate each merge
#
# Behavior:
#   - Merges branches one at a time (--no-ff) in the given order.
#   - On a CLEAN merge: continues to the next branch.
#   - On a CONFLICT: stops immediately, prints the conflicted files, and leaves the
#     tree mid-merge so an agent resolves exactly one set, commits, then re-runs the
#     train (rerere will auto-replay this resolution next time the train runs).
#   - With BUILD=1: runs `cargo build --workspace` after each merge and halts on failure.
#
set -euo pipefail

die() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }
ok()  { printf '\033[32m✓ %s\033[0m\n' "$*"; }
info(){ printf '\033[36m→ %s\033[0m\n' "$*"; }

[ $# -ge 1 ] || die "usage: $0 <integration-branch> [branch ...] | --from-file <file>"

INTEGRATION="$1"; shift

# rerere is the single biggest lever here — make sure it's on for this repo.
if [ "$(git config --get rerere.enabled || echo false)" != "true" ]; then
  info "enabling git rerere for this repo (records & replays conflict resolutions)"
  git config rerere.enabled true
  git config rerere.autoupdate true
fi

# Collect the branch list.
BRANCHES=()
if [ "${1:-}" = "--from-file" ]; then
  [ -n "${2:-}" ] || die "--from-file needs a path"
  while IFS= read -r line; do
    line="${line%%#*}"; line="$(echo "$line" | xargs)"  # strip comments + whitespace
    [ -n "$line" ] && BRANCHES+=("$line")
  done < "$2"
else
  BRANCHES=("$@")
fi
[ "${#BRANCHES[@]}" -ge 1 ] || die "no branches to merge"

# Refuse to run on a dirty tree — a half-finished resolution would be clobbered.
git diff --quiet && git diff --cached --quiet || \
  die "working tree is dirty — commit/stash (or finish the in-progress merge) first"

info "checking out integration branch: $INTEGRATION"
git checkout "$INTEGRATION" >/dev/null 2>&1 || die "no such branch: $INTEGRATION"

merged=0
for br in "${BRANCHES[@]}"; do
  git rev-parse --verify "$br" >/dev/null 2>&1 || { info "skip (no such branch): $br"; continue; }

  # Already contained? Nothing to do.
  if git merge-base --is-ancestor "$br" HEAD; then
    ok "already merged: $br"
    continue
  fi

  info "merging: $br"
  if git merge --no-ff --no-edit "$br"; then
    ok "clean merge: $br"
    merged=$((merged+1))
  else
    conflicts="$(git diff --name-only --diff-filter=U)"
    printf '\033[33m⚠ CONFLICT merging %s — train halted.\033[0m\n' "$br" >&2
    echo "Conflicted files:" >&2
    echo "$conflicts" | sed 's/^/  /' >&2
    cat >&2 <<EOF

Resolve, then continue the train:
  1) edit the files above; \`git add\` each
  2) git commit --no-edit            # rerere will remember this resolution
  3) scripts/merge-train.sh $INTEGRATION ${BRANCHES[*]}   # re-run; merged branches are skipped
EOF
    exit 2
  fi

  if [ "${BUILD:-0}" = "1" ]; then
    info "build-gating ($br) …"
    cargo build --workspace >/dev/null 2>&1 || die "build broke after merging $br — fix on $INTEGRATION before continuing"
    ok "build green after $br"
  fi
done

ok "merge train complete — $merged branch(es) merged into $INTEGRATION"
