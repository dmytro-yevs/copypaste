//! Opening a socket and joining the channel: where we connect, as whom, and
//! what we ask to be sent.
//!
//! The three things that decide whether the subscription is *safe* rather than
//! merely working all live here — the token's subject becomes the per-user
//! filter, the anon key goes in a header rather than the URL, and the join is
//! not considered done until the server has confirmed **the subscription we
//! asked for**.
//!
//! That last one is stronger than it sounds. Supabase answers `phx_join` with
//! `{"status":"ok","response":{"postgres_changes":[…]}}`, and the array it
//! returns is what the server actually registered — which is not necessarily
//! what was requested. A server that dropped or narrowed the
//! `user_id=eq.<uuid>` filter, or registered `INSERT` where `*` was asked for,
//! still answers `ok`. Taking that as a healthy join means believing a channel
//! is carrying deletes when it is carrying none, and slowing the poll loop to
//! its long ceiling on the strength of it (manifest 05 §4.8).

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use url::Url;

use super::event::RealtimeError;
use super::frame::parse_frame;
use super::{JOIN_TIMEOUT, TABLE, TOPIC};

pub(super) type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Open the socket and join the channel, returning only once the server has
/// confirmed the join with a `phx_reply` whose status is `ok`.
///
/// Gating on the *reply* rather than on socket-open is deliberate: the caller
/// uses "realtime is live" to slow its poll loop, and a socket whose channel
/// never joined delivers nothing (manifest 05 §4.8).
pub(super) async fn open_channel(
    url: &str,
    anon_key: &str,
    access_token: &str,
    user_id: &str,
) -> Result<WsStream, RealtimeError> {
    let mut request = url
        .into_client_request()
        .map_err(|_| RealtimeError::Connect("the configured url is not a websocket url"))?;
    // The publishable key goes in a header, not the query string: a URL ends up
    // in proxy logs and in error messages (`CopyPaste-lnjm`).
    request.headers_mut().insert(
        "apikey",
        anon_key
            .parse()
            .map_err(|_| RealtimeError::Connect("the anon key is not a valid header value"))?,
    );

    let connect = async {
        let (mut ws, _) = connect_async(request)
            .await
            .map_err(|_| RealtimeError::Connect("the websocket could not be opened"))?;

        ws.send(Message::Text(join_frame(access_token, user_id).to_string()))
            .await
            .map_err(|_| RealtimeError::Connect("the join could not be sent"))?;

        // Wait for the join confirmation, ignoring anything else that arrives
        // first.
        loop {
            match ws.next().await {
                Some(Ok(Message::Text(text))) => match join_reply(&text, user_id) {
                    Some(Ok(())) => return Ok(ws),
                    Some(Err(e)) => return Err(e),
                    None => continue,
                },
                Some(Ok(Message::Close(_))) | None => {
                    return Err(RealtimeError::JoinRefused);
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) => {
                    return Err(RealtimeError::Connect("the websocket failed during join"))
                }
            }
        }
    };

    tokio::time::timeout(JOIN_TIMEOUT, connect)
        .await
        .map_err(|_| RealtimeError::Connect("timed out opening the channel"))?
}

/// The `phx_join` frame.
///
/// Three things here are load-bearing (manifest 05 §4.7):
///
/// 1. `access_token` is the *current* user JWT, not the anon key.
/// 2. `event` is `"*"`, never `"INSERT"`. INSERT-only silently drops every
///    cross-device update and delete — which, since a delete is a tombstone
///    update, means every delete.
/// 3. `filter` is always `user_id=eq.<uuid>`. See
///    [`RealtimeError::MissingUserId`].
fn join_frame(access_token: &str, user_id: &str) -> Value {
    json!([
        "1",
        "1",
        TOPIC,
        "phx_join",
        {
            "config": {
                "access_token": access_token,
                "postgres_changes": [subscription(user_id)],
            }
        }
    ])
}

