#!/usr/bin/env bash
# End-to-end cloud-sync demo: two daemons, one account, no shared state.
#
# ⚠ THE BACKEND IS A STUB, NOT SUPABASE. ⚠
#
# `scripts/cloud-stub.py` answers the GoTrue and PostgREST calls the client
# makes and keeps the rows in memory. No Postgres, no row-level security, no JWT
# verification, and this container cannot reach a real Supabase project.
#
# What a pass means: the daemon is wired to `copypaste-cloud` and driven only
# through the CLI; two devices converge sharing nothing but a passphrase; a
# sensitive item never leaves the device that captured it; the backend receives
# ciphertext only (asserted against the stub's dump of every row it was given);
# and the client's request shapes are the ones PostgREST needs — the stub
# rejects a newest-first page and a strict bound on the millisecond alone, the
# two shapes manifest 05 §4.4 records as shipped v1 bugs, and it implements the
# compound `(created_at, item_id)` keyset the client pages with.
#
# What a pass does not mean: that Supabase accepts any of it. Nothing here has
# ever spoken to a deployment.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
A_DIR="$(mktemp -d)"
B_DIR="$(mktemp -d)"
STUB_PORT=47810
STUB_DUMP="$(mktemp)"
STUB_URL="http://127.0.0.1:$STUB_PORT"

# The two secrets. The password authenticates to the account; the passphrase
# derives the key the backend must never be able to derive. Both are passed by
# environment, never as arguments — argv is readable by every process running
# as this user.
export COPYPASTE_CLOUD_PASSWORD="stub-password"
export COPYPASTE_SYNC_PASSPHRASE="correct horse battery staple"
export COPYPASTE_CLOUD_URL="$STUB_URL"
export COPYPASTE_CLOUD_ANON_KEY="stub-anon-key"

# Prefer the pinned toolchain, fall back to whatever `cargo` resolves to. Asking
# the toolchain itself rather than `command -v cargo +1.96`, which exits 0 on
# `cargo` alone and applies a pin that is not installed (see demo-p2p.sh).
CARGO="cargo"
cargo +1.96 --version >/dev/null 2>&1 && CARGO="cargo +1.96"

