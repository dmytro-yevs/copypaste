#!/usr/bin/env bash
# Wait for the host to go quiet, then run a command and record what the load
# was while it ran.
#
# A benchmark taken while six other builds are running is not a benchmark. This
# is the only control this environment allows: there is no cpuset and no
# isolated core, so the alternative is to state the contention.
#
#   scripts/profile/quiet-run.sh <max_load> <max_wait_s> -- cmd...
set -euo pipefail

MAX_LOAD="${1:-1.5}"
MAX_WAIT="${2:-2400}"
shift 2
[[ "${1:-}" == "--" ]] && shift

load1() { cut -d' ' -f1 </proc/loadavg; }

waited=0
while awk -v l="$(load1)" -v m="$MAX_LOAD" 'BEGIN { exit !(l > m) }'; do
    if [[ $waited -ge $MAX_WAIT ]]; then
        echo "! still busy at $(load1) after ${waited}s; running anyway" >&2
        break
    fi
    sleep 15
    waited=$((waited + 15))
done

echo "== load before: $(cat /proc/loadavg)" >&2
"$@"
status=$?
echo "== load after:  $(cat /proc/loadavg)" >&2
exit $status