/// The one subscription this client asks for, and the one it checks it got.
///
/// Written once, so the request and the assertion cannot drift — a check
/// against a separately-spelled expectation is a check that keeps passing after
/// the request changes.
fn subscription(user_id: &str) -> Value {
    json!({
        "event": "*",
        "schema": "public",
        "table": TABLE,
        "filter": format!("user_id=eq.{user_id}"),
    })
}

/// `Some(Ok(()))` for a confirmed join of the subscription we asked for,
/// `Some(Err(_))` for a refused or altered one, `None` if this frame is not a
/// reply to our join at all.
fn join_reply(text: &str, user_id: &str) -> Option<Result<(), RealtimeError>> {
    let frame = parse_frame(text).ok()?;
    if frame.event != "phx_reply" || frame.topic != TOPIC {
        return None;
    }
    if frame.payload.get("status").and_then(Value::as_str) != Some("ok") {
        return Some(Err(RealtimeError::JoinRefused));
    }
    Some(confirms_subscription(&frame.payload, user_id))
}

/// Does the server's echo describe the subscription that was requested?
///
/// Compared field by field rather than as whole values: the echo adds a server
/// -assigned `id` that the request does not have, and an equality test would
/// therefore fail on every healthy join.
///
/// An **absent** echo is a mismatch, not a pass. A server that says nothing
/// about what it registered is indistinguishable, from here, from one that
/// registered something narrower — and the cost of being wrong is asymmetric:
/// refusing a good join loses latency, while accepting a bad one loses events
/// with no symptom at all. The poll loop is the correctness mechanism either
/// way (manifest 05 §5.1 row 9a).
fn confirms_subscription(payload: &Value, user_id: &str) -> Result<(), RealtimeError> {
    let confirmed = payload
        .get("response")
        .and_then(|r| r.get("postgres_changes"))
        .and_then(Value::as_array)
        .ok_or(RealtimeError::JoinMismatch)?;

    let [confirmed] = confirmed.as_slice() else {
        return Err(RealtimeError::JoinMismatch);
    };

    let requested = subscription(user_id);
    let same = ["event", "schema", "table", "filter"]
        .into_iter()
        .all(|field| confirmed.get(field) == requested.get(field));

    if same {
        Ok(())
    } else {
        // Not logged with the two configurations in it: the filter carries the
        // account's user id, and this module logs no payloads.
        tracing::warn!("the realtime server confirmed a different subscription than requested");
        Err(RealtimeError::JoinMismatch)
    }
}

/// `https://…` / `http://…` -> the realtime websocket endpoint.
///
/// A non-`http` scheme is passed through untouched so a `ws://` loopback URL
/// works in a test harness.
pub(super) fn websocket_url(base: &str) -> String {
    let Ok(mut base_url) = Url::parse(base) else {
        return base.to_owned();
    };
    match base_url.scheme() {
        "https" => base_url
            .set_scheme("wss")
            .expect("https and wss are both hierarchical schemes"),
        "http" => base_url
            .set_scheme("ws")
            .expect("http and ws are both hierarchical schemes"),
        _ => {}
    }
    let Ok(mut endpoint) = base_url.join("/realtime/v1/websocket") else {
        return base.to_owned();
    };
    // `vsn` selects the Phoenix serializer; 1.0.0 is the five-element array
    // this module parses.
    endpoint.query_pairs_mut().append_pair("vsn", "1.0.0");
    endpoint.into()
}

