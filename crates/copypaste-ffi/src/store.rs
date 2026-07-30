//! [`CopyPaste`] — the handle a native app holds, and its history operations.
//!
//! The peer operations live in [`crate::pairing`]; they are on the same object
//! but they are network I/O and `async`, which everything here is not.
//!
//! # Where the device secret comes from
//!
//! [`CopyPaste::open`] is handed 32 bytes and derives everything from them
//! through `copypaste_core::Keyring`, exactly as the macOS daemon does. It does
//! **not** call `Keyring::load_or_create`, which reads the macOS Keychain or a
//! `0600` file — neither exists on Android, and the file fallback is explicitly
//! a development posture.
//!
//! Android's answer is the Keystore, and the Keystore cannot hand back raw key
//! material: a hardware-backed key is non-exportable by design. So the app
//! generates the 32-byte secret once, wraps it with a Keystore-held AES key,
//! stores only the wrapped blob, and unwraps it here. The plaintext secret
//! exists in the app process for as long as the store is open and nowhere else.
//! Keeping that policy on the Kotlin side is deliberate — it is the half that
//! is platform-specific, and the half a reviewer with a device can actually
//! audit. See `apps/android/README.md`.
//!
//! # One ingest path
//!
//! [`CopyPaste::add`] is the only way an item enters the database, and it is
//! the same shape as the daemon's `capture::ingest`: probe for a duplicate,
//! detect, seal, insert, evict. v1 had two ingest paths that drifted and one of
//! them forgot the dedup probe.

use std::path::PathBuf;
use std::sync::Arc;

use copypaste_core::{Detector, Keyring, NewItem, Store, StoredItem};

use crate::error::CopyPasteError;
use crate::types::ClipItem;

/// File name of the v2 database.
///
/// `CLAUDE.md` rule 3: v2 must never open, or appear to open, a v0.4.x
/// database. The `-v2` suffix is what guarantees that — an old file is a
/// different name and is simply never touched, so a user who downgrades finds
/// their history where they left it. Same name the daemon uses, so a support
/// answer is the same on both platforms.
pub const DB_FILE_NAME: &str = "copypaste-v2.db";

/// Live items kept on the device.
///
/// Lower than the daemon's 10 000: this is a phone. Every row costs an AEAD
/// open to preview and the whole database is in the app's private storage,
/// which the user may be paying for in a way they do not pay for a laptop's SSD.
/// Pinned items are exempt inside the store, and so is the newest unpinned item
/// — copying something huge must never make it vanish.
pub const MAX_HISTORY_ITEMS: u64 = 2_000;

/// The whole of CopyPaste, embedded.
///
/// Cheap to hold and safe to share across threads: `Store` is a reference
/// counted pool and `Detector` is immutable once compiled. The app should open
/// one of these and keep it for the process lifetime — opening it per screen
/// would recompile the ~100-rule regex set every time.
#[derive(uniffi::Object)]
pub struct CopyPaste {
    pub(crate) store: Store,
    pub(crate) keyring: Keyring,
    pub(crate) detector: Detector,
    pub(crate) peers: copypaste_p2p::PeerStore,
    /// This device's name, as offered to a peer during pairing. Cosmetic.
    pub(crate) device_name: String,
}

impl std::fmt::Debug for CopyPaste {
    /// Deliberately opaque, and not derived.
    ///
    /// A derived impl would reach into `Keyring` and `Store`. Both are opaque
    /// themselves today — `Store`'s is opaque precisely because the pool's own
    /// `Debug` prints the database path — but a struct that holds a device
    /// secret and a database handle should not have a formatting story that
    /// depends on its fields keeping their manners.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CopyPaste { .. }")
    }
}

