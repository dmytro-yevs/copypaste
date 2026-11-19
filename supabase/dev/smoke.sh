#!/usr/bin/env bash
# End-to-end smoke test against a running local stack.
#
#   supabase start          # needs Docker
#   supabase/dev/smoke.sh
#
# ** NEVER RUN AGAINST SUPABASE. ** Every request below is transcribed from the
# client (`crates/copypaste-cloud/src/{auth,rest}/`), and no part of this
# repository has ever spoken to a hosted Supabase project.
#
# It *has* now been run end to end against a real PostgREST 12.2.3 on a stock
# PostgreSQL 16, by `supabase/dev/postgrest-harness.sh`, which needs neither
# Docker nor the Supabase CLI. Everything below passed there except the GoTrue
# sign-in, which that harness replaces with JWTs it mints itself. What remains
# unverified is therefore the account service and the platform's own PostgREST
# configuration — not the schema, the policies, the grants or the request
# shapes.
#
# It exercises the five things the assertions in `supabase/tests/` cannot,
# because they need PostgREST and GoTrue rather than SQL:
#
#   1. that `?on_conflict=user_id,item_id` resolves when the body does not
#      contain `user_id` — the single most load-bearing untested assumption in
#      the whole deployment, and a 400 (`42P10`) if it is wrong;
#   2. that the three read requests the client sends — the listing with an
#      inclusive bound, the listing with a compound keyset, and the fetch by
#      id — are accepted verbatim, and that a position the server returned
#      still names the same row once spelled the way the client spells it;
#   3. that a tombstone upsert reaches exactly the rows it names and wipes the
#      payload;
#   4. that the publishable key on its own reaches nothing;
#   5. that a row with no `signature` is refused by the column, not merely by
#      the client (manifest 05 §5.3).
#
# Needs: curl, jq, python3.
set -euo pipefail

for tool in curl jq python3; do
    command -v "$tool" >/dev/null || { echo "smoke: $tool is required" >&2; exit 2; }
done

URL="${SUPABASE_URL:-http://127.0.0.1:54321}"
# PostgREST serves the table at the root; Supabase puts it behind `/rest/v1`.
# Overridable so this script can run against a bare PostgREST — see
# `supabase/dev/postgrest-harness.sh`.
REST_BASE="${SMOKE_REST_BASE:-$URL/rest/v1}"
ANON="${SUPABASE_ANON_KEY:-}"
if [[ -z "$ANON" ]]; then
    if command -v supabase >/dev/null; then
        ANON="$(supabase status -o json 2>/dev/null | jq -r '.ANON_KEY // empty')"
    fi
fi
[[ -n "$ANON" ]] || { echo "smoke: set SUPABASE_ANON_KEY (or run inside a project with the CLI)" >&2; exit 2; }

REST="$REST_BASE/clipboard_items"
fail() { echo "smoke: FAIL — $*" >&2; exit 1; }
PASSWORD="${SMOKE_TEST_PASSWORD:-}"

sign_in() { # email -> access token
    [[ -n "$PASSWORD" ]] || fail "set SMOKE_TEST_PASSWORD or provide pre-minted tokens"
    curl -sS -X POST "$URL/auth/v1/token?grant_type=password" \
        -H "apikey: $ANON" -H 'Content-Type: application/json' \
        -d "{\"email\":\"$1\",\"password\":\"$PASSWORD\"}" \
    | jq -er '.access_token' || fail "could not sign in as $1 (is the seed applied?)"
}

# A pre-minted token skips the sign-in, for a harness that has PostgREST but no
# GoTrue. Everything after this point is identical either way.
alice="${SMOKE_ALICE_TOKEN:-$(sign_in dev-a@example.test)}"
bob="${SMOKE_BOB_TOKEN:-$(sign_in dev-b@example.test)}"
if [[ -n "${SMOKE_ALICE_TOKEN:-}" ]]; then
    echo "smoke: using pre-minted tokens (no account service in this run)"
else
    echo "smoke: signed in both development accounts"
fi

item="smoke-$(date +%s)"
now_ms="$(( $(date +%s) * 1000 ))"

