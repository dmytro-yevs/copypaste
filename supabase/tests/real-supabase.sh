#!/usr/bin/env bash
set -euo pipefail

for tool in supabase jq psql cargo; do
    command -v "$tool" >/dev/null || { echo "real-supabase: $tool is required" >&2; exit 2; }
done

started=false
cleanup() {
    if [[ "$started" == true ]]; then
        supabase stop --no-backup >/dev/null
    fi
}
trap cleanup EXIT

supabase start
started=true
supabase db reset

status="$(supabase status -o json)"
SUPABASE_URL="$(jq -er '.API_URL' <<<"$status")"
SUPABASE_ANON_KEY="$(jq -er '.ANON_KEY' <<<"$status")"
export SUPABASE_URL SUPABASE_ANON_KEY
database_url="$(jq -er '.DB_URL' <<<"$status")"

for test in supabase/tests/01_schema_audit.sql \
            supabase/tests/02_rls_behaviour.sql \
            supabase/tests/03_retention_and_paging.sql; do
    psql "$database_url" -v ON_ERROR_STOP=1 -f "$test"
done

cargo test -p copypaste-cloud --test real_supabase -- --ignored --exact real_supabase_contract