#[uniffi::export]
impl CopyPaste {
    /// Open the store in `data_dir`, deriving every key from `device_secret`.
    ///
    /// `data_dir` must already exist and should be the app's private directory
    /// (`Context.getFilesDir()`), which is scoped storage: no permission is
    /// required, no other app can read it, and it is removed with the app. It
    /// is a parameter rather than something this crate computes because
    /// `directories`' notion of a data dir is meaningless on Android.
    ///
    /// `device_secret` must be exactly 32 bytes of CSPRNG output. A secret that
    /// does not match the one the database was created with yields
    /// [`CopyPasteError::Locked`] — never a fallback read.
    ///
    /// # Errors
    ///
    /// [`CopyPasteError::BadDeviceSecret`], [`CopyPasteError::Locked`],
    /// [`CopyPasteError::Storage`], [`CopyPasteError::LegacyData`].
    #[uniffi::constructor]
    pub fn open(
        data_dir: String,
        device_secret: Vec<u8>,
        device_name: String,
    ) -> Result<Arc<Self>, CopyPasteError> {
        let secret: [u8; 32] = device_secret
            .as_slice()
            .try_into()
            .map_err(|_| CopyPasteError::BadDeviceSecret)?;
        let keyring = Keyring::from_secret(&secret);

        let dir = PathBuf::from(data_dir);
        let store = Store::open(&dir.join(DB_FILE_NAME), &keyring.db_key())?;
        let peers = copypaste_p2p::PeerStore::open(
            &dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME),
        )?;
        // Compiling the rule table is the expensive part of construction and it
        // is why this object is meant to be long-lived.
        let detector = Detector::new().map_err(|e| {
            tracing::error!(error = ?e, "the secret-detection rule table failed to compile");
            CopyPasteError::Storage
        })?;

