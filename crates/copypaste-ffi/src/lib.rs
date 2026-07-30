//! CopyPaste, as a native app embeds it.
//!
//! # Why this crate exists
//!
//! macOS runs a background daemon and its app is a thin IPC client. Android
//! does not get that shape — the OS will not host a long-lived background
//! process for a clipboard manager, and the parts of it that would matter most
//! (reading the clipboard while another app is in the foreground) are
//! prohibited outright since Android 10. So the Android app embeds the core
//! directly, and this crate is the whole of the boundary: storage, crypto,
//! secret detection and the peer operations, and nothing else.
//!
//! That makes it load-bearing in a way an ordinary binding layer is not. Every
//! security property the daemon enforces in `server.rs` has to be enforced here
//! too, because there is no daemon on the other side to fall back on.
//!
//! # UniFFI, proc macros only
//!
//! Every exported item is declared with `#[uniffi::export]`, `uniffi::Object`,
//! `uniffi::Record` or `uniffi::Error`. **There is no `.udl` file.** v1 kept
//! one, which meant every signature was written twice — once in Rust and once
//! in the interface file — and the two drifted, because nothing made them
//! agree. Here the Rust *is* the interface definition.
//!
//! There is also **no hand-written ABI version counter**. v1 carried a 366-line
//! `u32` that a human had to remember to bump, duplicating work UniFFI already
//! does: it derives a checksum per function from the actual signature and emits
//! a `uniffi_checksum_*` symbol that the generated Kotlin verifies at load
//! time. A mismatched `.so` and `.kt` pair fails loudly at startup, which is
//! strictly better than a counter someone forgot to increment.
//!
//! # The shape of the API
//!
//! One object, [`CopyPaste`], opened once per process and held for its
//! lifetime. History operations are blocking and belong on `Dispatchers.IO`;
//! peer operations are `async` and arrive in Kotlin as `suspend fun`s.
//!
//! | Kotlin | What it does |
//! |---|---|
//! | `CopyPaste(dataDir, secret, name)` | open the store |
//! | `list`, `search`, `count` | read history |
//! | `add` | store a clipping — the one ingest path |
//! | `itemText` | full plaintext of one item: copy, and reveal |
//! | `delete`, `setPinned` | mutate |
//! | `isSensitive` | ask before storing |
//! | `createPairing`, `acceptPairing` | pair a device |
//! | `listPeers`, `unpair`, `checkPeer` | manage pairings |
//! | `syncPeer` | not available in this build — see [`pairing`] |
//!
//! # The two rules this boundary is responsible for
//!
//! Both are carried from `docs/rewrite/port-manifest/06-ui-behaviour.md`, whose
//! behavioural half is binding, and both are enforced *here* rather than in the
//! app, because a rule the UI has to remember is a rule the UI eventually
//! forgets.
//!
//! * **A sensitive item's plaintext never crosses this boundary as part of a
//!   list.** [`ClipItem::preview`] is empty for a sensitive item and the
//!   content is never decrypted to build one. v1 put the plaintext in the DOM
//!   and blurred it with CSS, which TalkBack, `View.getText()`, screenshots and
//!   the Android assist structure all read straight through (INV-10, A11Y-3).
//! * **No error can carry a filesystem path.** [`CopyPasteError`]'s variants
//!   have no fields at all, so there is nothing to interpolate one into. Kotlin
//!   receives a code and chooses the sentence, which is the "code → copy
//!   mapping" INV-12 asks for (`CLAUDE.md` rule 4).
//!
//! # Generating the bindings
//!
//! ```text
//! cargo run -p copypaste-ffi --features bindgen --bin uniffi-bindgen -- \
//!     generate --library <path-to-libcopypaste_ffi.so> \
//!     --language kotlin --out-dir apps/android/app/src/main/java
//! ```
//!
//! `apps/android/README.md` has the full cross-compilation story, and is honest
//! about which parts of it have been run.

#![forbid(unsafe_code)]

mod error;
mod pairing;
mod store;
mod types;

pub use error::CopyPasteError;
pub use store::{CopyPaste, DB_FILE_NAME, MAX_HISTORY_ITEMS};
pub use types::{ClipItem, NewPairing, PairedDevice, SyncReport, PREVIEW_CHARS};

// Emits the FFI scaffolding for everything the proc macros above collected.
// This is the whole of what a `.udl` file and a `build.rs` used to do.
uniffi::setup_scaffolding!();