cleanup() {
    for pid in "${A_PID:-}" "${B_PID:-}" "${STUB_PID:-}"; do
        [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
    rm -rf "$A_DIR" "$B_DIR" "$STUB_DUMP"
}
trap cleanup EXIT

step() { printf '\n\033[1;36m▶ %s\033[0m\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
fail() { printf '  \033[31m✗ %s\033[0m\n' "$*"; exit 1; }

a() { XDG_DATA_HOME="$A_DIR" "$CLI" "$@"; }
b() { XDG_DATA_HOME="$B_DIR" "$CLI" "$@"; }

# Item content on one device, one line per item, read from `list --json` so
# nothing depends on table column widths.
contents() {
    "$@" list --limit 100 --json \
        | tr ',' '\n' \
        | grep -o '"content": *"[^"]*"' \
        | cut -d'"' -f4
}

has_item() {
    local needle="$1"
    shift
    contents "$@" | grep -qxF "$needle"
}

# One field out of `cloud sync --json`, e.g. `synced a uploaded`.
synced() {
    local field="$2"
    "$1" cloud sync --json \
        | tr -d ' ' \
        | grep -oE "\"$field\":[0-9]+" \
        | cut -d: -f2
}

printf '\033[1;33m%s\033[0m\n' \
    "NOTE: the backend in this demo is a local stub, not a Supabase deployment."

step "Building"
$CARGO build --release -p copypaste-daemon -p copypaste-cli 2>&1 | tail -3

DAEMON="$ROOT/target/release/copypaste-daemon"
CLI="$ROOT/target/release/copypaste"

step "Starting the stub backend"
python3 "$ROOT/scripts/cloud-stub.py" \
    --port "$STUB_PORT" \
    --password "$COPYPASTE_CLOUD_PASSWORD" \
    --dump "$STUB_DUMP" &
STUB_PID=$!
for _ in $(seq 1 50); do
    curl -fsS -o /dev/null "$STUB_URL/auth/v1/logout" -X POST 2>/dev/null && break
    sleep 0.2
done
ok "stub listening on $STUB_URL (NOT Supabase)"

step "Starting two daemons on separate data directories"
XDG_DATA_HOME="$A_DIR" "$DAEMON" --foreground --port 47811 --device-name device-a &
A_PID=$!
XDG_DATA_HOME="$B_DIR" "$DAEMON" --foreground --port 47812 --device-name device-b &
B_PID=$!
for _ in $(seq 1 50); do
    a status >/dev/null 2>&1 && b status >/dev/null 2>&1 && break
    sleep 0.2
done
a status >/dev/null || fail "daemon A did not become ready"
b status >/dev/null || fail "daemon B did not become ready"
ok "both daemons running, both configured with the stub URL"

# `cloud status` output is captured before being searched, never piped into
# `grep -q`: grep exits on the first match and the writer then takes an EPIPE
# mid-block.
cloud_status() { "$@" cloud status; }

step "Before signing in, cloud status says so and sync refuses"
grep -q "signed out" <<<"$(cloud_status a)" || fail "A should report a signed-out account"
if a cloud sync >/dev/null 2>&1; then
    fail "syncing while signed out must fail"
fi
ok "configured, signed out, and sync refused rather than silently doing nothing"

step "History that exists *before* sign-in must still upload (BUG C2)"
a add "captured before signing in"
ok "one item on A, captured while signed out"

step "A signs in"
a cloud sign-in --email demo@example.com || fail "sign-in failed"
grep -q "demo@example.com" <<<"$(cloud_status a)" || fail "A does not report the account"
ok "signed in, sync key derived from the passphrase"

step "A syncs: the pre-existing item goes up"
UPLOADED=$(synced a uploaded)
[[ "$UPLOADED" -ge 1 ]] || fail "the backlog sweep uploaded $UPLOADED items"
ok "uploaded $UPLOADED item(s) captured before sign-in"

step "A secret on A, which must never leave it"
a add "AKIAIOSFODNN7EXAMPLE"
has_item "AKIAIOSFODNN7EXAMPLE" a || fail "the secret was not stored on A"
# The round reports 0 withheld because the *store* filter caught it first: the
# outbound query never lists a sensitive row, so the driver's `SensitiveGuard`
# — the second layer — has nothing left to refuse. Both layers are required
# (AT-56 / CopyPaste-20yw); what matters below is that the backend never sees it.
WITHHELD=$(synced a skipped_sensitive)
ok "stored on A; the outbound query withheld it before the upload gate ($WITHHELD refused at the gate)"

step "B signs in to the same account with the same passphrase"
b cloud sign-in --email demo@example.com || fail "sign-in on B failed"
b add "from device b"
b cloud sync >/dev/null || fail "sync on B failed"
ok "B signed in and synced"

step "B has A's items; A picks up B's on its next round"
a cloud sync >/dev/null || fail "sync on A failed"
has_item "captured before signing in" b || fail "B never received A's item"
has_item "from device b" a || fail "A never received B's item"
ok "both devices hold the union, sharing only a passphrase"

step "The secret stayed on A"
has_item "AKIAIOSFODNN7EXAMPLE" a || fail "the secret vanished from A — data loss"
if has_item "AKIAIOSFODNN7EXAMPLE" b; then
    fail "SENSITIVE CONTENT WAS SYNCED THROUGH THE CLOUD"
fi
if grep -q "AKIAIOSFODNN7EXAMPLE" "$STUB_DUMP"; then
    fail "SENSITIVE CONTENT REACHED THE BACKEND"
fi
ok "sensitive item present on its origin, absent from the peer and the backend"

step "The backend holds ciphertext only"
for plaintext in "captured before signing in" "from device b"; do
    if grep -qF "$plaintext" "$STUB_DUMP"; then
        fail "PLAINTEXT REACHED THE BACKEND: $plaintext"
    fi
done
grep -q '"ciphertext"' "$STUB_DUMP" || fail "the backend received no rows at all"
ok "every stored row is sealed; no plaintext anywhere in what was sent"

step "Syncing again moves nothing"
a cloud sync >/dev/null
APPLIED=$(synced a applied)
UPLOADED=$(synced a uploaded)
[[ "$APPLIED" == "0" ]] || fail "a repeated round applied $APPLIED rows"
[[ "$UPLOADED" == "0" ]] || fail "a repeated round uploaded $UPLOADED rows"
ok "idempotent: 0 uploaded, 0 applied"

step "A delete propagates as a tombstone"
DOOMED_ID=$(b add "delete me" --json | grep -o '"id": *"[^"]*"' | head -1 | cut -d'"' -f4)
b cloud sync >/dev/null
a cloud sync >/dev/null
has_item "delete me" a || fail "A never received the item to be deleted"
b delete "$DOOMED_ID" >/dev/null || fail "delete failed on B"
b cloud sync >/dev/null
a cloud sync >/dev/null
if has_item "delete me" a; then
    fail "the delete did not propagate; the item is still on A"
fi
ok "deleted on B, gone from A, and not resurrected by the next round"

step "Sign-out is persistent"
a cloud sign-out || fail "sign-out failed"
grep -q "signed out" <<<"$(cloud_status a)" || fail "A still reports an account"
if a cloud sync >/dev/null 2>&1; then
    fail "syncing after sign-out must fail"
fi
has_item "from device b" a || fail "sign-out destroyed local history"
ok "account forgotten, local history untouched"

step "A wrong password is rejected, and nothing is stored"
if COPYPASTE_CLOUD_PASSWORD="not-the-password" a cloud sign-in --email demo@example.com \
    >/dev/null 2>&1; then
    fail "a wrong password was accepted"
fi
grep -q "signed out" <<<"$(cloud_status a)" || fail "a failed sign-in left an account behind"
ok "refused, and no account persisted"

step "Done"
a cloud status
b cloud status
printf '\n\033[1;32mCloud-sync demo passed \033[1;33m(against a stub backend, not Supabase).\033[0m\n'
