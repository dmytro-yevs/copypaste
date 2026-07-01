//! `build_join_payload`: the Phoenix Channel `phx_join` payload, including the
//! mandatory RLS `user_id` row filter (security-critical, CopyPaste-nr2y).

/// Build the Phoenix Channel join payload for a Supabase Realtime subscription.
///
/// # Bearer token
/// The `user_jwt` is placed under `config.access_token` so Supabase Realtime
/// authenticates the channel with the caller's RLS identity.  An empty string
/// disables per-user RLS (anonymous / anon-key-only access).
///
/// # Row filter (CopyPaste-nr2y — mandatory, defense-in-depth)
/// The `user_id` filter `"user_id=eq.{user_id}"` is **always** included in the
/// `postgres_changes` subscription.  Omitting it would mean the Realtime server
/// could deliver cross-user rows into the event stream before server-side RLS
/// applies them, leaking data on permissive or misconfigured deployments.
///
/// A missing `user_id` is therefore a **hard error** at the call site — callers
/// must obtain the GoTrue user UUID before establishing the Realtime connection.
/// See `run_session` which returns `SessionResult::ConnectError` when
/// `config.user_id` is `None`.
///
/// # Event filter
/// Registers `event: "*"` so INSERT, UPDATE **and** DELETE changes are all
/// delivered to this device.  Using `event: "INSERT"` only would mean that
/// cross-device UPDATE/DELETE operations are silently dropped.
///
/// The payload shape matches Supabase Realtime v2 (`vsn=1.0.0`):
/// ```json
/// {
///   "config": {
///     "access_token": "<jwt>",
///     "postgres_changes": [
///       { "event": "*", "schema": "public", "table": "clipboard_items",
///         "filter": "user_id=eq.<uuid>" }
///     ]
///   }
/// }
/// ```
pub(crate) fn build_join_payload(user_jwt: &str, user_id: &str) -> serde_json::Value {
    serde_json::json!({
        "config": {
            "access_token": user_jwt,
            "postgres_changes": [{
                "event": "*",
                "schema": "public",
                "table": "clipboard_items",
                "filter": format!("user_id=eq.{user_id}")
            }]
        }
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_join_payload_includes_bearer_token() {
        let jwt = "my.jwt.token";
        let uid = "550e8400-e29b-41d4-a716-446655440000";
        // CopyPaste-nr2y: user_id is now mandatory — pass a real UUID.
        let payload = build_join_payload(jwt, uid);
        // The JWT must appear under config.access_token (Supabase Realtime v2 shape).
        let token_in_payload = payload
            .pointer("/config/access_token")
            .and_then(|v| v.as_str())
            == Some(jwt);
        assert!(
            token_in_payload,
            "join payload must include JWT under /config/access_token, got: {}",
            serde_json::to_string(&payload).unwrap()
        );
    }

    #[test]
    fn build_join_payload_registers_all_events() {
        let uid = "550e8400-e29b-41d4-a716-446655440000";
        // CopyPaste-nr2y: user_id is now mandatory — pass a real UUID.
        let payload = build_join_payload("tok", uid);
        let payload_str = serde_json::to_string(&payload).unwrap();
        // event:"*" means INSERT + UPDATE + DELETE are all delivered.
        assert!(
            payload_str.contains("\"*\""),
            "join payload must register event:\"*\", got: {payload_str}"
        );
        assert!(
            !payload_str.contains("\"INSERT\""),
            "join payload must NOT limit to INSERT-only, got: {payload_str}"
        );
    }

    /// CopyPaste-nr2y: the user_id filter is always mandatory.
    /// build_join_payload always includes "user_id=eq.<uuid>" — a missing user_id
    /// is rejected at the run_session level (hard error, not silently omitted).
    #[test]
    fn build_join_payload_always_includes_mandatory_user_id_filter() {
        let uid = "550e8400-e29b-41d4-a716-446655440000";
        let payload = build_join_payload("tok", uid);
        let payload_str = serde_json::to_string(&payload).unwrap();
        // Filter clause must always be present (defense-in-depth).
        assert!(
            payload_str.contains("user_id=eq."),
            "join payload must always contain user_id filter; got: {payload_str}"
        );
        assert!(
            payload_str.contains(uid),
            "join payload must embed the user UUID in the filter; got: {payload_str}"
        );
        // Verify the filter is under the postgres_changes entry.
        let filter = payload
            .pointer("/config/postgres_changes/0/filter")
            .and_then(|v| v.as_str());
        assert_eq!(
            filter,
            Some("user_id=eq.550e8400-e29b-41d4-a716-446655440000"),
            "filter must be at /config/postgres_changes/0/filter; got: {payload_str}"
        );
    }
}
