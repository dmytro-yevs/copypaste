//! GoTrue accounts and sessions.
//!
//! # What this module is
//!
//! Four calls against Supabase's auth service — sign up, sign in, refresh,
//! sign out — and one [`Session`] type that holds the tokens the rest of the
//! crate presents to PostgREST and Realtime.
//!
//! ```text
//! POST /auth/v1/signup                        -> Session (or EmailConfirmationRequired)
//! POST /auth/v1/token?grant_type=password     -> Session
//! POST /auth/v1/token?grant_type=refresh_token-> Session (rotated refresh token)
//! POST /auth/v1/logout                        -> ()
//! ```
//!
//! Every request carries `apikey: <anon key>`. The bearer differs by call: the
//! anon key for the three unauthenticated ones, the user's access token for
//! logout.
//!
//! # The one thing that is easy to get wrong: `invalid_grant`
//!
//! GoTrue answers **both** "that password is wrong" and "that refresh token is
//! dead" with `400`/`422` and the OAuth code `invalid_grant`. The body is not a
//! discriminator, and guessing from its text is how this goes wrong in both
//! directions:
//!
//! * treat a bad password as a dead session and the daemon throws away a
//!   perfectly good session and signs the user out because they typo'd;
//! * treat a dead session as a bad password and a long-lived daemon retries a
//!   refresh that can never succeed, forever.
//!
//! The fix — carried from port manifest 05 §4.6.1, which is *binding* — is that
//! **the grant kind we asked for is authoritative**. `GrantKind` is threaded
//! into the request helper and decides the variant; nothing in this module
//! inspects the error body to classify a failure. The body is only ever
//! *logged*, truncated, so a 502 HTML gateway page stays diagnosable
//! (manifest AT-38). That rule lives in [`error`], next to the enum it decides
//! between.
//!
//! # Secret hygiene
//!
//! * [`Session`]'s `Debug` prints `<redacted>` for both tokens (AT-42). It is a
//!   hand-written impl precisely so a later `#[derive(Debug)]` cannot leak them.
//! * Both tokens are zeroized on drop.
//! * `Session` deliberately does **not** implement `Serialize`. Persisting a
//!   session is the session store's job and should be a deliberate act, not
//!   something that falls out of `serde_json::to_string` in a log line.
//! * Emails are masked in logs by `redact_email`.
//! * No error variant carries free text, so no path and no token can reach a
//!   user-facing message (`CLAUDE.md` rule 4). `reqwest`'s own errors have their
//!   URL stripped before we keep them.
//!
//! # Retry
//!
//! One policy, from the `backoff` crate, shared with [`crate::rest`] via
//! [`transient_backoff`]. Network faults and 5xx retry; everything else is
//! permanent and surfaces to the caller immediately. A `429` is **not** retried
//! here — it is returned as [`AuthError::RateLimited`] with the server's
//! `Retry-After`, because the caller (the refresh loop) is the thing that knows
//! how long it may sleep. v1 had six separate hand-rolled backoffs; this crate
//! has none of its own.
//!
//! # How the module is laid out
//!
//! | file | owns |
//! |---|---|
//! | [`session`] | [`Session`]: the tokens, their expiry arithmetic, and their redaction |
//! | [`token`] | the GoTrue response body and how it becomes a `Session` |
//! | [`error`] | [`AuthError`], `GrantKind`, and the `invalid_grant` disambiguation |
//! | [`client`] | [`SupabaseAuth`]: the four endpoints and the request helper |
//! | [`http`] | the pieces `crate::rest` shares: the one retry policy, `Retry-After`, redaction |
//!
//! [`http`] is a module rather than a handful of loose functions because every
//! item in it has exactly one job: to be the *only* implementation of something
//! v1 had several of. Keeping them together is what makes a second one
//! obvious.

pub mod client;
pub mod error;
pub mod http;
pub mod session;
pub mod token;

/// A scripted HTTP/1.1 server, so the tests exercise real `reqwest` requests
/// without touching the network. `crate::rest`'s tests use it too — one stub
/// for the crate, not one per client.
#[cfg(test)]
pub(crate) mod stub;

#[cfg(test)]
mod testkit;

pub use client::{SupabaseAuth, AUTH_TIMEOUT};
pub use error::AuthError;
pub use http::transient_backoff;
pub use session::{Session, REFRESH_MARGIN_MS};

// `rest` reads the clock and the `Retry-After` header through the same two
// functions the auth client uses; `redact_email` stays module-private because
// only the auth calls take an email at all.
pub(crate) use http::{now_ms, retry_after_secs};
