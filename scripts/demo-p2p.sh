#!/usr/bin/env bash
# End-to-end peer-sync demo: two daemons, one host, no shared state.
#
# Each daemon gets its own XDG_DATA_HOME — so its own database, its own device
# secret, its own socket and its own paired-device file — and its own peer port.
# Nothing here reaches into a database or a key: everything happens through the
# CLI, over the daemon socket, exactly as a user would drive it.
#
# Discovery is expected to be unavailable in a container. That is the point of
# the explicit `--addr`: mDNS saves typing an address and nothing else, so the
# demo passes with multicast filtered, which is also the state of a locked-down
# office network.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
A_DIR="$(mktemp -d)"
B_DIR="$(mktemp -d)"
A_PORT=47701
B_PORT=47702

CARGO="cargo"
command -v cargo +1.96 >/dev/null 2>&1 && CARGO="cargo +1.96"

cleanup() {
    [[ -n "${A_PID:-}" ]] && kill "$A_PID" 2>/dev/null || true
    [[ -n "${B_PID:-}" ]] && kill "$B_PID" 2>/dev/null || true
    wait "${A_PID:-}" 2>/dev/null || true
    wait "${B_PID:-}" 2>/dev/null || true
    rm -rf "$A_DIR" "$B_DIR"
}
trap cleanup EXIT

step() { printf '\n\033[1;36m▶ %s\033[0m\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
fail() { printf '  \033[31m✗ %s\033[0m\n' "$*"; exit 1; }

# Every CLI call names which device it is talking to by swapping the data home.
a() { XDG_DATA_HOME="$A_DIR" "$CLI" "$@"; }
b() { XDG_DATA_HOME="$B_DIR" "$CLI" "$@"; }

# Item content on one device, one line per item. Reads `list --json` rather
# than the table so nothing depends on column widths.
contents() {
    "$@" list --limit 100 --json \
        | tr ',' '\n' \
        | grep -o '"content": *"[^"]*"' \
        | cut -d'"' -f4
}

# Total items a sync run moved, in either direction. Read out of `--json` so
# it does not depend on table column positions.
moved() {
    "$@" sync --json \
        | tr -d ' ' \
        | grep -oE '"(sent|received)":[0-9]+' \
        | awk -F: '{ total += $2 } END { print total + 0 }'
}

has_item() {
    local needle="$1"
    shift
    contents "$@" | grep -qxF "$needle"
}

step "Building"
$CARGO build --release -p copypaste-daemon -p copypaste-cli 2>&1 | tail -3

DAEMON="$ROOT/target/release/copypaste-daemon"
CLI="$ROOT/target/release/copypaste"

step "Starting two daemons on separate data directories and ports"
XDG_DATA_HOME="$A_DIR" "$DAEMON" --foreground --port "$A_PORT" --device-name device-a &
A_PID=$!
XDG_DATA_HOME="$B_DIR" "$DAEMON" --foreground --port "$B_PORT" --device-name device-b &
B_PID=$!

for _ in $(seq 1 50); do
    a status >/dev/null 2>&1 && b status >/dev/null 2>&1 && break
    sleep 0.2
done
a status >/dev/null || fail "daemon A did not become ready"
b status >/dev/null || fail "daemon B did not become ready"
ok "A on port $A_PORT, B on port $B_PORT"

step "Give each device something the other has never seen"
a add "alpha one"
a add "alpha two"
b add "bravo one"
ok "A has 2 items, B has 1"

step "A secret on A, which must never leave it"
a add "AKIAIOSFODNN7EXAMPLE"
has_item "AKIAIOSFODNN7EXAMPLE" a || fail "the secret was not stored on A"
ok "stored on A, flagged by the detector"

step "Pair: A mints a code, B accepts it at an explicit address"
PAIR_OUT=$(a pair create --name device-b)
CODE=$(printf '%s\n' "$PAIR_OUT" | awk '/^code /{print $2}')
PAIRING_ID=$(printf '%s\n' "$PAIR_OUT" | awk '/^pairing id /{print $3}')
[[ -n "$CODE" ]] || fail "no pairing code was printed"
[[ -n "$PAIRING_ID" ]] || fail "no pairing id was printed"
[[ "$CODE" != "$PAIRING_ID" ]] || fail "the code and the pairing id must not be the same value"
ok "code minted (not echoed here — it is a secret)"

b pair accept "$CODE" --addr "127.0.0.1:$A_PORT" || fail "pairing failed"
ok "B paired with A over 127.0.0.1:$A_PORT"

step "Both devices list the pairing"
a peers | grep -q "$PAIRING_ID" || fail "A does not list the pairing"
b peers | grep -q "$PAIRING_ID" || fail "B does not list the pairing"
ok "pairing id $PAIRING_ID on both sides"

step "Pairing itself ran one session, so the first three items are already across"
for item in "alpha one" "alpha two" "bravo one"; do
    has_item "$item" a || fail "A is missing '$item'"
    has_item "$item" b || fail "B is missing '$item'"
done
ok "alpha one, alpha two, bravo one — on both devices"

step "New items on both sides, then an explicit sync"
a add "alpha three"
b add "bravo two"
MOVED=$(moved b)
[[ "$MOVED" -ge 2 ]] || fail "sync moved $MOVED items; expected one each way"
ok "sync moved items in both directions"

step "Both devices hold the union of the items"
for item in "alpha one" "alpha two" "alpha three" "bravo one" "bravo two"; do
    has_item "$item" a || fail "A is missing '$item'"
    has_item "$item" b || fail "B is missing '$item'"
done
ok "all five items on both devices"

step "The secret stayed on A"
has_item "AKIAIOSFODNN7EXAMPLE" a || fail "the secret vanished from A — data loss"
if has_item "AKIAIOSFODNN7EXAMPLE" b; then
    fail "SENSITIVE CONTENT WAS SYNCED TO THE PEER"
fi
if b search "AKIAIOSFODNN7EXAMPLE" 2>/dev/null | grep -q "AKIAIOSFODNN7EXAMPLE"; then
    fail "SENSITIVE CONTENT REACHED THE PEER'S SEARCH INDEX"
fi
ok "sensitive item present on its origin, absent from the peer"

step "Syncing again transfers nothing"
b sync
TRANSFERRED=$(moved b)
[[ "$TRANSFERRED" == "0" ]] || fail "a repeated sync moved $TRANSFERRED items; it must be a no-op"
ok "idempotent: 0 sent, 0 received"

step "A wrong pairing code cannot pair"
BEFORE=$(b peers | grep -c "$PAIRING_ID" || true)
WRONG="AAAA-BBBB-CCCC-DDDD-EEEE-FFFF-GGGG-HHHH-JJJJ-KKKK-MMMM-NNNN-PPPP"
if b pair accept "$WRONG" --addr "127.0.0.1:$A_PORT" 2>/dev/null; then
    fail "a wrong code was accepted"
fi
AFTER=$(b peers | grep -c "$PAIRING_ID" || true)
[[ "$BEFORE" == "$AFTER" ]] || fail "a failed pairing changed the peer list"
ok "handshake refused, and nothing was persisted"

step "A malformed code is refused before any connection is made"
if b pair accept "not-a-code" --addr "127.0.0.1:$A_PORT" 2>/dev/null; then
    fail "a malformed code was accepted"
fi
ok "refused"

step "Unpair"
b unpair "$PAIRING_ID" || fail "unpair failed"
if b peers | grep -q "$PAIRING_ID"; then
    fail "the peer survived unpair"
fi
ok "B forgot the pairing"

step "Done"
a status
b status
printf '\n\033[1;32mPeer-sync demo passed.\033[0m\n'
