# Supabase Setup — CopyPaste Cloud Sync

End-to-end guide to wire the CopyPaste daemon to a Supabase project for
multi-device clipboard sync. Free tier is plenty for personal use.

Cloud sync is **opt-in** and disabled by default. The daemon works fully
offline (local + LAN peers) until you complete every step below.

---

## 1. Create a Supabase project

1. Open <https://supabase.com> and sign in (free).
2. Click **New project**. Choose any region close to your devices.
3. Save the database password somewhere safe — you will not need it for
   CopyPaste, but losing it locks you out of the Supabase dashboard.
4. Wait ~2 minutes for provisioning.

## 2. Capture your project URL and anon key

In the dashboard sidebar: **Project Settings → API**.

| Setting          | Where you'll use it                                            |
| ---------------- | --------------------------------------------------------------- |
| Project URL      | `SUPABASE_URL` env var. **Must start with `https://`** — the daemon refuses plain `http://` for cloud sync. |
| `anon` `public` key | `SUPABASE_ANON_KEY` env var. Long string starting with `eyJ…`. |

Do **not** export `service_role` keys. The daemon only ever needs `anon`.

## 3. Run the schema

1. Sidebar: **SQL Editor → New query**.
2. Paste the entire contents of [`schema.sql`](./schema.sql).
3. Click **Run**. You should see "Success. No rows returned."

This creates the `public.clipboard_items` table, indexes, and the
`updated_at` trigger.

## 4. Enable Row-Level Security

1. SQL Editor → New query.
2. Paste the entire contents of [`rls-policies.sql`](./rls-policies.sql).
3. Click **Run**.

This activates RLS so every user only ever sees rows they own
(see the design note at the top of `rls-policies.sql`).

## 5. Add the table to the Realtime publication

The daemon subscribes to `realtime:clipboard_items` via WebSocket. The
table must be part of the `supabase_realtime` publication before any
row events flow.

1. Sidebar: **Database → Replication**.
2. Find the publication named `supabase_realtime`. Click it.
3. Toggle `clipboard_items` (under `public`) **on**.
4. Save.

Verify with:

```sql
select * from pg_publication_tables
where  pubname = 'supabase_realtime'
  and  schemaname = 'public';
```

You should see one row, `tablename = clipboard_items`.

## 6. Create your user account

The daemon authenticates via email/password (GoTrue password grant).

* **Easy path**: Sidebar **Authentication → Users → Add user → Create
  new user**. Enter an email + password. Toggle **Auto Confirm User**
  on so you don't have to verify by email.
* **Production path**: build a signup flow in your app of choice — the
  daemon doesn't ship a signup UI yet.

## 7. Export env vars before starting the daemon

The daemon reads these from its process environment. Required:

```sh
export SUPABASE_URL="https://YOUR-PROJECT.supabase.co"
export SUPABASE_ANON_KEY="eyJhbGciOi..."
export SUPABASE_EMAIL="you@example.com"
export SUPABASE_PASSWORD="..."
```

Optional:

| Variable                         | Purpose                                                 |
| -------------------------------- | -------------------------------------------------------- |
| `SUPABASE_REALTIME_TOPIC`        | Override the Phoenix topic (default: `realtime:clipboard_items`). |
| `SUPABASE_REALTIME_DISABLED=1`   | Hard-disable Realtime even if credentials are present.   |

For the macOS LaunchAgent, the cleanest place to put these is the
`EnvironmentVariables` dict in
`~/Library/LaunchAgents/com.copypaste.daemon.plist`. Editing the plist
keeps the secrets out of your shell rc files. Example fragment:

```xml
<key>EnvironmentVariables</key>
<dict>
    <key>SUPABASE_URL</key>          <string>https://YOUR-PROJECT.supabase.co</string>
    <key>SUPABASE_ANON_KEY</key>     <string>eyJ...</string>
    <key>SUPABASE_EMAIL</key>        <string>you@example.com</string>
    <key>SUPABASE_PASSWORD</key>     <string>...</string>
</dict>
```

## 8. Restart the daemon

```sh
launchctl unload ~/Library/LaunchAgents/com.copypaste.daemon.plist
launchctl load   ~/Library/LaunchAgents/com.copypaste.daemon.plist
```

The daemon will:

1. Read the four env vars.
2. POST `/auth/v1/token?grant_type=password` to sign in.
3. Open a WebSocket to `wss://YOUR-PROJECT.supabase.co/realtime/v1/websocket`.
4. Send `phx_join` for `realtime:clipboard_items`.
5. Start heartbeating every 30 s.

## 9. Verify end-to-end

On the device where the daemon is running:

```sh
pbcopy "supabase smoke test $(date +%s)"
```

Then in the Supabase dashboard:

* **Table Editor → clipboard_items** — a new row should appear within a
  second. `payload_ct` will be a hex blob (correct — server never sees
  plaintext), and `device_id` will match the daemon's UUID.
* **Logs → Realtime** — you should see a `phx_join` accepted for
  `realtime:clipboard_items` and a `postgres_changes` event fire.

---

## Troubleshooting

### "cloud sign-in failed" in the daemon log

* Re-check `SUPABASE_EMAIL` / `SUPABASE_PASSWORD`. The daemon surfaces the
  exact GoTrue error message — common ones:
  * `Invalid login credentials` → wrong password, or user wasn't auto-confirmed.
  * `Email not confirmed` → toggle **Auto Confirm User** and recreate.
* Re-check that `SUPABASE_URL` starts with **`https://`**. The daemon
  enforces HTTPS-only and will not fall back to `http://`.

### Realtime connects but no events arrive

* You forgot **step 5**. RLS is fine, but without the table being part of
  `supabase_realtime`, Postgres never emits row changes to the Realtime
  service. Re-do step 5 and watch **Logs → Realtime**.

### `insert violates row-level security policy`

* The signed-in user differs from the `user_id` you tried to write. The
  daemon relies on `user_id default auth.uid()`, so don't supply
  `user_id` from the client — let the default fire.

### Items appear on device A but never on device B

* Confirm both devices use the **same** `SUPABASE_EMAIL` (same trust
  circle — see RLS design note).
* Both daemons must be running with `SUPABASE_REALTIME_DISABLED` unset.
* Tail device B's daemon log for `Phoenix Channel join confirmed`. If
  you see disconnect-loop messages instead, check that the project URL
  is reachable from that network (corporate proxies often block
  `wss://` upgrades).

### Cloud sync is disabled by default — how do I confirm it's on?

* The daemon emits one of these lines at startup:
  * `Supabase Realtime is disabled (feature flag)` — env var missing or
    `SUPABASE_REALTIME_DISABLED=1`. Cloud is off.
  * `Connecting to Supabase Realtime` — env vars seen, cloud is on.

### How do I rotate the password?

1. Supabase dashboard → **Authentication → Users → ⋯ → Send password reset**.
2. Update `SUPABASE_PASSWORD` in the launchd plist.
3. Restart the daemon (step 8).

### Free-tier limits

Supabase free tier currently allows 200 concurrent Realtime connections
and 2 million messages/month — plenty of headroom for a personal
clipboard. Hard limits are at <https://supabase.com/pricing>.