        Ok(Arc::new(Self {
            store,
            keyring,
            detector,
            peers,
            device_name: trim_device_name(&device_name),
        }))
    }

    /// One page of history: pinned first, then newest first.
    ///
    /// The ordering is total (the store breaks ties on id), which is what keeps
    /// `offset` paging from duplicating or skipping rows that share a
    /// timestamp. Sensitive items appear in the page — the user must be able to
    /// see that they exist, and to delete them — but carry no preview text.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<ClipItem>, CopyPasteError> {
        self.rows(self.store.list(limit, offset)?)
    }

    /// Full-text search.
    ///
    /// A sensitive item can never appear here: it was never written to the
    /// index, and the search JOIN filters on `is_sensitive = 0` as well
    /// (manifest 03 ADR-015). Nothing in this crate needs to re-filter, and
    /// nothing in this crate should — a fourth layer that silently did the work
    /// would hide the day one of the first three broke.
    pub fn search(&self, query: String, limit: u32) -> Result<Vec<ClipItem>, CopyPasteError> {
        self.rows(self.store.search(&query, limit)?)
    }

    /// Store a new clipping. The one path into the database.
    ///
    /// Deduplicates against the last 60 seconds, so a caller that hands us the
    /// same clipboard content twice gets the original row back rather than a
    /// second copy.
    ///
    /// # Errors
    ///
    /// [`CopyPasteError::EmptyContent`] when there is nothing to store — an
    /// ordinary state, not a fault. [`CopyPasteError::Crypto`],
    /// [`CopyPasteError::Storage`].
    pub fn add(&self, text: String, content_type: String) -> Result<ClipItem, CopyPasteError> {
        if text.trim().is_empty() {
            return Err(CopyPasteError::EmptyContent);
        }

        let hash = copypaste_core::storage::compute_content_hash(text.as_bytes());

        // Manifest 01 I-33: a failed dedup probe falls through to the insert.
        // A duplicate row is recoverable; a dropped capture is not, and data
        // loss is the worst outcome.
        let cutoff = copypaste_core::now_ms() - copypaste_core::storage::DEDUP_WINDOW_MS;
        let recent = self.store.find_recent_by_hash(&hash, cutoff).unwrap_or_else(|e| {
            tracing::warn!(error = ?e, "dedup probe failed; storing the item anyway");
            None
        });
        if let Some(existing) = recent {
            return self.row(&existing);
        }

        let is_sensitive = self.detector.is_sensitive(&text);

        // The AEAD binds the id as associated data, so the id has to exist
        // before the seal — which is why `NewItem` carries one rather than the
        // store minting it. Getting this backwards writes rows whose ciphertext
        // authenticates against an id they do not have, and every later read of
        // them fails closed: a silent, total loss of the item.
        let id = uuid::Uuid::new_v4().to_string();
        let (nonce, ciphertext) =
            copypaste_core::encrypt(text.as_bytes(), &self.keyring.item_key(), &id)?;

        let stored = self.store.insert(NewItem {
            id,
            content_ciphertext: ciphertext,
            nonce,
            content_type,
            content_hash: hash,
            is_sensitive,
            // ADR-015, write-time layer. The store enforces this again rather
            // than trusting us, and so does the search JOIN.
            search_text: if is_sensitive { None } else { Some(text.clone()) },
            created_at: copypaste_core::now_ms(),
        })?;

        // After the insert and best-effort: the item is already durable, and a
        // failed sweep must never turn a stored capture into a lost one.
        if let Err(e) = self.store.evict_over_cap(MAX_HISTORY_ITEMS) {
            tracing::warn!(error = ?e, "history cap eviction failed");
        }

        self.row(&stored)
    }

    /// The full plaintext of one item.
    ///
    /// This is the **only** way plaintext leaves the core for a single item,
    /// and it serves both of the two things a user can ask for: putting the
    /// item on the system clipboard, and revealing a sensitive one on screen.
    /// They are one call on purpose — two would be two places to audit, and the
    /// core cannot tell them apart anyway.
    ///
    /// It does not mutate anything. In particular it does not re-insert the
    /// item to float it to the top of the list: manifest 06 INV-31 says a
    /// pinned item must not jump on copy, and the dedup window would collapse
    /// the write for an unpinned one regardless.
    ///
    /// The caller owns what happens next. If the item is sensitive, the app is
    /// responsible for re-hiding it (INV-11) and for keeping it out of the
    /// accessibility tree until the user asks.
    ///
    /// # Errors
    ///
    /// [`CopyPasteError::ItemNotFound`], [`CopyPasteError::Crypto`],
    /// [`CopyPasteError::Storage`].
    pub fn item_text(&self, id: String) -> Result<String, CopyPasteError> {
        let item = self.store.get(&id)?.ok_or(CopyPasteError::ItemNotFound)?;
        self.decrypt(&item)
    }

    /// Soft-delete an item. Returns whether there was one to delete.
    ///
    /// The ciphertext and nonce are wiped and the search-index row goes with
    /// them, in the same transaction. The row survives as a tombstone so a
    /// later sync can propagate the delete rather than resurrecting the item
    /// from a peer that has not heard about it.
    pub fn delete(&self, id: String) -> Result<bool, CopyPasteError> {
        Ok(self.store.delete(&id)?)
    }

    /// Pin or unpin. Returns whether there was an item to change.
    ///
    /// A pinned item is exempt from every eviction path.
    pub fn set_pinned(&self, id: String, pinned: bool) -> Result<bool, CopyPasteError> {
        Ok(self.store.set_pinned(&id, pinned)?)
    }

    /// How many live items there are. For the empty state and for pagination.
    pub fn count(&self) -> Result<u64, CopyPasteError> {
        Ok(self.store.count()?)
    }

    /// Would this text be flagged as sensitive if it were stored?
    ///
    /// For a "this looks like a password — store it anyway?" prompt, and for a
    /// capture path that wants to decide before writing. Content-only: it
    /// cannot see which app the text came from.
    pub fn is_sensitive(&self, text: String) -> bool {
        self.detector.is_sensitive(&text)
    }

    /// What this device calls itself when it introduces itself to a peer.
    pub fn device_name(&self) -> String {
        self.device_name.clone()
    }
}

// --------------------------------------------------------------- private glue

