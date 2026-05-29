#!/usr/bin/env bash
# e2e.sh — isolated end-to-end harness that wraps scripts/smoke_test.sh and
#          GUARANTEES teardown of everything it spawns.
#
# Usage:  bash scripts/e2e.sh [--skip-build] [smoke-test-args...]
#         All extra args are passed straight through to scripts/smoke_test.sh
#         (the only one it understands today is --skip-build).
#
# ---------------------------------------------------------------------------
# WHY THIS RUNS SERIALLY (one invocation at a time, never in parallel):
#
#   This is a network/multi-process integration test. It launches a real
#   copypaste-daemon that:
#     * binds a fixed Unix domain socket (daemon.sock),
#     * opens an exclusive SQLCipher database (clipboard.db, WAL mode),
#     * (optionally) talks to a single local Supabase stack on fixed ports,
#     * mutates the shared macOS pasteboard via pbcopy.
#
#   Two copies running at once would contend on the same socket path, the
#   same DB file/WAL lock, the same Supabase ports, and the same single
#   system clipboard — producing flaky cross-talk and corrupt state. So CI
#   and humans must run e2e tests SERIALLY. This script isolates per-run
#   state into private temp dirs, but the system clipboard and any local
#   Supabase ports remain global, hence: one at a time.
# ---------------------------------------------------------------------------
#
# What it does:
#   1. Defensively kills any leftover daemon from a prior crashed run.
#   2. Builds a fully isolated sandbox HOME + COPYPASTE_* env so the smoke
#      test never touches your real ~/Library/Application Support/CopyPaste.
#   3. Runs scripts/smoke_test.sh (passing through args).
#   4. On EXIT/INT/TERM — even on failure — a trap tears EVERYTHING down:
#      the daemon binary we launched, any local Supabase stack we started,
#      the sandbox temp dirs, and any stale socket. A re-run starts clean.

set -euo pipefail

# ---------------------------------------------------------------------------
# Locate repo root (this script lives in <repo>/scripts/).
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

SMOKE_TEST="$REPO_ROOT/scripts/smoke_test.sh"
DAEMON_BIN="$REPO_ROOT/target/release/copypaste-daemon"

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[e2e]${NC} $*"; }
ok()    { echo -e "${GREEN}[e2e] OK${NC}: $*"; }
warn()  { echo -e "${YELLOW}[e2e] WARN${NC}: $*" >&2; }
# die NAMES the failing setup step, loudly, then exits (which fires the trap).
die()   { echo -e "${RED}[e2e] FAIL at step: $*${NC}" >&2; exit 1; }

# ---------------------------------------------------------------------------
# State the trap needs. Declared up front so the trap is always well-defined,
# even if we die before they are populated.
# ---------------------------------------------------------------------------
SANDBOX=""                 # root temp dir for this run
SUPABASE_STARTED=false     # true only if WE ran `supabase start` here
SUPABASE_DIR=""            # dir from which we ran supabase (for `supabase stop`)

# The exact daemon binary path the smoke test launches. pkill -f against this
# specific absolute path only ever matches a daemon WE spawned from this repo —
# it will not nuke an unrelated installed daemon elsewhere.
DAEMON_PKILL_PATTERN="$DAEMON_BIN"

# ---------------------------------------------------------------------------
# kill_stray_daemons — terminate any copypaste-daemon launched from THIS repo's
# release binary. Used both defensively at startup and in cleanup. Sends TERM,
# waits briefly, then KILLs survivors. Never errors out (best-effort).
# ---------------------------------------------------------------------------
kill_stray_daemons() {
  # Match only the daemon binary built from this repo, by absolute path.
  if pgrep -f "$DAEMON_PKILL_PATTERN" >/dev/null 2>&1; then
    pkill -TERM -f "$DAEMON_PKILL_PATTERN" 2>/dev/null || true
    # Give it up to ~2s to exit cleanly.
    for _ in 1 2 3 4 5 6 7 8 9 10; do
      pgrep -f "$DAEMON_PKILL_PATTERN" >/dev/null 2>&1 || break
      sleep 0.2
    done
    # Hard-kill anything still standing.
    pkill -KILL -f "$DAEMON_PKILL_PATTERN" 2>/dev/null || true
  fi
}

