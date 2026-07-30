#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Run `smoke.sh` against a real PostgREST on a throwaway PostgreSQL cluster,
# with no Docker and no Supabase CLI.
#
#   supabase/dev/postgrest-harness.sh              # needs a postgrest binary
#   PGRST_BIN=/path/to/postgrest supabase/dev/postgrest-harness.sh
#
# Why this exists: `supabase start` needs Docker, and where Docker is not
# available the assumptions in `smoke.sh` had never been executed at all — most
# importantly whether `?on_conflict=user_id,item_id` resolves when the request
# body does not contain `user_id`. That question is PostgREST's and Postgres',
# not GoTrue's or Realtime's, so it can be answered with the two of them alone.
#
# What this proves: everything in `smoke.sh` except the GoTrue sign-in, which is
# replaced by JWTs this script mints under PostgREST's own secret. Same
# migrations, same policies, same column grants, same request shapes.
#
# What it does not prove: that a hosted Supabase project behaves identically.
# Nothing in this repository has ever spoken to a real Supabase project. The
# platform's PostgREST is configured by Supabase and its GoTrue is not here at
# all.
#
# Needs: PostgreSQL server binaries (initdb, pg_ctl, psql), a `postgrest`
# binary, curl, jq, python3.
# ---------------------------------------------------------------------------
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"

PGRST_BIN="${PGRST_BIN:-$(command -v postgrest || true)}"
[[ -x "$PGRST_BIN" ]] || {
    echo "harness: no postgrest binary (set PGRST_BIN)" >&2
    echo "         releases: https://github.com/PostgREST/postgrest/releases" >&2
    exit 2
}
for tool in curl jq python3; do
    command -v "$tool" >/dev/null || { echo "harness: $tool is required" >&2; exit 2; }
done

bindir="${PG_BINDIR:-}"
if [[ -z "$bindir" ]]; then
    if command -v initdb >/dev/null 2>&1; then
        bindir="$(dirname "$(command -v initdb)")"
    else
        bindir="$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -1 || true)"
    fi
fi
[[ -x "$bindir/initdb" ]] || { echo "harness: no PostgreSQL server binaries (set PG_BINDIR)" >&2; exit 2; }

# PostgreSQL refuses to run as root; PostgREST does not care.
run_as=()
if [[ "$(id -u)" == "0" ]]; then
    pg_user="${PG_RUN_AS:-postgres}"
    id "$pg_user" >/dev/null 2>&1 || { echo "harness: no '$pg_user' account to drop to (set PG_RUN_AS)" >&2; exit 2; }
    run_as=(runuser -u "$pg_user" --)
fi

PG_PORT="${PG_PORT:-54329}"
REST_PORT="${REST_PORT:-54330}"
# Any 32+ byte string; both this script and PostgREST use it and nothing else
# ever sees it. It is not a Supabase key and must not be used as one.
JWT_SECRET="${JWT_SECRET:-copypaste-local-harness-secret-not-a-real-key}"

