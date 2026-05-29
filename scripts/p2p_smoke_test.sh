#!/usr/bin/env bash
# p2p_smoke_test.sh — REAL two-process P2P clipboard-sync acceptance test.
#
# Spins up TWO fully isolated `copypaste-daemon` subprocesses (separate socket,
# DB, data/config/cache/log dirs, ephemeral key, P2P enabled), drives the REAL
# production pairing path over the network bootstrap PAKE flow
# (pair_generate_qr on A -> pair_accept_qr {qr} on B), imports a known plaintext
# on A, and asserts it appears AND decrypts to the same plaintext on B's history
# within a timeout. A negative control (unpaired daemon C) must never receive it.
#
# This is the test that would have caught cross-device sync failures: it exercises
# the shipped binaries end-to-end, two real processes, real mTLS link, real DB.
#
# It mirrors crates/copypaste-daemon/tests/p2p_sync_e2e.rs but against the actual
# built binaries (not the cargo-test harness), so it catches build-skew too.
#
# Usage:
#   bash scripts/p2p_smoke_test.sh [--skip-build] [--from-bundle <CopyPaste.app>]
#
# Requirements: macOS or Linux, Rust toolchain (unless --skip-build/--from-bundle),
#               nc (netcat with -U), base64, sed/grep. jq optional (not required).

set -euo pipefail

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
SKIP_BUILD=false
FROM_BUNDLE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=true; shift ;;
    --from-bundle) FROM_BUNDLE="${2:?--from-bundle needs a path}"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# ---------------------------------------------------------------------------
# Resolve the daemon binary (release build, or inside a .app bundle)
# ---------------------------------------------------------------------------
if [[ -n "$FROM_BUNDLE" ]]; then
  DAEMON_BIN="$FROM_BUNDLE/Contents/MacOS/copypaste-daemon"
  [[ -x "$DAEMON_BIN" ]] || { echo -e "${RED}FAIL: daemon not found in bundle: $DAEMON_BIN${NC}" >&2; exit 1; }
  echo "Using daemon from bundle: $DAEMON_BIN"
else
  DAEMON_BIN="$REPO_ROOT/target/release/copypaste-daemon"
fi

# ---------------------------------------------------------------------------
# Per-daemon state (parallel arrays, indexed by slot 0/1/2 = A/B/C)
# ---------------------------------------------------------------------------
PIDS=()
ROOTS=()
SOCKS=()
STDERRS=()

# ---------------------------------------------------------------------------
# Cleanup — kill every spawned daemon, remove every temp root. Always runs.
# ---------------------------------------------------------------------------
cleanup() {
  local pid
  for pid in "${PIDS[@]:-}"; do
    [[ -n "$pid" ]] || continue
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  done
  local root
  for root in "${ROOTS[@]:-}"; do
    [[ -n "$root" && -d "$root" ]] || continue
    rm -rf "$root" 2>/dev/null || true
  done
}
trap 'cleanup' EXIT

fail() {
  echo -e "${RED}FAIL: $*${NC}" >&2
  # Dump each daemon's stderr to aid diagnosis of the real failure.
  local i
  for i in "${!STDERRS[@]}"; do
    local label="$i"
    case "$i" in 0) label=A ;; 1) label=B ;; 2) label=C ;; esac
    if [[ -n "${STDERRS[$i]:-}" && -s "${STDERRS[$i]}" ]]; then
      echo -e "${YELLOW}--- daemon $label stderr (${STDERRS[$i]}) ---${NC}" >&2
      tail -n 60 "${STDERRS[$i]}" >&2 || true
    fi
  done
  exit 1
}

run_step() { echo -e "${YELLOW}STEP${NC}: $*"; }
pass_step() { echo -e "${GREEN}  OK${NC}: $*"; }

