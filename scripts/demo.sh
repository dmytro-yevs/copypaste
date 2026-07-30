#!/usr/bin/env bash
# End-to-end MVP demo: start the daemon, exercise the pipeline through the CLI,
# and assert the security rules that matter most.
#
# Runs against the fake clipboard backend on Linux; on macOS the same script
# drives the real NSPasteboard source. The daemon reports which backend is live
# so a demo can never be mistaken for the real thing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="$(mktemp -d)"
export XDG_DATA_HOME="$DATA_DIR"

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
    [[ -n "${DAEMON_PID:-}" ]] && kill "$DAEMON_PID" 2>/dev/null || true
    wait "${DAEMON_PID:-}" 2>/dev/null || true
    rm -rf "$DATA_DIR"
}
trap cleanup EXIT

step() { printf '\n\033[1;36m▶ %s\033[0m\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
fail() { printf '  \033[31m✗ %s\033[0m\n' "$*"; exit 1; }

step "Building"
$CARGO build --release -p copypaste-daemon -p copypaste-cli 2>&1 | tail -3

DAEMON="$ROOT/target/release/copypaste-daemon"
CLI="$ROOT/target/release/copypaste"

step "Starting daemon"
"$DAEMON" --foreground &
DAEMON_PID=$!
for _ in $(seq 1 50); do
    "$CLI" status >/dev/null 2>&1 && break
    sleep 0.2
done
"$CLI" status || fail "daemon did not become ready"
ok "daemon up"

step "Capture: add items"
"$CLI" add "hello from the demo"
"$CLI" add "second entry"
"$CLI" add "https://example.com/some/link"
ok "3 items added"

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

step "Done"
"$CLI" status
printf '\n\033[1;32mMVP demo passed.\033[0m\n'