impl CopyPaste {
    /// Decrypt one item's content.
    fn decrypt(&self, item: &StoredItem) -> Result<String, CopyPasteError> {
        let plain = copypaste_core::decrypt(
            &item.content_ciphertext,
            &item.nonce,
            &self.keyring.item_key(),
            &item.id,
        )?;
        // Clipboard content is text. A byte sequence that is not valid UTF-8
        // came from a caller that lied about `content_type`; replacing the bad
        // bytes shows the user something rather than failing the whole read.
        Ok(String::from_utf8_lossy(&plain).into_owned())
    }

    /// Project one stored row into a list row.
    fn row(&self, item: &StoredItem) -> Result<ClipItem, CopyPasteError> {
        // The sensitive branch does not decrypt at all. That is the point: the
        // plaintext of a sensitive item is never materialised on the path that
        // ends in a Compose node, so there is no string for the view tree or
        // the accessibility tree to pick up (INV-10).
        let plaintext = if item.is_sensitive {
            None
        } else {
            Some(self.decrypt(item)?)
        };
        Ok(ClipItem::from_stored(item, plaintext.as_deref()))
    }

    /// Project a page.
    ///
    /// One unreadable row must not fail the whole page — the rest of the
    /// history is still worth showing, and a page that errors out is a history
    /// screen the user cannot open at all.
    fn rows(&self, items: Vec<StoredItem>) -> Result<Vec<ClipItem>, CopyPasteError> {
        let mut out = Vec::with_capacity(items.len());
        for item in &items {
            match self.row(item) {
                Ok(row) => out.push(row),
                Err(e) => tracing::warn!(id = %item.id, error = ?e, "skipping an unreadable row"),
            }
        }
        Ok(out)
    }
}

