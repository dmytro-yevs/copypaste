//! The Unix-socket IPC server.
//!
//! Four files, following the path a request takes:
//!
//! * [`listener`] — the socket: binding it, refusing to steal a live one,
//!   locking it to `0600`, and framing lines off it. The only file here that
//!   touches the filesystem or a socket.
//! * [`dispatch`] — parse, the protocol and readiness gates, and the `match`
//!   that routes a `Method` to a handler. This is also where the split between
//!   the reactor (peer operations, which are network I/O) and `spawn_blocking`
//!   (everything else, which is SQLite and AEAD) is decided.
//! * [`items`] — the history handlers, and the decrypt-to-wire step they share.
//!   The peer handlers are `crate::p2p::handlers`; the two files divide on the
//!   same line the two thread pools do.
//! * [`messages`] — every client-visible failure string, in one place.
//!
//! **Errors never carry a filesystem path.** The socket path discloses the
//! local username (CLAUDE.md rule 4), and a `StoreError` from SQLite routinely
//! embeds the database path. Every failure is mapped to one of the fixed
//! sentences in [`messages`]; the underlying error goes to the local log and
//! never onto the wire. Gathering them in one file is what lets a single test
//! pin the whole set.

mod dispatch;
mod items;
mod listener;
mod messages;

pub use listener::{bind, run};
