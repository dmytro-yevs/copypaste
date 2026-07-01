//! Supabase Realtime WebSocket client with Phoenix Channel protocol support.
//!
//! Handles:
//! - Connection to `wss://{project}.supabase.co/realtime/v1/websocket`
//! - Phoenix Channel join for `realtime:clipboard_items`
//! - Heartbeat every 30 seconds
//! - Exponential backoff reconnection
//! - Graceful shutdown via [`ClientHandle`]
//!
//! # Module layout
//!
//! | Sub-module | Responsibility |
//! |---|---|
//! | [`realtime_tls`] | TLS cert-pinning ([`SpkiPins`], `PinningVerifier`, `DerReader`, `build_rustls_connector`) |
//! | [`realtime_config`] | URL helpers, config struct, PII redaction ([`RealtimeConfig`], [`scrub_ws_url`], …) |
//! | `client` | [`RealtimeClient`], [`ClientHandle`], `RealtimeError`, connection lifecycle |
//! | `reconnect` | `RunningGuard`, `connection_loop` (exponential-backoff orchestration), `SessionResult` |
//! | `session` | `run_session` — a single WS session (connect → join → heartbeat + recv) |
//! | `dispatch` | `handle_message` (frame decode) + `dispatch_event` (Phoenix event routing) |
//! | `join` | `build_join_payload` (mandatory RLS `user_id` row filter) |
//!
//! CopyPaste-vp63.26: `realtime_client.rs` (formerly a single 1102-line file)
//! was split into the `client`/`reconnect`/`session`/`dispatch`/`join`
//! submodules above; this `mod.rs` re-exports the same public surface
//! (`ClientHandle`, `RealtimeClient`, `RealtimeError`) so `mod.rs` consumers
//! (e.g. `copypaste-daemon`'s `cloud/ws.rs`) are unaffected.

#![allow(clippy::result_large_err)] // RealtimeError carries WebSocket variants; boxing not worth the noise here

mod client;
mod dispatch;
mod join;
mod reconnect;
mod session;

pub mod realtime_config;
pub mod realtime_tls;

// ── Public re-exports (preserves the API surface of the old flat realtime.rs) ─

pub use client::{ClientHandle, RealtimeClient, RealtimeError};
pub use realtime_config::{scrub_ws_url, RealtimeConfig};
pub use realtime_tls::SpkiPins;

// ── Crate-internal re-exports used across the sub-modules ─────────────────────
//
// Only symbols that are consumed by a *different* sub-module (not the one that
// defines them) need to be re-exported here. Symbols used only within their
// own file are kept module-private there.

pub(crate) use realtime_config::{build_ws_request, redact_payload};
pub(crate) use realtime_tls::build_rustls_connector;
