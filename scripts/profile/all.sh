#!/usr/bin/env bash
# Every measurement behind docs/rewrite/performance.md, in order, into a log.
#
# Sequential on purpose: two of these running at once measure each other.
#
#   scripts/profile/all.sh [output.log]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
LOG="${1:-$ROOT/target/profile-$(date +%Y%m%d-%H%M%S).log}"
CARGO="cargo"
cargo +1.96 --version >/dev/null 2>&1 && CARGO="cargo +1.96"

exec > >(tee "$LOG") 2>&1
echo "== $(date -u +%FT%TZ)  $(uname -sr)  $(nproc) cpu  load $(cat /proc/loadavg)"

section() { printf '\n\n######## %s\n\n' "$*"; }

section "daemon idle (battery proxy)"
"$HERE/quiet-run.sh" "${MAX_LOAD:-2.0}" "${MAX_WAIT:-600}" -- "$HERE/daemon-idle.sh" 300 500 1000 5000

section "capture, history and detection microbenchmarks"
cd "$ROOT"
for bench in detect capture history; do
    "$HERE/quiet-run.sh" "${MAX_LOAD:-2.0}" "${MAX_WAIT:-600}" -- \
        $CARGO bench -p copypaste-core --bench "$bench"
done

section "memory over a capture stream"
"$HERE/quiet-run.sh" "${MAX_LOAD:-2.0}" "${MAX_WAIT:-600}" -- "$HERE/daemon-soak.sh" 8 4 512

section "what the app's polling costs the daemon"
"$HERE/quiet-run.sh" "${MAX_LOAD:-2.0}" "${MAX_WAIT:-600}" -- "$HERE/ipc-cost.sh" 10000 200

echo
echo "== done  $(date -u +%FT%TZ)  log: $LOG"