# ---------------------------------------------------------------------------
# ipc <socket> <json>  — send one newline-delimited JSON-RPC request, print reply
# ---------------------------------------------------------------------------
ipc() {
  local sock="$1" body="$2"
  printf '%s\n' "$body" | nc -U "$sock" -w 5 2>/dev/null
}

# json_field <json> <dotted.path>  — extract a string scalar without jq.
# Supports a small set of paths used here: data.fingerprint, data.qr,
# data.inserted, ok. Uses grep/sed; tolerant of whitespace.
json_str() {
  # $1 json, $2 key (last key in the path)
  echo "$1" | grep -o "\"$2\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" \
    | head -1 | sed -E "s/.*\"$2\"[[:space:]]*:[[:space:]]*\"([^\"]*)\".*/\1/"
}

# ---------------------------------------------------------------------------
# STEP 1 — Build (or trust prebuilt) daemon
# ---------------------------------------------------------------------------
run_step "Resolve daemon binary"
if [[ -z "$FROM_BUNDLE" ]]; then
  if [[ "$SKIP_BUILD" == "true" && -x "$DAEMON_BIN" ]]; then
    pass_step "Skipping build (--skip-build, binary present)"
  else
    ( cd "$REPO_ROOT" && cargo build --release -p copypaste-daemon ) \
      || fail "cargo build of copypaste-daemon failed"
  fi
fi
[[ -x "$DAEMON_BIN" ]] || fail "daemon binary not found/executable: $DAEMON_BIN"
pass_step "Daemon: $DAEMON_BIN"

# ---------------------------------------------------------------------------
# spawn_daemon <slot>  — launch one isolated P2P-enabled daemon, wait for socket.
# Populates PIDS/ROOTS/SOCKS/STDERRS at <slot>.
# ---------------------------------------------------------------------------
spawn_daemon() {
  local slot="$1"
  local root sock db data cfg cache logd devid stderr
  root="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-p2p-${slot}.XXXXXX")"
  sock="$root/copypaste.sock"
  db="$root/clipboard.db"
  data="$root/data"
  cfg="$root/config"
  cache="$root/cache"
  logd="$root/logs"
  devid="$root/device_id"
  stderr="$root/daemon.stderr"
  mkdir -p "$data" "$cfg" "$cache" "$logd"

  COPYPASTE_SOCKET="$sock" \
  COPYPASTE_DB="$db" \
  COPYPASTE_DATA_DIR="$data" \
  COPYPASTE_CONFIG_DIR="$cfg" \
  COPYPASTE_CACHE_DIR="$cache" \
  COPYPASTE_LOG_DIR="$logd" \
  COPYPASTE_DEVICE_ID_PATH="$devid" \
  COPYPASTE_EPHEMERAL_KEY=1 \
  COPYPASTE_P2P=1 \
  RUST_LOG="${RUST_LOG:-error}" \
    "$DAEMON_BIN" >/dev/null 2>"$stderr" &
  local pid=$!

  PIDS[$slot]="$pid"
  ROOTS[$slot]="$root"
  SOCKS[$slot]="$sock"
  STDERRS[$slot]="$stderr"

  # Wait for the IPC socket to bind AND answer a status round-trip.
  local deadline=$(( $(date +%s) + 30 ))
  while (( $(date +%s) < deadline )); do
    if ! kill -0 "$pid" 2>/dev/null; then
      fail "daemon (slot $slot) exited before its socket came up"
    fi
    if [[ -S "$sock" ]]; then
      local r
      r="$(ipc "$sock" '{"id":"rdy","method":"status","params":{}}')"
      if echo "$r" | grep -q '"ok":true'; then
        return 0
      fi
    fi
    sleep 0.2
  done
  fail "daemon (slot $slot) socket not ready within 30s"
}