/// A device always has *some* name, and never an unbounded one.
fn trim_device_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Android device".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const SECRET: [u8; 32] = [42u8; 32];

    /// A real store on a real (temporary) SQLCipher file, opened exactly the
    /// way the app opens it.
    pub(crate) fn open() -> (Arc<CopyPaste>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let cp = CopyPaste::open(
            dir.path().to_string_lossy().into_owned(),
            SECRET.to_vec(),
            "Pixel".into(),
        )
        .expect("open");
        (cp, dir)
    }

    #[test]
    fn a_secret_of_the_wrong_length_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let err = CopyPaste::open(
            dir.path().to_string_lossy().into_owned(),
            vec![1u8; 31],
            "Pixel".into(),
        )
        .unwrap_err();
        assert_eq!(err, CopyPasteError::BadDeviceSecret);
    }

    #[test]
    fn a_different_secret_does_not_open_the_database() {
        // Fail closed (CLAUDE.md rule 4). Not "empty history", not a fresh
        // database on top of the old one: an error.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let cp = CopyPaste::open(path.clone(), SECRET.to_vec(), "Pixel".into()).unwrap();
        cp.add("hello".into(), "text".into()).unwrap();
        drop(cp);

        let err = CopyPaste::open(path, vec![7u8; 32], "Pixel".into()).unwrap_err();
        assert_eq!(err, CopyPasteError::Locked);
    }

    #[test]
    fn the_database_file_is_the_v2_name() {
        // CLAUDE.md rule 3: a v1 file must never be touched, and a distinct
        // name is the whole mechanism.
        let (_cp, dir) = open();
        assert!(dir.path().join(DB_FILE_NAME).exists());
        assert!(DB_FILE_NAME.contains("v2"));
    }

    #[test]
    fn an_item_round_trips() {
        let (cp, _dir) = open();
        let added = cp.add("hello there".into(), "text".into()).unwrap();
        assert_eq!(added.preview, "hello there");
        assert_eq!(cp.item_text(added.id.clone()).unwrap(), "hello there");
        assert_eq!(cp.list(10, 0).unwrap().len(), 1);
        assert_eq!(cp.count().unwrap(), 1);
    }

    #[test]
    fn empty_content_is_refused_rather_than_stored() {
        let (cp, _dir) = open();
        assert_eq!(
            cp.add("   \n\t ".into(), "text".into()).unwrap_err(),
            CopyPasteError::EmptyContent
        );
        assert_eq!(cp.count().unwrap(), 0);
    }

    #[test]
    fn the_same_content_twice_is_one_item() {
        let (cp, _dir) = open();
        let first = cp.add("repeated".into(), "text".into()).unwrap();
        let second = cp.add("repeated".into(), "text".into()).unwrap();
        assert_eq!(first.id, second.id, "the dedup probe must return the original");
        assert_eq!(cp.count().unwrap(), 1);
    }

    #[test]
    fn plaintext_never_reaches_the_database_file() {
        let (cp, dir) = open();
        cp.add("a very distinctive phrase".into(), "text".into())
            .unwrap();
        let bytes = std::fs::read(dir.path().join(DB_FILE_NAME)).unwrap();
        let needle = b"a very distinctive phrase";
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle),
            "plaintext reached the database file"
        );
    }

    // ------------------------------------------------- the sensitive-item rule

    /// An AWS access key id. Manifest 07's canonical high-confidence shape.
    const SECRET_TEXT: &str = "AKIAIOSFODNN7EXAMPLE";

    #[test]
    fn a_sensitive_item_is_flagged_and_carries_no_preview() {
        // INV-10 / A11Y-3, enforced where it cannot be undone by a layout
        // change: the plaintext is not in the value Kotlin receives.
        let (cp, _dir) = open();
        let added = cp.add(SECRET_TEXT.into(), "text".into()).unwrap();
        assert!(added.is_sensitive);
        assert!(added.preview.is_empty());

        let listed = cp.list(10, 0).unwrap();
        assert_eq!(listed.len(), 1, "it must still be visible as a row");
        assert!(listed[0].is_sensitive);
        assert!(listed[0].preview.is_empty());
    }

    #[test]
    fn no_list_response_anywhere_contains_a_sensitive_items_plaintext() {
        // The property stated as the app sees it: whatever the history screen
        // is handed, the secret is not in it.
        let (cp, _dir) = open();
        cp.add(SECRET_TEXT.into(), "text".into()).unwrap();
        cp.add("something ordinary".into(), "text".into()).unwrap();

        for row in cp.list(50, 0).unwrap() {
            assert!(!row.preview.contains(SECRET_TEXT));
        }
        for row in cp.search("ordinary".into(), 50).unwrap() {
            assert!(!row.preview.contains(SECRET_TEXT));
        }
    }

    #[test]
    fn a_sensitive_item_is_not_searchable() {
        // ADR-015. Three layers enforce it inside the store; this asserts the
        // property survives all the way out to the boundary.
        let (cp, _dir) = open();
        cp.add(SECRET_TEXT.into(), "text".into()).unwrap();
        assert!(cp.search(SECRET_TEXT.into(), 10).unwrap().is_empty());
        assert!(cp.search("AKIA".into(), 10).unwrap().is_empty());
    }

    #[test]
    fn revealing_a_sensitive_item_is_a_separate_deliberate_call() {
        // The user must still be able to copy their own password. What the rule
        // forbids is the *list* carrying it.
        let (cp, _dir) = open();
        let added = cp.add(SECRET_TEXT.into(), "text".into()).unwrap();
        assert_eq!(cp.item_text(added.id).unwrap(), SECRET_TEXT);
    }

    #[test]
    fn is_sensitive_answers_without_storing_anything() {
        let (cp, _dir) = open();
        assert!(cp.is_sensitive(SECRET_TEXT.into()));
        assert!(!cp.is_sensitive("just some words".into()));
        assert_eq!(cp.count().unwrap(), 0);
    }

    // ------------------------------------------------------- mutations & order

    #[test]
    fn deleting_removes_the_item_and_its_text() {
        let (cp, _dir) = open();
        let added = cp.add("doomed".into(), "text".into()).unwrap();
        assert!(cp.delete(added.id.clone()).unwrap());
        assert!(!cp.delete(added.id.clone()).unwrap(), "a second delete is a no-op");
        assert_eq!(
            cp.item_text(added.id).unwrap_err(),
            CopyPasteError::ItemNotFound
        );
        assert_eq!(cp.count().unwrap(), 0);
    }

    #[test]
    fn pinning_floats_an_item_above_newer_ones() {
        let (cp, _dir) = open();
        let old = cp.add("older".into(), "text".into()).unwrap();
        cp.add("newer".into(), "text".into()).unwrap();
        assert_eq!(cp.list(10, 0).unwrap()[0].preview, "newer");

        assert!(cp.set_pinned(old.id.clone(), true).unwrap());
        let listed = cp.list(10, 0).unwrap();
        assert_eq!(listed[0].preview, "older");
        assert!(listed[0].pinned);

        assert!(cp.set_pinned(old.id, false).unwrap());
        assert_eq!(cp.list(10, 0).unwrap()[0].preview, "newer");
    }

    #[test]
    fn pinning_something_that_is_not_there_is_false_not_an_error() {
        let (cp, _dir) = open();
        assert!(!cp.set_pinned("no-such-id".into(), true).unwrap());
    }

    #[test]
    fn reading_an_item_that_is_gone_says_so() {
        let (cp, _dir) = open();
        assert_eq!(
            cp.item_text("no-such-id".into()).unwrap_err(),
            CopyPasteError::ItemNotFound
        );
    }

    #[test]
    fn copying_an_item_does_not_reorder_the_list() {
        // INV-31. `item_text` is a read; nothing about it may move a row, least
        // of all a pinned one.
        let (cp, _dir) = open();
        let first = cp.add("first".into(), "text".into()).unwrap();
        cp.add("second".into(), "text".into()).unwrap();
        let before: Vec<String> = cp.list(10, 0).unwrap().into_iter().map(|r| r.id).collect();

        cp.item_text(first.id).unwrap();

        let after: Vec<String> = cp.list(10, 0).unwrap().into_iter().map(|r| r.id).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn paging_covers_every_item_exactly_once() {
        // The store's ordering is total, which is what makes this true even
        // when several items share a millisecond.
        let (cp, _dir) = open();
        for n in 0..12 {
            cp.add(format!("item {n}"), "text".into()).unwrap();
        }
        let mut seen = Vec::new();
        for page in 0..4u32 {
            seen.extend(cp.list(3, page * 3).unwrap().into_iter().map(|r| r.id));
        }
        assert_eq!(seen.len(), 12);
        let unique: std::collections::HashSet<_> = seen.iter().collect();
        assert_eq!(unique.len(), 12, "a page duplicated or skipped a row");
    }

    #[test]
    fn a_long_item_is_previewed_short_but_copied_whole() {
        let (cp, _dir) = open();
        let long = "z".repeat(crate::types::PREVIEW_CHARS * 3);
        let added = cp.add(long.clone(), "text".into()).unwrap();
        assert!(added.truncated);
        assert_eq!(added.preview.chars().count(), crate::types::PREVIEW_CHARS);
        assert_eq!(cp.item_text(added.id).unwrap(), long);
    }

    #[test]
    fn the_device_name_is_never_empty_and_never_unbounded() {
        let dir = tempfile::tempdir().unwrap();
        let cp = CopyPaste::open(
            dir.path().to_string_lossy().into_owned(),
            SECRET.to_vec(),
            "   ".into(),
        )
        .unwrap();
        assert!(!cp.device_name().is_empty());

        let dir2 = tempfile::tempdir().unwrap();
        let cp2 = CopyPaste::open(
            dir2.path().to_string_lossy().into_owned(),
            SECRET.to_vec(),
            "n".repeat(500),
        )
        .unwrap();
        assert_eq!(cp2.device_name().chars().count(), 64);
    }
}
