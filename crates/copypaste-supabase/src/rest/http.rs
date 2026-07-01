//! Shared HTTP request-building helpers for [`super::client::RestClient`]:
//! table URL construction, auth headers, and PostgREST error-body decoding.
//!
//! These are `pub(super)` (visible within `crate::rest` and its submodules)
//! rather than `pub` — they are internal plumbing shared by [`super::read`],
//! [`super::write`], and [`super::reencrypt`], not part of the crate's public
//! API.

use serde_json::Value;

use super::client::RestClient;

impl RestClient {
    pub(super) fn table_url(&self) -> String {
        format!("{}/rest/v1/clipboard_items", self.base_url)
    }

    pub(super) fn auth_headers(&self) -> [(&'static str, String); 2] {
        [
            ("apikey", self.anon_key.clone()),
            ("Authorization", format!("Bearer {}", self.access_token)),
        ]
    }

    /// Decode a PostgREST error body.  Returns a best-effort human-readable
    /// message suitable for [`super::error::RestError::PostgRest`]'s `message`
    /// field.
    pub(super) async fn decode_error(resp: reqwest::Response, fallback: &str) -> String {
        let raw = match resp.text().await {
            Ok(t) => t,
            Err(_) => return fallback.to_owned(),
        };
        // PostgREST errors look like: {"code":"...", "details":"...", "hint":"...", "message":"..."}
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
                return msg.to_owned();
            }
        }
        let snippet: String = raw.chars().take(200).collect();
        if snippet.is_empty() {
            fallback.to_owned()
        } else {
            snippet
        }
    }
}
