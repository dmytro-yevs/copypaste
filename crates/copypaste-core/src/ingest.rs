//! The single ingest path every new item goes through.
//!
//! One implementation, four callers: the daemon's clipboard poll loop, the
//! daemon's `add` IPC method, its history import, and the Android backend that
//! links this crate in-process. v1 had two ingest paths that drifted — the IPC
//! one forgot the dedup probe, so `copypaste add` could insert a row the poll
//! loop would have collapsed — and a second copy on the Android side would have
//! been the same defect with a new platform attached.
//!
//! Manifest 01's data-loss rules this file is responsible for:
//!
//! * **I-33** — a dedup lookup failure falls through to the insert. Storing a
//!   duplicate is recoverable; dropping a capture is not.
//! * The refusals are reported, never silent: an over-cap item is
//!   [`IngestError::TooLarge`], not a discarded capture.

use tracing::warn;

use crate::sensitive::Detector;
use crate::storage::{NewItem, Store, StoreError, StoredItem};
use crate::{now_ms, CryptoError, Keyring};

/// What a successful ingest did.
#[derive(Debug)]
pub enum Ingested {
    /// A new row.
    Stored(StoredItem),
    /// The same content was already stored inside the dedup window; this is the
    /// existing row.
    Duplicate(StoredItem),
}

impl Ingested {
    #[must_use]
    pub fn into_item(self) -> StoredItem {
        match self {
            Ingested::Stored(item) | Ingested::Duplicate(item) => item,
        }
    }
}