# ---------------------------------------------------------------------------
# STEP 2 — Spawn daemons A, B and unpaired control C
# ---------------------------------------------------------------------------
run_step "Spawn isolated P2P daemons A, B, C"
spawn_daemon 0; pass_step "A up (pid ${PIDS[0]}) socket ${SOCKS[0]}"
spawn_daemon 1; pass_step "B up (pid ${PIDS[1]}) socket ${SOCKS[1]}"
spawn_daemon 2; pass_step "C up (pid ${PIDS[2]}) socket ${SOCKS[2]} [unpaired control]"

SOCK_A="${SOCKS[0]}"; SOCK_B="${SOCKS[1]}"; SOCK_C="${SOCKS[2]}"
CFG_A="${ROOTS[0]}/config"; CFG_B="${ROOTS[1]}/config"
PEERS_A="$CFG_A/copypaste/peers.json"
PEERS_B="$CFG_B/copypaste/peers.json"

# ---------------------------------------------------------------------------
# STEP 3 — Sanity: both expose an mTLS cert fingerprint (P2P actually enabled)
# ---------------------------------------------------------------------------
run_step "Verify P2P certificates present (get_own_fingerprint)"
FP_A_RESP="$(ipc "$SOCK_A" '{"id":"fa","method":"get_own_fingerprint","params":{}}')"
echo "$FP_A_RESP" | grep -q '"ok":true' \
  || fail "A get_own_fingerprint not ok (is COPYPASTE_P2P honored?): $FP_A_RESP"
FP_A="$(json_str "$FP_A_RESP" fingerprint)"
[[ -n "$FP_A" ]] || fail "A fingerprint empty: $FP_A_RESP"
pass_step "A cert fingerprint: $FP_A"

FP_B_RESP="$(ipc "$SOCK_B" '{"id":"fb","method":"get_own_fingerprint","params":{}}')"
echo "$FP_B_RESP" | grep -q '"ok":true' || fail "B get_own_fingerprint not ok: $FP_B_RESP"
FP_B="$(json_str "$FP_B_RESP" fingerprint)"
[[ -n "$FP_B" ]] || fail "B fingerprint empty: $FP_B_RESP"
pass_step "B cert fingerprint: $FP_B"

# ---------------------------------------------------------------------------
# STEP 4 — REAL production pairing: A generates QR, B accepts it over network.
# This is the exact bootstrap PAKE path the shipped app uses (pair_accept_qr
# with a {qr} param dials A's addr_hint and runs the initiator handshake).
# ---------------------------------------------------------------------------
run_step "Pair A <-> B over network bootstrap PAKE (pair_generate_qr / pair_accept_qr)"
QR_RESP="$(ipc "$SOCK_A" '{"id":"qa","method":"pair_generate_qr","params":{}}')"
echo "$QR_RESP" | grep -q '"ok":true' || fail "pair_generate_qr on A failed: $QR_RESP"
QR="$(json_str "$QR_RESP" qr)"
[[ -n "$QR" ]] || fail "QR string empty: $QR_RESP"
pass_step "A generated pairing QR (${#QR} chars)"

# Build the accept request with the QR embedded. The QR payload contains no
# double-quote chars (it is a single base64url-ish token string), so embedding
# it directly in JSON is safe; assert that to be defensive.
case "$QR" in
  *'"'*) fail "QR string unexpectedly contains a double-quote; cannot embed safely: $QR" ;;
esac
ACCEPT_BODY="{\"id\":\"qb\",\"method\":\"pair_accept_qr\",\"params\":{\"qr\":\"$QR\"}}"
ACCEPT_RESP="$(ipc "$SOCK_B" "$ACCEPT_BODY")"
echo "$ACCEPT_RESP" | grep -q '"ok":true' \
  || fail "network PAKE pairing (pair_accept_qr) failed: $ACCEPT_RESP"
pass_step "B accepted QR; PAKE handshake reported ok"

