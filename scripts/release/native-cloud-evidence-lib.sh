#!/usr/bin/env bash
set -uo pipefail

PASS="${PASS:-0}"
FAIL="${FAIL:-0}"
if ! declare -F ok >/dev/null; then
    ok() { PASS=$((PASS + 1)); printf '  ok    %s\n' "$1"; }
    bad() { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$1"; [[ -z "${2:-}" ]] || printf '        %s\n' "$2"; }
    group() { printf '\n== %s\n' "$1"; }
fi

cloud_latency_record() { # <tsv> <scenario> <latency_ms> <budget_ms>
    printf '%s\t%s\t%s\n' "$2" "$3" "$4" >> "$1"
    (( $3 <= $4 ))
}

cloud_latency_write() { # <tsv> <json> <platform>
    python3 - "$1" "$2" "$3" <<'PY'
import json
import pathlib
import sys

rows = []
for line in pathlib.Path(sys.argv[1]).read_text().splitlines():
    scenario, latency, budget = line.split("\t")
    rows.append({"scenario": scenario, "latency_ms": int(latency), "budget_ms": int(budget)})
if not rows:
    raise SystemExit("no native cloud latency measurements")
pathlib.Path(sys.argv[2]).write_text(json.dumps({"platform": sys.argv[3], "measurements": rows}, indent=2) + "\n")
PY
}

cloud_evidence_self_test() {
    local temp="$1" records="$1/latency.tsv"
    : > "$records"
    cloud_latency_record "$records" status-ready 120 500 \
        && ok "a latency within budget passes" \
        || bad "a latency within budget passes"
    cloud_latency_record "$records" sign-in 501 500 \
        && bad "a latency over budget fails" \
        || ok "a latency over budget fails"
    cloud_latency_write "$records" "$temp/latency.json" self-test
    python3 - "$temp/latency.json" <<'PY' \
        && ok "latency evidence is structured and complete" \
        || bad "latency evidence is structured and complete"
import json, pathlib, sys
doc = json.loads(pathlib.Path(sys.argv[1]).read_text())
assert doc["platform"] == "self-test"
assert [row["scenario"] for row in doc["measurements"]] == ["status-ready", "sign-in"]
PY
}

cloud_evidence_summary() { # <platform>
    local verdict=passed
    [[ $FAIL -eq 0 ]] || verdict=FAILED
    printf '\n## %s native cloud evidence: %s\n\n%d assertions passed, %d failed.\n' \
        "$1" "$verdict" "$PASS" "$FAIL"
}
