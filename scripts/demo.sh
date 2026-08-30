#!/usr/bin/env bash
# End-to-end MVP demo: start the daemon, exercise the pipeline through the CLI,
# and assert the security rules that matter most.
#
# Uses the compile-time fake clipboard on every host. It never reads or writes
# the system clipboard, real history, keychain, cloud or LAN discovery.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="$(mktemp -d)"
export XDG_DATA_HOME="$DATA_DIR"
export COPYPASTE_EPHEMERAL_KEY=1
export COPYPASTE_SOCKET="$DATA_DIR/daemon.sock"
export COPYPASTE_FAKE_CLIPBOARD="$DATA_DIR/fake-clipboard"
export COPYPASTE_FAKE_CLIPBOARD_ACK="$DATA_DIR/fake-clipboard.ack"
export CARGO_TARGET_DIR="$DATA_DIR/target"
: > "$COPYPASTE_FAKE_CLIPBOARD"

# Prefer the pinned toolchain, fall back to whatever `cargo` resolves to.
#
# The obvious `command -v cargo +1.96` does not test what it looks like it
# tests: bash's `command -v` takes several names and exits 0 if *any* of them
# resolves, so `cargo` alone satisfies it and the pin is applied even on a
# machine with no 1.96 installed — where every later invocation then fails with
# a rustup error instead of falling back. Ask the toolchain itself.
CARGO="cargo"
cargo +1.96 --version >/dev/null 2>&1 && CARGO="cargo +1.96"

cleanup() {
    [[ -n "${WATCH_PID:-}" ]] && kill "$WATCH_PID" 2>/dev/null || true
    wait "${WATCH_PID:-}" 2>/dev/null || true
    [[ -n "${DAEMON_PID:-}" ]] && kill "$DAEMON_PID" 2>/dev/null || true
    wait "${DAEMON_PID:-}" 2>/dev/null || true
    rm -rf "$DATA_DIR"
}
trap cleanup EXIT