/// The `sub` claim of a JWT, without verifying it.
///
/// We are not authenticating anything here — the server does that, and it holds
/// the signing key. All we need is the user id the token already asserts, so
/// that the channel filter can be built. Reading one claim is a base64url
/// decode and a JSON lookup; pulling a full JWT crate in would add a second
/// signature-verification stack to the tree for a value we deliberately do not
/// verify (`CLAUDE.md` rule 1, exemption 3).
///
/// Returns `None` for anything that is not a three-part token with a decodable
/// JSON payload carrying a non-empty string `sub` — which the caller turns into
/// a hard error rather than an omitted filter.
pub(super) fn jwt_subject(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    // A JWT has three parts. Two is not a token we should be reading, and the
    // signature is not checked here — see the doc comment.
    parts.next()?;
    let decoded = B64URL.decode(payload).ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    let sub = claims.get("sub")?.as_str()?;
    if sub.is_empty() {
        return None;
    }
    Some(sub.to_owned())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// The join frame, the reply rule, the URL and the subject claim are all pure
// functions of a string. Nothing here opens a socket.

#[cfg(test)]
mod tests {
    use super::*;

    /// A JWT-shaped token whose payload decodes to `{"sub": id}`. Not signed —
    /// nothing here verifies signatures, and pretending otherwise in a test
    /// would suggest that it does.
    fn token_for(id: &str) -> String {
        let payload = B64URL.encode(format!(r#"{{"sub":"{id}","role":"authenticated"}}"#));
        format!("header.{payload}.signature")
    }

    const USER: &str = "6b1e2f80-0000-4000-8000-000000000001";

    #[test]
    fn the_join_frame_carries_the_jwt_the_wildcard_and_the_filter() {
        let frame = join_frame("the.jwt.here", USER);
        let config = &frame[4]["config"];

        assert_eq!(config["access_token"], "the.jwt.here");

        let change = &config["postgres_changes"][0];
        assert_eq!(
            change["event"], "*",
            "INSERT-only drops updates and deletes"
        );
        assert_ne!(change["event"], "INSERT");
        assert_eq!(change["schema"], "public");
        assert_eq!(change["table"], TABLE);
        assert_eq!(change["filter"], format!("user_id=eq.{USER}"));

        // Envelope: five elements, our topic, and ref 1 (the heartbeat counter
        // starts at 2 because of this).
        assert_eq!(frame.as_array().unwrap().len(), 5);
        assert_eq!(frame[1], "1");
        assert_eq!(frame[2], TOPIC);
        assert_eq!(frame[3], "phx_join");
    }

    /// A `phx_reply` carrying the server's echo of what it registered.
    fn reply(status: &str, confirmed: &str) -> String {
        format!(
            r#"["1","1","realtime:clipboard_items","phx_reply",
                {{"status":"{status}","response":{{"postgres_changes":{confirmed}}}}}]"#
        )
    }

    /// What a healthy Supabase join answers: our four fields plus a
    /// server-assigned id.
    fn echo(user: &str) -> String {
        format!(
            r#"[{{"id":12345,"event":"*","schema":"public",
                  "table":"clipboard_items","filter":"user_id=eq.{user}"}}]"#
        )
    }

    #[test]
    fn only_an_ok_reply_on_our_topic_confirms_the_join() {
        assert_eq!(join_reply(&reply("ok", &echo(USER)), USER), Some(Ok(())));
        assert_eq!(
            join_reply(&reply("error", &echo(USER)), USER),
            Some(Err(RealtimeError::JoinRefused))
        );
        // A reply for another topic, or another event, is not our confirmation.
        assert_eq!(
            join_reply(r#"[null,"2","phoenix","phx_reply",{"status":"ok"}]"#, USER),
            None
        );
        assert_eq!(
            join_reply(
                r#"["1","1","realtime:clipboard_items","phx_error",{}]"#,
                USER
            ),
            None
        );
        assert_eq!(join_reply("garbage", USER), None);
    }

    #[test]
    fn an_ok_reply_that_confirms_a_different_subscription_is_not_a_join() {
        // Each of these is a channel that would look healthy and deliver less
        // than the caller believes. The filter cases are the sharp ones: a
        // subscription without `user_id=eq.<uuid>` sees another account's rows
        // before RLS applies, and one narrowed to somebody else's id sees
        // nothing at all.
        let other = "6b1e2f80-0000-4000-8000-000000000002";
        let cases = [
            // INSERT-only: no updates, and therefore no deletes.
            r#"[{"id":1,"event":"INSERT","schema":"public","table":"clipboard_items","filter":"user_id=eq.6b1e2f80-0000-4000-8000-000000000001"}]"#.to_string(),
            // The filter dropped entirely.
            r#"[{"id":1,"event":"*","schema":"public","table":"clipboard_items"}]"#.to_string(),
            // The filter silently narrowed to another account.
            format!(r#"[{{"id":1,"event":"*","schema":"public","table":"clipboard_items","filter":"user_id=eq.{other}"}}]"#),
            // Another table.
            r#"[{"id":1,"event":"*","schema":"public","table":"other","filter":"user_id=eq.6b1e2f80-0000-4000-8000-000000000001"}]"#.to_string(),
            // Nothing registered, or more than we asked for.
            "[]".to_string(),
            format!("[{}, {}]", &echo(USER)[1..echo(USER).len() - 1], &echo(USER)[1..echo(USER).len() - 1]),
        ];

        for confirmed in cases {
            assert_eq!(
                join_reply(&reply("ok", &confirmed), USER),
                Some(Err(RealtimeError::JoinMismatch)),
                "accepted a join that confirmed {confirmed}"
            );
        }
    }

    #[test]
    fn an_ok_reply_that_says_nothing_about_what_it_registered_is_refused() {
        // Believing an unqualified `ok` is exactly the hole this closes: we
        // cannot tell "registered what you asked for" from "registered
        // something narrower" without being told.
        for payload in [
            r#"{"status":"ok"}"#,
            r#"{"status":"ok","response":{}}"#,
            r#"{"status":"ok","response":{"postgres_changes":null}}"#,
        ] {
            let frame = format!(r#"["1","1","realtime:clipboard_items","phx_reply",{payload}]"#);
            assert_eq!(
                join_reply(&frame, USER),
                Some(Err(RealtimeError::JoinMismatch)),
                "accepted {payload}"
            );
        }
    }

    #[test]
    fn the_request_and_the_check_read_the_same_configuration() {
        // One spelling of the subscription. If these ever came apart, the check
        // would keep passing while the request changed underneath it.
        let requested = &join_frame("jwt", USER)[4]["config"]["postgres_changes"][0];
        assert_eq!(requested, &subscription(USER));
    }

    #[test]
    fn a_token_without_a_subject_is_a_hard_error() {
        // CopyPaste-nr2y: the filter is never silently omitted.
        assert_eq!(jwt_subject(&token_for(USER)).as_deref(), Some(USER));

        // Present but empty is still no user id.
        let empty_sub = B64URL.encode(r#"{"sub":""}"#);
        assert_eq!(jwt_subject(&format!("h.{empty_sub}.s")), None);

        assert_eq!(jwt_subject("not.a"), None);
        assert_eq!(jwt_subject(""), None);
        assert_eq!(jwt_subject("a.b.c"), None);

        let no_sub = B64URL.encode(r#"{"role":"authenticated"}"#);
        assert_eq!(jwt_subject(&format!("h.{no_sub}.s")), None);
    }

    #[test]
    fn the_websocket_url_is_derived_from_the_rest_url() {
        assert_eq!(
            websocket_url("https://proj.supabase.co"),
            "wss://proj.supabase.co/realtime/v1/websocket?vsn=1.0.0"
        );
        // A trailing slash must not double up.
        assert_eq!(
            websocket_url("https://proj.supabase.co/"),
            "wss://proj.supabase.co/realtime/v1/websocket?vsn=1.0.0"
        );
        assert_eq!(
            websocket_url("http://127.0.0.1:54321"),
            "ws://127.0.0.1:54321/realtime/v1/websocket?vsn=1.0.0"
        );
        assert_eq!(
            websocket_url("ws://127.0.0.1:54321/harness/"),
            "ws://127.0.0.1:54321/realtime/v1/websocket?vsn=1.0.0"
        );
        assert_eq!(
            websocket_url("https://[2001:db8::1]:8443/nested%20base/?apikey=must-go#fragment"),
            "wss://[2001:db8::1]:8443/realtime/v1/websocket?vsn=1.0.0"
        );
        // The anon key is never in the URL.
        assert!(!websocket_url("https://proj.supabase.co").contains("apikey"));
    }
}