/// Ingest failures.
///
/// The `Display` text is deliberately free of detail: these strings can end up
/// in an IPC error, and the underlying `StoreError` may carry a database path,
/// which discloses the local username (CLAUDE.md rule 4). The full error is
/// kept as the `source` and goes to the local log, never to a client.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("clipboard content was empty")]
    Empty,
    #[error("the item is larger than the configured size limit")]
    TooLarge,
    #[error("the item could not be encrypted")]
    Crypto(#[from] CryptoError),
    #[error("the item could not be stored")]
    Storage(#[from] StoreError),
}

/// Detect, encrypt, deduplicate, store — the one path into the database.
///
/// Stamped now. Use [`ingest_into`] to keep an imported item's own capture
/// time.
pub fn ingest(
    store: &Store,
    detector: &Detector,
    keyring: &Keyring,
    content: &str,
    content_type: &str,
    settings: &copypaste_ipc::ConfigData,
) -> Result<Ingested, IngestError> {
    ingest_into(
        store,
        detector,
        keyring,
        content,
        content_type,
        now_ms(),
        settings,
    )
}

/// [`ingest`] with the item's own timestamp, for an import.
///
/// A restored item keeps the moment it was originally captured, which is what
/// keeps a restored history in order and its ages honest. The dedup window is
/// applied around *that* stamp, not around now, so importing a file twice
/// collapses rather than doubling.
///
/// Recording which device an item originated on is deliberately **not** here:
/// the origin table belongs to whatever owns sync metadata, and a caller with
/// no peers has none.
#[allow(clippy::too_many_arguments)]
pub fn ingest_into(
    store: &Store,
    detector: &Detector,
    keyring: &Keyring,
    content: &str,
    content_type: &str,
    created_at: i64,
    settings: &copypaste_ipc::ConfigData,
) -> Result<Ingested, IngestError> {
    if content.trim().is_empty() {
        return Err(IngestError::Empty);
    }
    // Refusing a capture is data loss, so the cap has to be a *user's* number
    // rather than a compiled-in one — and the refusal is reported, not silent.
    if content.len() as u64 > settings.max_item_bytes {
        return Err(IngestError::TooLarge);
    }

    let hash = crate::storage::compute_content_hash(content.as_bytes());

    // Manifest 01 I-33: a probe failure must not abort the ingest. Falling
    // through costs at most a duplicate row; returning here costs the capture.
    // `find_recent_by_hash` takes an absolute epoch-ms cutoff, not a duration.
    // Passing the window width itself would compare every row's timestamp
    // against 60000 ms after 1970 and match the entire history, silently
    // collapsing all repeats of a value into the first one ever stored.
    let cutoff_ms = created_at - dedup_window_ms(settings);
    let recent = match store.find_recent_by_hash(&hash, cutoff_ms) {
        Ok(found) => found,
        Err(e) => {
            warn!(error = ?e, "dedup probe failed; storing the item anyway");
            None
        }
    };
    if let Some(row) = recent {
        return Ok(Ingested::Duplicate(row));
    }

    let is_sensitive = detector.is_sensitive(content);

    // The AEAD binds the item id as associated data (manifest 02: "AAD must
    // bind item identity"), and `decrypt` is handed `StoredItem::id` on the way
    // back out. The id therefore has to be chosen *before* the seal — it cannot
    // be assigned by the insert, or the AAD written and the AAD read differ and
    // every row fails authentication on every later read. That is why `NewItem`
    // carries `id` rather than the store minting one.
    let item_id = uuid::Uuid::new_v4().to_string();
    let key = keyring.item_key();
    let (nonce, ciphertext) = crate::encrypt(content.as_bytes(), &key, &item_id)?;

    let stored = store.insert(NewItem {
        id: item_id,
        content_ciphertext: ciphertext,
        nonce,
        content_type: content_type.to_string(),
        content_hash: hash,
        is_sensitive,
        // CLAUDE.md rule 4 / manifest 03 ADR-015: a sensitive item never
        // reaches the search index. This is the write-time layer of that rule;
        // the search handler enforces it again at read time.
        search_text: if is_sensitive {
            None
        } else {
            Some(content.to_string())
        },
        created_at,
    })?;

    // Best-effort, and deliberately after the insert: the item is already
    // durable, and a failed sweep must never turn a stored capture into a lost
    // one.
    if let Err(e) = store.evict_over_cap(u64::from(settings.history_limit)) {
        warn!(error = ?e, "history cap eviction failed");
    }
    // Age-based retention, disabled by the `0` sentinel. Best-effort and after
    // the insert, for the same reason the cap sweep is.
    //
    // Measured from the wall clock, never from `created_at`. `created_at` is
    // caller-supplied and, on the import path, comes straight out of a user's
    // JSON file: one row stamped a year ahead put the cutoff a year ahead too
    // and hard-deleted every unpinned item in the history. Both sync
    // transports already refuse an implausibly-future stamp; import is the
    // third writer and inherits neither guard, so this is where it holds.
    if settings.retention_days > 0 {
        let cutoff = crate::now_ms() - i64::from(settings.retention_days) * 86_400_000;
        if let Err(e) = store.evict_older_than(cutoff) {
            warn!(error = ?e, "age-based retention failed");
        }
    }

    Ok(Ingested::Stored(stored))
}

/// The dedup window, in milliseconds.
///
/// [`crate::storage::DEDUP_WINDOW_MS`] is the default this setting starts at;
/// the setting is what is in force. Two definitions of "the same thing twice"
/// is exactly the duplication CLAUDE.md rule 1 is about, so the constant is
/// referenced only in `ConfigData::default`.
fn dedup_window_ms(settings: &copypaste_ipc::ConfigData) -> i64 {
    i64::from(settings.dedup_window_secs) * 1_000
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::ConfigData;

    const T0: i64 = 1_700_000_000_000;

    struct Fixture {
        store: Store,
        detector: Detector,
        keyring: Keyring,
        settings: ConfigData,
    }

    fn fixture() -> Fixture {
        let keyring = Keyring::from_secret(&[5u8; 32]);
        Fixture {
            store: Store::open_in_memory(&keyring.db_key()).expect("in-memory store"),
            detector: Detector::new().expect("detector"),
            keyring,
            settings: ConfigData::default(),
        }
    }

    impl Fixture {
        fn at(&self, content: &str, created_at: i64) -> Result<Ingested, IngestError> {
            ingest_into(
                &self.store,
                &self.detector,
                &self.keyring,
                content,
                "text",
                created_at,
                &self.settings,
            )
        }

        fn plaintext(&self, item: &StoredItem) -> String {
            let key = self.keyring.item_key();
            let bytes = crate::decrypt(&item.content_ciphertext, &item.nonce, &key, &item.id)
                .expect("the local key must open it");
            String::from_utf8(bytes).unwrap()
        }
    }

    #[test]
    fn a_capture_is_sealed_under_the_item_key_and_indexed() {
        let f = fixture();
        let stored = match f.at("hello from the clipboard", T0).unwrap() {
            Ingested::Stored(item) => item,
            other => panic!("{other:?}"),
        };

        assert_eq!(f.plaintext(&stored), "hello from the clipboard");
        assert_eq!(f.store.search("clipboard", 10).unwrap().len(), 1);
    }

    #[test]
    fn empty_and_blank_content_is_refused_rather_than_stored() {
        let f = fixture();
        assert!(matches!(f.at("", T0), Err(IngestError::Empty)));
        assert!(matches!(f.at("   \n\t ", T0), Err(IngestError::Empty)));
        assert_eq!(f.store.count().unwrap(), 0);
    }

    /// Over the user's cap is a *reported* refusal: a silent drop is a lost
    /// capture the user cannot tell from a broken daemon.
    #[test]
    fn an_item_over_the_configured_cap_is_refused() {
        let mut f = fixture();
        f.settings.max_item_bytes = 16;
        assert!(matches!(
            f.at(&"x".repeat(17), T0),
            Err(IngestError::TooLarge)
        ));
        assert!(f.at(&"x".repeat(16), T0).is_ok());
    }

    /// The `cutoff_ms` argument is an absolute epoch stamp, not a window width.
    /// Passing the width would compare every row against 60 s after 1970 and
    /// collapse every repeat of a value into the first one ever stored.
    #[test]
    fn a_repeat_inside_the_window_deduplicates_and_one_outside_it_does_not() {
        let f = fixture();
        let first = f.at("repeated", T0).unwrap().into_item();

        let inside = f.at("repeated", T0 + 30_000).unwrap();
        assert!(matches!(inside, Ingested::Duplicate(_)));
        assert_eq!(inside.into_item().id, first.id);

        // Past the window the probe misses — and `insert_or_bump` then promotes
        // the row that already holds this content rather than writing a second.
        let outside = f.at("repeated", T0 + 120_000).unwrap();
        assert!(matches!(outside, Ingested::Stored(_)));
        assert_eq!(outside.into_item().id, first.id);
        assert_eq!(f.store.count().unwrap(), 1);
    }

    /// The write-time layer of "a sensitive item never reaches the search
    /// index" (manifest 03 ADR-015).
    #[test]
    fn a_detected_secret_is_flagged_and_kept_out_of_the_index() {
        let f = fixture();
        let stored = f.at("AKIAIOSFODNN7EXAMPLE", T0).unwrap().into_item();

        assert!(stored.is_sensitive);
        assert!(f
            .store
            .search("AKIAIOSFODNN7EXAMPLE", 10)
            .unwrap()
            .is_empty());
        // ...and it is still stored and still readable: flagging is not
        // deleting (CLAUDE.md rule 4).
        assert_eq!(f.plaintext(&stored), "AKIAIOSFODNN7EXAMPLE");
    }

    #[test]
    fn the_history_cap_is_enforced_after_the_insert() {
        let mut f = fixture();
        f.settings.history_limit = 3;
        for n in 0..6 {
            f.at(&format!("item-{n}"), T0 + n * 300_000).unwrap();
        }
        assert_eq!(f.store.count().unwrap(), 3);
        // The newest survives; the cap never eats the capture that triggered it.
        assert_eq!(f.plaintext(&f.store.list(1, 0).unwrap()[0]), "item-5");
    }

    #[test]
    fn age_based_retention_is_off_by_default_and_applied_when_set() {
        // Stamps are relative to the wall clock because the cutoff is.
        let now = crate::now_ms();
        let mut f = fixture();
        f.at("old", now - 10 * 86_400_000).unwrap();
        f.at("new", now).unwrap();
        assert_eq!(f.store.count().unwrap(), 2, "0 days must disable retention");

        f.settings.retention_days = 7;
        f.at("newer", now + 1_000).unwrap();
        assert_eq!(f.store.count().unwrap(), 2, "the old item must be evicted");
    }

    /// An imported row carries a caller-supplied `created_at`. Deriving the
    /// retention cutoff from it let one row stamped in the future delete every
    /// live item in the history.
    #[test]
    fn a_future_stamped_item_evicts_nothing() {
        let now = crate::now_ms();
        let mut f = fixture();
        f.settings.retention_days = 30;
        for n in 0..5 {
            f.at(&format!("item-{n}"), now - i64::from(n) * 60_000)
                .unwrap();
        }
        assert_eq!(f.store.count().unwrap(), 5);

        f.at("imported", now + 60 * 86_400_000).unwrap();
        assert_eq!(
            f.store.count().unwrap(),
            6,
            "a future stamp must not move the cutoff"
        );
    }

    #[test]
    fn ingest_errors_disclose_no_path() {
        for message in [
            IngestError::Empty.to_string(),
            IngestError::TooLarge.to_string(),
            IngestError::Storage(StoreError::InvalidKey).to_string(),
        ] {
            assert!(!message.contains('/'), "{message}");
        }
    }
}