# ---------------------------------------------------------------------------
# STEP 5 — Both sides must PERSIST the peer with a shared sync key.
# Without the shared content sync key, B could never decrypt A's items.
# ---------------------------------------------------------------------------
run_step "Wait for both peers.json to carry a shared sync_key_b64"
wait_for_synckey() {
  local file="$1" deadline=$(( $(date +%s) + 15 ))
  while (( $(date +%s) < deadline )); do
    if [[ -f "$file" ]] && grep -q '"sync_key_b64"' "$file" 2>/dev/null; then
      return 0
    fi
    sleep 0.2
  done
  return 1
}
wait_for_synckey "$PEERS_A" || fail "A's peers.json never got a sync_key_b64 (path: $PEERS_A)"
wait_for_synckey "$PEERS_B" || fail "B's peers.json never got a sync_key_b64 (path: $PEERS_B)"

# Extract the shared key from each and confirm they CONVERGED to the same value.
KEY_A="$(json_str "$(cat "$PEERS_A")" sync_key_b64)"
KEY_B="$(json_str "$(cat "$PEERS_B")" sync_key_b64)"
[[ -n "$KEY_A" && -n "$KEY_B" ]] || fail "sync_key_b64 empty on one side (A='$KEY_A' B='$KEY_B')"
[[ "$KEY_A" == "$KEY_B" ]] \
  || fail "both daemons must derive the SAME shared sync key; A=$KEY_A B=$KEY_B"
pass_step "Shared content sync key established and converged on both sides"

# ---------------------------------------------------------------------------
# STEP 6 — Import a KNOWN plaintext on A (encrypts + broadcasts into sync).
# ---------------------------------------------------------------------------
run_step "Import a known plaintext item on A"
PLAINTEXT="p2p-smoke-secret-$(date +%s)-2f9a1c7e"
PLAINTEXT_B64="$(printf '%s' "$PLAINTEXT" | base64 | tr -d '\n')"
IMPORT_BODY="{\"id\":\"imp\",\"method\":\"import\",\"params\":{\"items\":[{\"content_type\":\"text\",\"content_bytes_b64\":\"$PLAINTEXT_B64\",\"created_at_ms\":1700000123456}]}}"
IMPORT_RESP="$(ipc "$SOCK_A" "$IMPORT_BODY")"
echo "$IMPORT_RESP" | grep -q '"ok":true' || fail "import on A failed: $IMPORT_RESP"
echo "$IMPORT_RESP" | grep -q '"inserted":1' || fail "A did not insert the item: $IMPORT_RESP"
pass_step "A imported plaintext: $PLAINTEXT"

# ---------------------------------------------------------------------------
# STEP 7 — Assert it appears (decrypted) on B's history within a timeout.
# ---------------------------------------------------------------------------
run_step "Wait for the item to sync to B (decrypted to same plaintext)"
wait_for_plaintext() {
  local sock="$1" want="$2" timeout="$3"
  local deadline=$(( $(date +%s) + timeout ))
  while (( $(date +%s) < deadline )); do
    local resp
    resp="$(ipc "$sock" '{"id":"hp","method":"history_page","params":{"limit":50,"offset":0}}')"
    if echo "$resp" | grep -qF "$want"; then
      return 0
    fi
    sleep 0.5
  done
  return 1
}
if wait_for_plaintext "$SOCK_B" "$PLAINTEXT" 30; then
  pass_step "B received AND decrypted A's item over live two-process P2P"
else
  fail "B did NOT receive A's clipboard item within 30s — TWO-PROCESS P2P SYNC IS BROKEN"
fi

# ---------------------------------------------------------------------------
# STEP 8 — Negative control: unpaired C must NOT receive it.
# ---------------------------------------------------------------------------
run_step "Negative control: unpaired daemon C must NOT have the item"
if wait_for_plaintext "$SOCK_C" "$PLAINTEXT" 3; then
  fail "SECURITY: unpaired daemon C received A's clipboard item — leak!"
fi
pass_step "Unpaired C correctly did not receive the item"

echo ""
echo -e "${GREEN}PASS: two-process P2P sync smoke completed${NC}"
exit 0