# ---------------------------------------------------------------------------
# cleanup — the GUARANTEE. Runs on EXIT (success OR failure) and on INT/TERM.
# Tears down, in order:
#   1. the daemon process(es) we launched,
#   2. any local Supabase stack we started,
#   3. stale sockets in the sandbox,
#   4. the sandbox temp dirs.
# Every action is best-effort so one failure never blocks the rest.
# ---------------------------------------------------------------------------
cleanup() {
  local rc=$?
  # Prevent re-entrancy (INT during cleanup, etc.).
  trap - EXIT INT TERM
  echo ""
  info "cleanup: tearing down e2e environment (exit code was $rc)"

  # 1. Daemon(s) we spawned.
  kill_stray_daemons
  ok "cleanup: no e2e daemon processes remain"

  # 2. Local Supabase stack, only if WE started it.
  if [[ "$SUPABASE_STARTED" == "true" ]]; then
    info "cleanup: stopping local Supabase stack"
    ( cd "$SUPABASE_DIR" 2>/dev/null && supabase stop --no-backup >/dev/null 2>&1 ) || true
    ok "cleanup: Supabase stack stopped"
  fi

  # 3. Stale socket(s) in the sandbox (the daemon should remove its own, but
  #    be defensive in case it was hard-killed).
  if [[ -n "${COPYPASTE_SOCKET:-}" && -e "$COPYPASTE_SOCKET" ]]; then
    rm -f "$COPYPASTE_SOCKET" 2>/dev/null || true
  fi

  # 4. Sandbox temp dirs.
  if [[ -n "$SANDBOX" && -d "$SANDBOX" ]]; then
    rm -rf "$SANDBOX" 2>/dev/null || true
    ok "cleanup: removed sandbox $SANDBOX"
  fi

  info "cleanup: done"
  exit "$rc"
}
# Fire on normal exit, on the `exit 1` from die(), and on Ctrl-C / kill.
trap cleanup EXIT INT TERM

# ===========================================================================
# Pre-flight
# ===========================================================================

# Smoke test must exist and be readable — this is what we wrap.
[[ -f "$SMOKE_TEST" ]] || die "locate-smoke-test (missing $SMOKE_TEST)"

# Warn (do not block) on a dirty tree: e2e mutates nothing tracked, but a dirty
# tree means the binaries built may not match committed source.
if git -C "$REPO_ROOT" rev-parse --git-dir >/dev/null 2>&1; then
  if ! git -C "$REPO_ROOT" diff --quiet || ! git -C "$REPO_ROOT" diff --cached --quiet; then
    warn "git tree is DIRTY — e2e will run, but built binaries may not match committed source"
  fi
fi

# Defensive: a previous crashed run may have left a daemon (and its socket)
# alive. Kill it now so this run starts from a known-clean slate.
info "pre-flight: killing any leftover e2e daemons from prior runs"
kill_stray_daemons

# ===========================================================================
# Build the isolated sandbox.
#
# CRITICAL: smoke_test.sh hardcodes "$HOME/Library/Application Support/CopyPaste"
# (it has no env override of its own). The daemon, however, resolves that path
# from home::home_dir() (i.e. $HOME) AND additionally honours COPYPASTE_SOCKET /
# COPYPASTE_DB / COPYPASTE_DATA_DIR / COPYPASTE_CONFIG_DIR. So to fully isolate
# BOTH the script's bookkeeping and the daemon's real I/O, we:
#   * point HOME at a sandbox (redirects the hardcoded macOS support path), and
#   * set the COPYPASTE_* overrides explicitly (belt and suspenders), and
#   * set COPYPASTE_EPHEMERAL_KEY=1 so the daemon never touches the real macOS
#     Keychain (uses an in-memory device keypair instead).
# ===========================================================================
SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-e2e.XXXXXX")" \
  || die "create-sandbox (mktemp -d failed)"
[[ -d "$SANDBOX" ]] || die "create-sandbox (sandbox dir missing after mktemp)"

