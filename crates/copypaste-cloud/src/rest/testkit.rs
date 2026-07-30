//! Fixtures shared by this module's tests.
//!
//! One client builder and one row builder, so that the request-shape tests and
//! the status-handling tests are asserting against the same client rather than
//! two subtly different ones. The scripted server itself is
//! [`crate::auth::stub`] — one stub for the crate, not one per HTTP client.

use std::time::Duration;

use backon::ExponentialBuilder;

use super::{CloudItem, SupabaseRest};
use crate::auth::stub::Stub;
use crate::crypto::{derive_sync_key, SyncKey, KEY_LEN};
use crate::CloudConfig;

pub(super) const ANON: &str = "anon-key-abc";
pub(super) const TOKEN: &str = "user-access-token";
const PASS: &str = "correct horse battery staple";
const ACCOUNT: &str = "acct-rest";

/// The fixture key, derived once per binary. Argon2id is deliberately
/// expensive; a per-row derivation would make the suite slow enough that people
/// stop running it.
pub(super) fn key() -> SyncKey {
    static KEY: std::sync::OnceLock<[u8; KEY_LEN]> = std::sync::OnceLock::new();
    SyncKey::from_bytes(*KEY.get_or_init(|| *derive_sync_key(PASS, ACCOUNT).unwrap().to_bytes()))
}

/// A retry policy that finishes in milliseconds, so a test that exercises the
/// transient path does not sleep for seconds.
pub(super) fn fast_retry() -> ExponentialBuilder {
    ExponentialBuilder::new()
        .with_min_delay(Duration::from_millis(1))
        .with_max_delay(Duration::from_millis(2))
        .with_max_times(3)
}

pub(super) fn client(stub: &Stub) -> SupabaseRest {
    SupabaseRest::new(CloudConfig {
        url: stub.base_url.clone(),
        anon_key: ANON.to_string(),
    })
    .with_retry_policy(fast_retry())
}

/// A signed row, because an unsigned one is not a row this client will send.
pub(super) fn item(id: &str) -> CloudItem {
    let mut row = CloudItem::sealed(
        id,
        b"sealed-bytes",
        b"nonce12",
        "text",
        1_700_000_000_000,
        "device-a",
    );
    row.sign(&key());
    row
}

/// Split a captured `?a=b&c=d` target into pairs, percent-decoded enough for
/// assertions to read naturally.
pub(super) fn query_pairs(target: &str) -> Vec<(String, String)> {
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"),
                16,
            ) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub(super) fn value_of(pairs: &[(String, String)], key: &str) -> String {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| panic!("no `{key}` in query: {pairs:?}"))
}