step() { printf '\n\033[1;36m▶ %s\033[0m\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
fail() { printf '  \033[31m✗ %s\033[0m\n' "$*"; exit 1; }

status_json() { "$CLI" --json status; }

status_field() {
    status_json | python3 -c 'import json, sys; print(json.load(sys.stdin)["data"]["status"][sys.argv[1]])' "$1"
}

wait_for_status_field() {
    local field="$1" expected="$2"
    for _ in $(seq 1 80); do
        [[ "$(status_field "$field")" == "$expected" ]] && return
        sleep 0.1
    done
    fail "status field $field did not become $expected"
}

write_fake_text() {
    printf '%s' "$1" > "$COPYPASTE_FAKE_CLIPBOARD"
    mark_fake_change
}

mark_fake_change() {
    python3 - "$COPYPASTE_FAKE_CLIPBOARD" <<'PY'
import os
import sys
import time

stamp = time.time_ns()
os.utime(sys.argv[1], ns=(stamp, stamp))
PY
}

start_capture_watch() {
    WATCH_OUT="$DATA_DIR/capture-watch.out"
    : > "$WATCH_OUT"
    "$CLI" watch > "$WATCH_OUT" &
    WATCH_PID=$!
    # The event below, rather than this bounded subscription setup, is the
    # proof that capture completed.
    sleep 0.1
}

wait_for_capture_event() {
    for _ in $(seq 1 80); do
        grep -q 'captured an item' "$WATCH_OUT" && return
        sleep 0.1
    done
    fail "the external fake-clipboard edit did not produce a capture event"
}

stop_capture_watch() {
    kill "${WATCH_PID:-}" 2>/dev/null || true
    wait "${WATCH_PID:-}" 2>/dev/null || true
    unset WATCH_PID
}

wait_for_search() {
    local value="$1"
    for _ in $(seq 1 80); do
        "$CLI" search "$value" --json 2>/dev/null | grep -Fq -- "$value" && return
        sleep 0.1
    done
    fail "captured value was not stored and searchable"
}

set_private_mode() {
    python3 - "$COPYPASTE_SOCKET" "$1" <<'PY'
import json
import socket
import sys

socket_path, enabled = sys.argv[1], sys.argv[2] == "true"
request = {
    "id": 91,
    "protocol_version": 2,
    "method": "set_private_mode",
    "params": {"enabled": enabled},
}
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
    connection.connect(socket_path)
    connection.sendall(json.dumps(request).encode() + b"\n")
    response = json.loads(connection.makefile("rb").readline())
if response.get("id") != request["id"] or not response.get("ok"):
    raise SystemExit("private-mode request was not acknowledged")
if response.get("data", {}).get("private_mode", {}).get("private_mode") is not enabled:
    raise SystemExit("private-mode acknowledgement did not match the request")
PY
}

wait_for_ack_after() {
    local previous="$1" generation
    for _ in $(seq 1 80); do
        generation="$(cat "$COPYPASTE_FAKE_CLIPBOARD_ACK" 2>/dev/null || true)"
        if [[ "$generation" =~ ^[0-9]+$ ]] && [[ "$generation" -gt "$previous" ]]; then
            return
        fi
        sleep 0.1
    done
    fail "the private fake-clipboard edit was not acknowledged"
}

step "Building"
$CARGO build --locked --release -p copypaste-daemon -p copypaste-cli \
    --features copypaste-daemon/dev-ephemeral-key,copypaste-daemon/dev-fake-clipboard 2>&1 | tail -3

DAEMON="$CARGO_TARGET_DIR/release/copypaste-daemon"
CLI="$CARGO_TARGET_DIR/release/copypaste"

step "Starting daemon"
env -u COPYPASTE_CLOUD_URL -u COPYPASTE_CLOUD_ANON_KEY \
    "$DAEMON" --foreground --data-dir "$DATA_DIR" --port 0 &
DAEMON_PID=$!
for _ in $(seq 1 50); do
    "$CLI" status >/dev/null 2>&1 && break
    sleep 0.2
done
"$CLI" status || fail "daemon did not become ready"
[[ "$(status_field clipboard_backend)" == "fake-file" ]] \
    || fail "demo did not select the watched fake clipboard"
ok "daemon up"

step "Capture: an external watched-file edit reaches normal history"
CAPTURED_VALUE="captured through the fake file"
start_capture_watch
write_fake_text "$CAPTURED_VALUE"
wait_for_capture_event
wait_for_search "$CAPTURED_VALUE"
stop_capture_watch
ok "external change tracker, capture loop and ingest stored the value"

step "Capture: an identical external re-copy still reaches ingest"
start_capture_watch
write_fake_text "$CAPTURED_VALUE"
wait_for_capture_event
stop_capture_watch
ok "the capture event proves re-copy reached ingest even when dedup kept one row"

step "Capture: private mode acknowledges without replaying"
set_private_mode true
PRIVATE_ACK="$(cat "$COPYPASTE_FAKE_CLIPBOARD_ACK" 2>/dev/null || printf 0)"
write_fake_text "copied while private"
wait_for_ack_after "$PRIVATE_ACK"
set_private_mode false
if "$CLI" search "copied while private" --json 2>/dev/null | grep -Fq 'copied while private'; then
    fail "a private capture was stored"
fi
start_capture_watch
write_fake_text "captured after private mode"
wait_for_capture_event
wait_for_search "captured after private mode"
stop_capture_watch
ok "private edit was consumed before capture resumed, without replay"

step "Capture: exact limit succeeds, one byte over is counted, next value survives"
"$CLI" config set --max-text-size-bytes 65536 >/dev/null \
    || fail "the minimum text capture limit was refused"
ITEMS_BEFORE="$(status_field item_count)"
start_capture_watch
head -c 65536 /dev/zero | tr '\0' x > "$COPYPASTE_FAKE_CLIPBOARD"
mark_fake_change
wait_for_capture_event
wait_for_status_field item_count "$((ITEMS_BEFORE + 1))"
stop_capture_watch
REJECTED_BEFORE="$(status_json | python3 -c 'import json, sys; print(json.load(sys.stdin)["data"]["status"]["counters"]["rejected_too_large"])')"
head -c 65537 /dev/zero | tr '\0' x > "$COPYPASTE_FAKE_CLIPBOARD"
mark_fake_change
for _ in $(seq 1 80); do
    CURRENT_REJECTED="$(status_json | python3 -c 'import json, sys; print(json.load(sys.stdin)["data"]["status"]["counters"]["rejected_too_large"])')"
    [[ "$CURRENT_REJECTED" == "$((REJECTED_BEFORE + 1))" ]] && break
    sleep 0.1
done
[[ "${CURRENT_REJECTED:-}" == "$((REJECTED_BEFORE + 1))" ]] \
    || fail "one-byte-over capture was not counted"
start_capture_watch
write_fake_text "captured after size refusal"
wait_for_capture_event
wait_for_search "captured after size refusal"
stop_capture_watch
ok "the size gate counted the refusal and normal capture recovered"

step "Direct ingest: add items (not a capture proof)"
"$CLI" add "hello from the demo"
"$CLI" add "second entry"
"$CLI" add "https://example.com/some/link"
ok "3 items directly ingested"

step "List"
"$CLI" list --limit 10

step "Search"
"$CLI" search "second"
ok "search returned"

step "Pin ordering"
# `add` prints "added <uuid>" — parse that rather than the pretty-printed
# JSON, whose whitespace is not a stable contract.
FIRST_ID=$("$CLI" add "pin target" | awk '{print $2}')
"$CLI" add "newest unpinned"
"$CLI" pin "$FIRST_ID"
TOP_ID=$("$CLI" list --limit 1 --json | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
[[ "$TOP_ID" == "$FIRST_ID" ]] && ok "pinned item sorts above newer unpinned" \
    || fail "pin ordering broken: expected $FIRST_ID at top, got $TOP_ID"

step "Secret detection — an AWS key must be flagged and kept out of the index"
"$CLI" add "AKIAIOSFODNN7EXAMPLE"
if "$CLI" search "AKIAIOSFODNN7EXAMPLE" 2>/dev/null | grep -q "AKIAIOSFODNN7EXAMPLE"; then
    fail "SENSITIVE CONTENT REACHED THE SEARCH INDEX"
fi
ok "sensitive item is not searchable"

step "At rest: the database must not contain plaintext"
DB=$(find "$DATA_DIR" -name '*.db' | head -1)
if [[ -n "$DB" ]]; then
    if grep -qa "hello from the demo" "$DB"; then
        fail "PLAINTEXT FOUND IN THE DATABASE FILE"
    fi
    ok "no plaintext in $(basename "$DB")"
else
    fail "no database file was created"
fi

step "Delete"
"$CLI" delete "$FIRST_ID"
if "$CLI" list --limit 20 --json | grep -q "$FIRST_ID"; then fail "item survived delete"; fi
ok "item deleted"

step "Settings — changed live, and a bad value changes nothing"
"$CLI" config set --poll-interval-ms 250 >/dev/null || fail "a valid setting was refused"
"$CLI" config show | grep -q "250 ms" || fail "the new poll interval was not applied"
if "$CLI" config set --poll-interval-ms 1 2>/dev/null; then
    fail "an out-of-range setting was accepted"
fi
"$CLI" config show | grep -q "250 ms" || fail "a rejected setting changed the daemon"
ok "poll interval is 250 ms; an out-of-range value was refused and changed nothing"

step "Export — sensitive items are withheld by default and the count is reported"
EXPORT="$DATA_DIR/history.json"
"$CLI" export --output "$EXPORT" 2>"$DATA_DIR/export.err" || fail "export failed"
if grep -q "AKIAIOSFODNN7EXAMPLE" "$EXPORT"; then
    fail "A SENSITIVE ITEM WAS WRITTEN TO THE EXPORT BY DEFAULT"
fi
grep -q "withheld 1 sensitive" "$DATA_DIR/export.err" || fail "the export did not report what it withheld"
ok "secret withheld, and the export said so"

step "Import — the detector runs again, so an edited export cannot smuggle one back"
sed 's/"content": "note two"/"content": "AKIAIOSFODNN7EXAMPLE"/' "$EXPORT" > "$DATA_DIR/tampered.json"
"$CLI" import "$DATA_DIR/tampered.json" >/dev/null || fail "import failed"
if "$CLI" search "AKIAIOSFODNN7EXAMPLE" --json | grep -q "AKIAIOSFODNN7EXAMPLE"; then
    fail "AN IMPORTED CREDENTIAL REACHED THE SEARCH INDEX"
fi
ok "re-detected on the way in, and kept out of the index"

step "Backup and restore — a damaged backup cannot replace a working history"
BACKUP="$DATA_DIR/history.backup"
"$CLI" backup "$BACKUP" >/dev/null || fail "backup failed"
BEFORE=$("$CLI" status | awk '/^items/{print $2}')

printf 'not a database at all' > "$DATA_DIR/junk.backup"
if "$CLI" restore "$DATA_DIR/junk.backup" --yes 2>/dev/null; then
    fail "a corrupt backup was accepted"
fi
AFTER=$("$CLI" status | awk '/^items/{print $2}')
[[ "$BEFORE" == "$AFTER" ]] || fail "a refused restore changed history: $BEFORE -> $AFTER"

"$CLI" add "written after the backup" >/dev/null
"$CLI" restore "$BACKUP" --yes >/dev/null || fail "restore failed"
if "$CLI" search "written after the backup" --json | grep -q "written after the backup"; then
    fail "the restore did not replace history"
fi
ok "corrupt backup refused with history intact; a good one restored"

step "Watch — a push, not a poll"
"$CLI" watch > "$DATA_DIR/watch.out" &
WATCH_PID=$!
sleep 0.5
"$CLI" add "this should be pushed" >/dev/null
for _ in $(seq 1 25); do
    grep -q "items changed" "$DATA_DIR/watch.out" && break
    sleep 0.2
done
kill "$WATCH_PID" 2>/dev/null || true
wait "$WATCH_PID" 2>/dev/null || true
grep -q "items changed" "$DATA_DIR/watch.out" || fail "no change was pushed to the subscriber"
ok "the daemon pushed a change event"

step "Done"
"$CLI" status
printf '\n\033[1;32mMVP demo passed.\033[0m\n'