# Mirror the macOS layout the smoke test expects, under the sandbox HOME.
SANDBOX_HOME="$SANDBOX/home"
SANDBOX_SUPPORT="$SANDBOX_HOME/Library/Application Support/CopyPaste"
SANDBOX_DATA="$SANDBOX/data"
SANDBOX_CONFIG="$SANDBOX/config"

mkdir -p "$SANDBOX_SUPPORT" "$SANDBOX_DATA" "$SANDBOX_CONFIG" \
  || die "create-sandbox-dirs (mkdir failed under $SANDBOX)"

ok "sandbox ready: $SANDBOX"

# Export the isolated environment for the smoke test + daemon it spawns.
#
# IMPORTANT: smoke_test.sh derives its socket/db paths from HOME
# ("$HOME/Library/Application Support/CopyPaste/{daemon.sock,clipboard.db}")
# and has no env override of its own. We therefore pin the daemon's
# COPYPASTE_SOCKET / COPYPASTE_DB to the SAME support dir under the sandbox
# HOME so the daemon and the smoke test agree on where the socket/db live —
# otherwise the daemon would listen on one path while the test polls another
# and "socket not ready" would (correctly) fail.
export HOME="$SANDBOX_HOME"
export COPYPASTE_DATA_DIR="$SANDBOX_DATA"
export COPYPASTE_CONFIG_DIR="$SANDBOX_CONFIG"
export COPYPASTE_SOCKET="$SANDBOX_SUPPORT/daemon.sock"
export COPYPASTE_DB="$SANDBOX_SUPPORT/clipboard.db"
export COPYPASTE_EPHEMERAL_KEY=1
# Keep the daemon quiet but surface real errors.
export RUST_LOG="${RUST_LOG:-error}"

info "isolated env:"
info "  HOME                 = $HOME"
info "  COPYPASTE_SOCKET     = $COPYPASTE_SOCKET"
info "  COPYPASTE_DB         = $COPYPASTE_DB"
info "  COPYPASTE_DATA_DIR   = $COPYPASTE_DATA_DIR"
info "  COPYPASTE_CONFIG_DIR = $COPYPASTE_CONFIG_DIR"
info "  COPYPASTE_EPHEMERAL_KEY = $COPYPASTE_EPHEMERAL_KEY"

# ===========================================================================
# OPTIONAL: bring up a local Supabase stack — ONLY if the CLI is installed.
# If absent, we SKIP loudly (we never fake a cloud backend).
# ===========================================================================
if [[ -d "$REPO_ROOT/supabase" ]] && command -v supabase >/dev/null 2>&1; then
  info "Supabase CLI + ./supabase config found — starting local stack"
  SUPABASE_DIR="$REPO_ROOT"
  if ( cd "$SUPABASE_DIR" && supabase start >/dev/null 2>&1 ); then
    SUPABASE_STARTED=true
    ok "local Supabase stack started (will be stopped on exit)"
  else
    # Not fatal to the smoke test (it is P2P/IPC-centric); warn and move on.
    warn "supabase start failed — continuing WITHOUT a local Supabase stack"
  fi
elif command -v supabase >/dev/null 2>&1; then
  info "Supabase CLI present but no ./supabase project dir — SKIPPING local stack"
else
  info "Supabase CLI not installed — SKIPPING local stack (cloud-sync E2E needs a real Supabase)"
fi

# ===========================================================================
# Run the real smoke test, passing through args (e.g. --skip-build).
# ===========================================================================
info "running scripts/smoke_test.sh ${*:-(no args)}"
echo "---------------------------------------------------------------------"
if bash "$SMOKE_TEST" "$@"; then
  echo "---------------------------------------------------------------------"
  ok "smoke test PASSED"
else
  rc=$?
  echo "---------------------------------------------------------------------"
  # die() exits non-zero -> EXIT trap fires -> full cleanup runs anyway.
  die "smoke-test (scripts/smoke_test.sh exited $rc)"
fi

# Normal success path. The EXIT trap still runs cleanup after this.
info "e2e harness finished successfully"
exit 0
