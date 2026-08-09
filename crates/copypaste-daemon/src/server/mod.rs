//! The local IPC server.
//!
//! The files follow the path a request takes:
//!
//! * [`listener`] — the socket: binding it, refusing to steal a live one,
//!   locking it to `0600`, and framing lines off it. `pipe` is the Windows
//!   half of those first three; the framing is shared.
//! * [`dispatch`] — parse, the protocol and readiness gates, and the `match`
//!   that routes a `Method` to a handler. This is also where the split between
//!   the reactor (peer operations, which are network I/O) and `spawn_blocking`
//!   (everything else, which is SQLite and AEAD) is decided.
//! * [`items`] — the history handlers, and the decrypt-to-wire step they share.
//!   The peer handlers are `crate::p2p::handlers`; the two files divide on the
//!   same line the two thread pools do.
//! * [`transfer`] — export and import, and the rules about what may leave.
//! * [`dbadmin`] — backup, and the validate-then-swap restore.
//! * [`config`] — reading and changing the daemon's settings.
//! * [`watch`] — the push half: a connection that has subscribed, and what it
//!   is exempt from.
//! * [`messages`] — every client-visible failure string, in one place, and the
//!   classification of the failures that carry a code of their own.
//! * [`halted`] — the socket when there is no history to serve at all.
//!
//! **Errors never carry a filesystem path.** The socket path discloses the
//! local username (AGENTS.md rule 4), and a `StoreError` from SQLite routinely
//! embeds the database path. Every failure is mapped to one of the fixed
//! sentences in [`messages`]; the underlying error goes to the local log and
//! never onto the wire. Gathering them in one file is what lets a single test
//! pin the whole set.

mod config;
#[cfg(test)]
mod contract_tests;
mod dbadmin;
pub(crate) mod dispatch;
mod halted;
mod items;
mod listener;
pub(crate) mod messages;
#[cfg(windows)]
mod pipe;
mod transfer;
mod watch;

pub use halted::serve as serve_halted;
#[cfg(unix)]
pub use listener::bind;
pub use listener::run;
#[cfg(windows)]
pub use pipe::bind;
