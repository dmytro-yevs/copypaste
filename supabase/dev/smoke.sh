#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# End-to-end smoke test against a running local stack.
#
#   supabase start          # needs Docker
#   supabase/dev/smoke.sh
#
# ** UNVERIFIED. ** This script has never been executed: the container this was
# written in has a Docker client but no daemon, so `supabase start` cannot run
# here. Every request below is transcribed from the client
# (`crates/copypaste-cloud/src/{auth,rest}/`) rather than from a captured
# session. Treat a first run as a test of this script as much as of the schema.
#
# It exercises the four things the assertions in `supabase/tests/` cannot,
# because they need PostgREST and GoTrue rather than SQL:
#
#   1. that `?on_conflict=user_id,item_id` resolves when the body does not
#      contain `user_id` — the single most load-bearing untested assumption in
#      the whole deployment, and a 409 if it is wrong;
#   2. that the `select=`, `order=` and `created_at=gte.` query string the
#      client sends is accepted verbatim;
#   3. that the PATCH tombstone reaches exactly the rows it names;
#   4. that the publishable key on its own reaches nothing.
#
# Needs: curl, jq.
# ---------------------------------------------------------------------------
set -euo pipefail

for tool in curl jq; do
    command -v "$tool" >/dev/null || { echo "smoke: $tool is required" >&2; exit 2; }
done

URL="${SUPABASE_URL:-http://127.0.0.1:54321}"
ANON="${SUPABASE_ANON_KEY:-}"
if [[ -z "$ANON" ]]; then
    if command -v supabase >/dev/null; then
        ANON="$(supabase status -o json 2>/dev/null | jq -r '.ANON_KEY // empty')"
    fi
fi
[[ -n "$ANON" ]] || { echo "smoke: set SUPABASE_ANON_KEY (or run inside a project with the CLI)" >&2; exit 2; }

REST="$URL/rest/v1/clipboard_items"
fail() { echo "smoke: FAIL — $*" >&2; exit 1; }

sign_in() { # email -> access token
    curl -sS -X POST "$URL/auth/v1/token?grant_type=password" \
        -H "apikey: $ANON" -H 'Content-Type: application/json' \
        -d "{\"email\":\"$1\",\"password\":\"copypaste-dev\"}" \
    | jq -er '.access_token' || fail "could not sign in as $1 (is the seed applied?)"
}

alice="$(sign_in dev-a@example.test)"
bob="$(sign_in dev-b@example.test)"
echo "smoke: signed in both development accounts"

item="smoke-$(date +%s)"
now_ms="$(( $(date +%s) * 1000 ))"

# The payload is not real ciphertext — this script holds no sync passphrase and
# must not: a server-side key is the one thing this design does not have. A
# client will refuse to decrypt these rows and will count them as skipped, which
# is the correct behaviour (INV-N3).
row() {
    jq -nc --arg id "$item" --arg ct "$1" --argjson at "$2" '[{
        item_id: $id, ciphertext: $ct, nonce: "bm9uY2U=", content_type: "text",
        created_at: $at, deleted: false, origin_device_id: "smoke-device"
    }]'
}

upsert() { # token, body
    curl -sS -o /dev/null -w '%{http_code}' -X POST "$REST?on_conflict=user_id,item_id" \
        -H "apikey: $ANON" -H "Authorization: Bearer $1" \
        -H 'Content-Type: application/json' \
        -H 'Prefer: resolution=merge-duplicates,return=minimal' \
        -d "$2"
}

code="$(upsert "$alice" "$(row 'Zmlyc3Q=' "$now_ms")")"
[[ "$code" =~ ^(200|201|204)$ ]] || fail "first upsert returned $code"

# The replay. A 409 here means the unique index on (user_id, item_id) is not
# resolving the conflict target — exactly what `RestError::Rejected { 409 }`
# reports and what manifest 05 §4.2 records as the schema gap.
code="$(upsert "$alice" "$(row 'c2Vjb25k' "$((now_ms + 1))")")"
[[ "$code" =~ ^(200|201|204)$ ]] || fail "the replayed upsert returned $code (409 = missing unique index)"
echo "smoke: upsert is idempotent without the client ever sending user_id"

