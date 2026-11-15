//! The local IPC server.
//!
//! The files follow the path a request takes:
//!
//! * [`listener`] — the socket: binding it, refusing to steal a live one,
//!   locking it to `0600`, and framing lines off it. `pipe` is the Windows
//!   half of those first three; the framing is shared.
//! * [`dispatch`] — parse, the protocol and readiness gates, admission, and
//!   the split between the reactor and `spawn_blocking`. The handlers
//!   themselves are the shared `copypaste_runtime` dispatch.
//! * [`watch`] — the push half: a connection that has subscribed, and what it
//!   is exempt from.
//! * [`endpoint`] — a connection relaying frames to a module endpoint.
//! * [`messages`] — the transport's own failure strings; the handler
//!   sentences live with the shared dispatch.
//! * [`halted`] — the socket when there is no history to serve at all.
//!
//! **Errors never carry a filesystem path.** The socket path discloses the
//! local username (the privacy contract), so every failure is one of a fixed
//! set of sentences and the underlying error goes only to the local log.

#[cfg(test)]
mod config;
#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod dbadmin;
pub(crate) mod dispatch;
mod endpoint;
mod halted;
#[cfg(test)]
mod items;
mod listener;
pub(crate) mod messages;
#[cfg(all(test, unix))]
mod module_endpoint_tests;
mod peer_identity;
#[cfg(windows)]
mod pipe;
#[cfg(test)]
mod transfer;
mod watch;

pub use halted::serve as serve_halted;
#[cfg(unix)]
pub use listener::bind;
pub use listener::run;
#[cfg(windows)]
pub use pipe::bind;