# The payload is not real ciphertext — this script holds no sync passphrase and
# must not: a server-side key is the one thing this design does not have. A
# client will refuse to decrypt these rows and will count them as skipped, which
# is the correct behaviour (INV-N3).
# The signature is not a real one either, for the same reason: producing one
# needs the sync key, and this script must never hold it. A client will refuse
# these rows twice over — the signature does not verify and the payload does not
# decrypt — which is the correct behaviour (§5.3, INV-N3). The column is here
# because the deployment requires it, which is itself one of the assertions.
row() {
    jq -nc --arg id "$item" --arg ct "$1" --argjson at "$2" '[{
        item_id: $id, ciphertext: $ct, nonce: "bm9uY2U=", content_type: "text",
        created_at: $at, deleted: false, origin_device_id: "smoke-device",
        signature: "c21va2Utc2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDA="
    }]'
}

upsert() { # token, body
    curl -sS -o /dev/null -w '%{http_code}' -X POST "$REST?on_conflict=user_id,item_id" \
        -H "apikey: $ANON" -H "Authorization: Bearer $1" \
        -H 'Content-Type: application/json' \
        -H 'Prefer: resolution=merge-duplicates,return=minimal' \
        -d "$2"
}

# Measured, not assumed: with the unique index dropped, PostgREST 12.2.3
# answers this **400** (`42P10`, "no unique or exclusion constraint matching the
# ON CONFLICT specification") — on the *first* upsert, not on the replay. The
# code was expected to be a 409 on the replay; it is not. Either way the client
# classifies it `Permanent` and the round fails.
code="$(upsert "$alice" "$(row 'Zmlyc3Q=' "$now_ms")")"
[[ "$code" =~ ^(200|201|204)$ ]] || fail "first upsert returned $code (400 = missing unique index on (user_id, item_id))"

# The replay, which is what makes the write path idempotent.
code="$(upsert "$alice" "$(row 'c2Vjb25k' "$((now_ms + 1))")")"
[[ "$code" =~ ^(200|201|204)$ ]] || fail "the replayed upsert returned $code"
echo "smoke: upsert is idempotent without the client ever sending user_id"

# The read path, character for character as the client builds it
# (`SupabaseRest::list_since` and `fetch_rows`): a listing names versions and
# positions, never the payload, and whole rows are then fetched by id.
listing_columns='item_id,created_at,deleted,origin_device_id,signature,updated_at'
row_columns='item_id,ciphertext,nonce,content_type,payload_metadata,source_app_bundle_id,source_app_name,created_at,deleted,origin_device_id,signature,updated_at'
get() { # query parameters, already `name=value`
    local args=()
    for pair in "$@"; do args+=(--data-urlencode "$pair"); done
    curl -sS -G "$REST" \
        -H "apikey: $ANON" -H "Authorization: Bearer $alice" -H 'Accept: application/json' \
        "${args[@]}"
}

listing="$(get "select=$listing_columns" 'order=updated_at.asc,item_id.asc' 'limit=200' \
    'updated_at=gte.1970-01-01T00:00:00.000000Z')"
echo "$listing" | jq -e --arg id "$item" 'map(select(.item_id == $id)) | length == 1' >/dev/null \
    || fail "the row was not listed exactly once: $listing"
echo "$listing" | jq -e --arg columns "$listing_columns" \
    'map(keys | sort) | all(. == ($columns | split(",") | sort))' >/dev/null \
    || fail "the listing carries columns the client does not model: $listing"
echo "smoke: the listing has exactly the columns the client parses, and no payload"

# The position the server gave the row, spelled as the client spells it: UTC,
# six fractional digits, `Z` (`rest/item.rs`, `format_position`).
placed="$(echo "$listing" | jq -er --arg id "$item" 'map(select(.item_id == $id))[0].updated_at')"
position="$(python3 - "$placed" <<'PY'
import datetime, re, sys
m = re.fullmatch(
    r"(\d{4})-(\d\d)-(\d\d)[T ](\d\d):(\d\d):(\d\d)(?:\.(\d{1,6}))?(Z|[+-]\d\d(?::\d\d)?)",
    sys.argv[1])
if not m:
    sys.exit(f"not a timestamp the client reads: {sys.argv[1]}")
year, month, day, hour, minute, second = (int(part) for part in m.groups()[:6])
micros = int((m.group(7) or "").ljust(6, "0"))
zone = m.group(8)
offset = 0 if zone == "Z" else (1 if zone[0] == "+" else -1) * (
    int(zone[1:3]) * 3600 + int(zone[4:6] or 0) * 60)
utc = datetime.datetime(year, month, day, hour, minute, second) - datetime.timedelta(seconds=offset)
print(f"{utc:%Y-%m-%dT%H:%M:%S}.{micros:06d}Z")
PY
)"