# The read path, character for character as `fetch_since` builds it.
page="$(curl -sS -G "$REST" \
    -H "apikey: $ANON" -H "Authorization: Bearer $alice" -H 'Accept: application/json' \
    --data-urlencode 'select=item_id,ciphertext,nonce,content_type,created_at,deleted,origin_device_id' \
    --data-urlencode "created_at=gte.$now_ms" \
    --data-urlencode 'order=created_at.asc,item_id.asc' \
    --data-urlencode 'limit=100')"

echo "$page" | jq -e --arg id "$item" 'map(select(.item_id == $id)) | length == 1' >/dev/null \
    || fail "the row did not come back exactly once: $page"
echo "$page" | jq -e 'map(keys | sort) | all(. == ["ciphertext","content_type","created_at","deleted","item_id","nonce","origin_device_id"])' >/dev/null \
    || fail "the page carries columns the client does not model: $page"
echo "smoke: the cursor page has exactly the seven columns the client parses"

# Cross-account. This is the assertion the whole RLS migration exists for.
others="$(curl -sS -G "$REST" \
    -H "apikey: $ANON" -H "Authorization: Bearer $bob" -H 'Accept: application/json' \
    --data-urlencode 'select=item_id' --data-urlencode 'limit=1000')"
[[ "$(echo "$others" | jq 'length')" == "0" ]] || fail "one account can read another's rows: $others"
echo "smoke: the second account sees none of the first account's rows"

# The publishable key on its own. `anon` holds no privilege and no policy, so
# this must not be a 200 with an empty array — that would mean a policy exists
# and simply matched nothing.
code="$(curl -sS -o /dev/null -w '%{http_code}' -G "$REST" \
    -H "apikey: $ANON" -H "Authorization: Bearer $ANON" \
    --data-urlencode 'select=item_id')"
[[ "$code" == "401" || "$code" == "403" ]] || fail "the anon key alone got $code, expected 401/403"
echo "smoke: the publishable key alone is refused"

# The tombstone, as `SupabaseRest::tombstone` sends it: a PATCH that wipes the
# payload and restamps `created_at` so the deletion sorts above every device's
# watermark.
code="$(curl -sS -o /dev/null -w '%{http_code}' -X PATCH "$REST?item_id=in.($item)" \
    -H "apikey: $ANON" -H "Authorization: Bearer $alice" \
    -H 'Content-Type: application/json' -H 'Prefer: return=minimal' \
    -d "{\"deleted\":true,\"ciphertext\":null,\"nonce\":null,\"created_at\":$((now_ms + 2))}")"
[[ "$code" =~ ^(200|204)$ ]] || fail "the tombstone PATCH returned $code"

state="$(curl -sS -G "$REST" \
    -H "apikey: $ANON" -H "Authorization: Bearer $alice" -H 'Accept: application/json' \
    --data-urlencode 'select=item_id,ciphertext,deleted' \
    --data-urlencode "item_id=eq.$item")"
echo "$state" | jq -e '.[0].deleted == true and (.[0].ciphertext | not)' >/dev/null \
    || fail "the tombstone did not wipe the payload: $state"
echo "smoke: the tombstone is a row version, and it carries no ciphertext"

# Clean up after ourselves — these rows are undecryptable noise for any client
# signed into the same local account.
curl -sS -o /dev/null -X DELETE "$REST?item_id=eq.$item" \
    -H "apikey: $ANON" -H "Authorization: Bearer $alice"

echo "smoke: all checks passed"
echo
echo "Realtime is not covered here. To watch it by hand:"
echo "  websocat \"ws://127.0.0.1:54321/realtime/v1/websocket?vsn=1.0.0&apikey=\$SUPABASE_ANON_KEY\""
echo "  then send the phx_join frame from crates/copypaste-cloud/src/realtime/channel.rs"
echo "  with your access token and filter, and upsert a row from another shell."
