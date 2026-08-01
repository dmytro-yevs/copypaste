#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Apply the migrations to a throwaway PostgreSQL cluster and run the assertions
# against it.
#
#   supabase/dev/verify-schema.sh
#
# What this proves: the SQL applies cleanly, the policies isolate accounts, the
# constraints hold, the pull query gets an index scan, and retention evicts by
# the server's clock. What it cannot prove: anything about GoTrue, PostgREST or
# Realtime — those are containers, and this script starts no containers. Use
# `supabase start` plus `supabase/dev/smoke.sh` for that half.
#
# Needs a PostgreSQL server installation (initdb, pg_ctl, psql). It touches no
# system cluster: everything lives in a temporary directory that is removed on
# exit, and the server listens on a unix socket inside it rather than on a port.
# ---------------------------------------------------------------------------
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"

bindir="${PG_BINDIR:-}"
if [[ -z "$bindir" ]]; then
    if command -v initdb >/dev/null 2>&1; then
        bindir="$(dirname "$(command -v initdb)")"
    else
        # Debian/Ubuntu keep the server binaries off PATH.
        bindir="$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -1 || true)"
    fi
fi
if [[ -z "$bindir" || ! -x "$bindir/initdb" ]]; then
    echo "verify-schema: no PostgreSQL server binaries found (set PG_BINDIR)" >&2
    exit 2
fi

# PostgreSQL refuses to run as root. Drop to an unprivileged account if needed.
run_as=()
if [[ "$(id -u)" == "0" ]]; then
    pg_user="${PG_RUN_AS:-postgres}"
    if ! id "$pg_user" >/dev/null 2>&1; then
        echo "verify-schema: running as root and no '$pg_user' account to drop to (set PG_RUN_AS)" >&2
        exit 2
    fi
    run_as=(runuser -u "$pg_user" --)
fi

run_pg() {
    if ((${#run_as[@]})); then
        "${run_as[@]}" "$@"
    else
        "$@"
    fi
}

workdir="$(mktemp -d /tmp/copypaste-verify-XXXXXX)"
datadir="$workdir/data"
sockdir="$workdir/sock"
mkdir -p "$datadir" "$sockdir"
if ((${#run_as[@]})); then
    chown -R "${PG_RUN_AS:-postgres}" "$workdir"
fi

cleanup() {
    run_pg "$bindir/pg_ctl" -D "$datadir" -m immediate stop >/dev/null 2>&1 || true
    rm -rf "$workdir"
}
trap cleanup EXIT

echo "verify-schema: initialising a throwaway cluster in $workdir"
run_pg "$bindir/initdb" -D "$datadir" -U copypaste_verify --auth=trust >"$workdir/initdb.log" 2>&1

# Socket only: no TCP port to collide with anything already running.
# `wal_level=logical` because the Realtime migration creates a publication, and
# Supabase's database runs at that level.
run_pg "$bindir/pg_ctl" -D "$datadir" -l "$workdir/server.log" \
    -o "-k $sockdir -h '' -c wal_level=logical -c fsync=off -c full_page_writes=off" \
    -w start >/dev/null

psql=(run_pg "$bindir/psql" -h "$sockdir" -U copypaste_verify -v ON_ERROR_STOP=1 -X -q)

"${psql[@]}" -d postgres -c "create database copypaste;" >/dev/null
db=("${psql[@]}" -d copypaste)

echo "verify-schema: stubbing auth.uid(), auth.users and the API roles"
"${db[@]}" -f "$here/harness/00-supabase-stubs.sql"

# One WARNING is expected here and is not a failure: pg_cron does not exist on a
# stock server, so the retention job is created but not scheduled.
echo "verify-schema: applying migrations"
for migration in "$root"/supabase/migrations/*.sql; do
    echo "  - $(basename "$migration")"
    "${db[@]}" -f "$migration"
done

# The two accounts the behavioural assertions expect. `seed.sql` cannot be used
# here: it writes the ~30 columns real GoTrue has, and the stub has three.
"${db[@]}" -c "
insert into auth.users (id, email) values
  ('11111111-1111-4111-8111-111111111111', 'dev-a@example.test'),
  ('22222222-2222-4222-8222-222222222222', 'dev-b@example.test')
on conflict do nothing;" >/dev/null

echo "verify-schema: running assertions"
status=0
for suite in "$root"/supabase/tests/*.sql; do
    if ! "${db[@]}" -f "$suite"; then
        echo "verify-schema: FAILED in $(basename "$suite")" >&2
        status=1
        break
    fi
done

if [[ $status -eq 0 ]]; then
    echo "verify-schema: all assertions passed"
else
    echo "verify-schema: see $workdir/server.log (removed on exit)" >&2
fi
exit $status