workdir="$(mktemp -d /tmp/copypaste-postgrest-XXXXXX)"
datadir="$workdir/data"
mkdir -p "$datadir"
[[ ${#run_as[@]} -eq 0 ]] || chown -R "${PG_RUN_AS:-postgres}" "$workdir"

cleanup() {
    [[ -n "${REST_PID:-}" ]] && kill "$REST_PID" 2>/dev/null
    "${run_as[@]}" "$bindir/pg_ctl" -D "$datadir" -m immediate stop >/dev/null 2>&1 || true
    rm -rf "$workdir"
}
trap cleanup EXIT

echo "harness: initialising a throwaway cluster in $workdir"
"${run_as[@]}" "$bindir/initdb" -D "$datadir" -U copypaste_verify --auth=trust >"$workdir/initdb.log" 2>&1
"${run_as[@]}" "$bindir/pg_ctl" -D "$datadir" -l "$workdir/server.log" \
    -o "-p $PG_PORT -h 127.0.0.1 -c wal_level=logical -c fsync=off -c full_page_writes=off" \
    -w start >/dev/null

psql=("${run_as[@]}" "$bindir/psql" -h 127.0.0.1 -p "$PG_PORT" -U copypaste_verify -v ON_ERROR_STOP=1 -X -q)
"${psql[@]}" -d postgres -c "create database copypaste;" >/dev/null
db=("${psql[@]}" -d copypaste)

echo "harness: stubbing auth.uid(), auth.users and the API roles"
"${db[@]}" -f "$here/harness/00-supabase-stubs.sql"

# The one role Supabase provides that the SQL stubs do not: PostgREST logs in as
# `authenticator` and switches to `anon` or to the JWT's `role` claim.
"${db[@]}" -c "
do \$\$ begin
    if not exists (select 1 from pg_roles where rolname = 'authenticator') then
        create role authenticator login noinherit;
    end if;
end \$\$;
grant anon, authenticated, service_role to authenticator;" >/dev/null

echo "harness: applying migrations"
for migration in "$root"/supabase/migrations/*.sql; do
    "${db[@]}" -f "$migration" >/dev/null
done

# The two accounts `smoke.sh` expects. `seed.sql` writes the ~30 columns real
# GoTrue has and the stub has three, so the rows go in directly.
alice_id="11111111-1111-4111-8111-111111111111"
bob_id="22222222-2222-4222-8222-222222222222"
"${db[@]}" -c "
insert into auth.users (id, email) values
  ('$alice_id', 'dev-a@example.test'),
  ('$bob_id',   'dev-b@example.test')
on conflict do nothing;" >/dev/null

echo "harness: starting PostgREST on 127.0.0.1:$REST_PORT"
PGRST_DB_URI="postgres://authenticator@127.0.0.1:$PG_PORT/copypaste" \
PGRST_DB_SCHEMAS="public" \
PGRST_DB_ANON_ROLE="anon" \
PGRST_JWT_SECRET="$JWT_SECRET" \
PGRST_SERVER_PORT="$REST_PORT" \
PGRST_LOG_LEVEL="error" \
"$PGRST_BIN" >"$workdir/postgrest.log" 2>&1 &
REST_PID=$!

for _ in $(seq 1 100); do
    curl -fsS -o /dev/null "http://127.0.0.1:$REST_PORT/" 2>/dev/null && break
    sleep 0.2
done
curl -fsS -o /dev/null "http://127.0.0.1:$REST_PORT/" || {
    echo "harness: PostgREST did not start" >&2
    cat "$workdir/postgrest.log" >&2
    exit 1
}

# HS256 JWTs, minted here because GoTrue is not part of this harness. `role` is
# what PostgREST switches to; `sub` is what `auth.uid()` reads.
mint() {
    python3 - "$JWT_SECRET" "$1" "$2" <<'PY'
import base64, hashlib, hmac, json, sys, time
secret, role, sub = sys.argv[1], sys.argv[2], sys.argv[3]
b64 = lambda raw: base64.urlsafe_b64encode(raw).rstrip(b"=").decode()
head = b64(json.dumps({"alg": "HS256", "typ": "JWT"}, separators=(",", ":")).encode())
claims = {"role": role, "exp": int(time.time()) + 3600}
if sub:
    claims["sub"] = sub
body = b64(json.dumps(claims, separators=(",", ":")).encode())
signing = f"{head}.{body}".encode()
sig = b64(hmac.new(secret.encode(), signing, hashlib.sha256).digest())
print(f"{head}.{body}.{sig}")
PY
}

export SUPABASE_URL="http://127.0.0.1:$REST_PORT"
export SUPABASE_ANON_KEY="$(mint anon '')"
export SMOKE_REST_BASE="http://127.0.0.1:$REST_PORT"   # PostgREST has no /rest/v1 prefix
export SMOKE_ALICE_TOKEN="$(mint authenticated "$alice_id")"
export SMOKE_BOB_TOKEN="$(mint authenticated "$bob_id")"

echo "harness: running supabase/dev/smoke.sh against it"
echo
"$here/smoke.sh"