same="$(get 'select=item_id' "item_id=eq.$item" "updated_at=eq.$position")"
echo "$same" | jq -e 'length == 1' >/dev/null \
    || fail "$placed read back as $position no longer names the row: $same"

# The compound keyset, bounded just before the row and then on the row itself.
after() { # tie-break id -> ids listed strictly after ($position, id)
    get "select=$listing_columns" 'order=updated_at.asc,item_id.asc' 'limit=200' \
        "or=(updated_at.gt.$position,and(updated_at.eq.$position,item_id.gt.$1))"
}
before_it="$(after 0)"
echo "$before_it" | jq -e --arg id "$item" 'map(select(.item_id == $id)) | length == 1' >/dev/null \
    || fail "the keyset was not accepted, or lost a row sharing its position: $before_it"
on_it="$(after "$item")"
echo "$on_it" | jq -e --arg id "$item" 'map(select(.item_id == $id)) | length == 0' >/dev/null \
    || fail "the keyset returned the row it is positioned on: $on_it"
echo "smoke: a server position survives the client's spelling, and the keyset pages on it"

page="$(get "select=$row_columns" "item_id=in.($item,$item-absent)" \
    'order=updated_at.asc,item_id.asc' 'limit=2')"
echo "$page" | jq -e --arg id "$item" 'map(.item_id) == [$id]' >/dev/null \
    || fail "the fetch by id did not return exactly the row that exists: $page"
echo "$page" | jq -e --arg columns "$row_columns" \
    'map(keys | sort) | all(. == ($columns | split(",") | sort))' >/dev/null \
    || fail "the row carries columns the client does not model: $page"
echo "smoke: rows are fetched by id with exactly the columns the client parses"

# An unsigned row must not exist at all. The client refuses to send one and
# refuses to merge one; this is the third gate, and the only one that applies to
# a writer that is not this client (manifest 05 §5.3).
unsigned="$(jq -nc --arg id "$item-unsigned" --argjson at "$now_ms" '[{
    item_id: $id, ciphertext: "dW5zaWduZWQ=", nonce: "bm9uY2U=", content_type: "text",
    created_at: $at, deleted: false, origin_device_id: "smoke-device"
}]')"
code="$(upsert "$alice" "$unsigned")"
[[ "$code" =~ ^(4|5) ]] || fail "a row with no signature was accepted ($code)"
echo "smoke: an unsigned row is refused by the column, not merely by the client"

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

# The tombstone, as `push` sends it: an ordinary upsert of a version with
# `deleted = true`, no payload, and `created_at` restamped so the deletion sorts
# above every device's watermark. There is no PATCH path — a partial write
# cannot carry a signature over the columns it does not send.
dead="$(jq -nc --arg id "$item" --argjson at "$((now_ms + 2))" '[{
    item_id: $id, ciphertext: "", nonce: "", content_type: "text",
    created_at: $at, deleted: true, origin_device_id: "smoke-device",
    signature: "c21va2Utc2lnbmF0dXJlLXBsYWNlaG9sZGVyLTAwMDA="
}]')"
code="$(upsert "$alice" "$dead")"
[[ "$code" =~ ^(200|201|204)$ ]] || fail "the tombstone upsert returned $code"

state="$(curl -sS -G "$REST" \
    -H "apikey: $ANON" -H "Authorization: Bearer $alice" -H 'Accept: application/json' \
    --data-urlencode 'select=item_id,ciphertext,deleted' \
    --data-urlencode "item_id=eq.$item")"
# `// ""` because a tombstone's payload is the empty string on the upsert path
# and null on a row written by anything else; both mean "no payload", and jq
# treats an empty string as truthy.
echo "$state" | jq -e '.[0].deleted == true and ((.[0].ciphertext // "") == "")' >/dev/null \
    || fail "the tombstone did not wipe the payload: $state"
echo "smoke: the tombstone is a row version, and it carries no ciphertext"

# Clean up after ourselves — these rows are undecryptable noise for any client
# signed into the same local account.
curl -sS -o /dev/null -X DELETE "$REST?item_id=like.$item*" \
    -H "apikey: $ANON" -H "Authorization: Bearer $alice"

echo "smoke: all checks passed"
echo
echo "Realtime is not covered here. To watch it by hand:"
echo "  websocat \"ws://127.0.0.1:54321/realtime/v1/websocket?vsn=1.0.0&apikey=\$SUPABASE_ANON_KEY\""
echo "  then send the phx_join frame from crates/copypaste-cloud/src/realtime/channel.rs"
echo "  with your access token and filter, and upsert a row from another shell."
