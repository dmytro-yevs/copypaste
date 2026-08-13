//! The single ingest path every new item goes through.
//!
//! One implementation, four callers: the daemon's clipboard poll loop, the
//! daemon's `add` IPC method, its history import, and the Android backend that
//! links this crate in-process. v1 had two ingest paths that drifted — the IPC
//! one forgot the dedup probe, so `copypaste add` could insert a row the poll
//! loop would have collapsed — and a second copy on the Android side would have
//! been the same defect with a new platform attached.
//!
//! Manifest 01's data-loss rule this file is responsible for: refusals are
//! reported, never silent. An over-cap item is [`IngestError::TooLarge`], not a
//! discarded capture. [`Store::insert_or_bump`] owns the only dedup probe; it
//! exposes no probe-only error that ingest could safely fail open from without
//! maintaining a second implementation.

use crate::retention::RetentionBatch;
use crate::sensitive::Detector;
use crate::storage::{Ingest, NewItem, Store, StoreError, StoredItem};
use crate::{now_ms, CryptoError, Keyring};

/// The retention bound every write here is subject to. Re-exported because
/// `ingest::enforce_retention` is the name the daemon and the sync source
/// already call; [`crate::retention`] owns when it runs.
pub use crate::retention::enforce_retention;

/// What a successful ingest did.
#[derive(Debug)]
pub enum Ingested {
    /// A new row.
    Stored(StoredItem),
    /// The same content was already stored; this is the existing row after the
    /// forward-only recency bump.
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

impl From<Ingest> for Ingested {
    fn from(ingest: Ingest) -> Self {
        match ingest {
            Ingest::Inserted(item) => Self::Stored(item),
            Ingest::Bumped(item) => Self::Duplicate(item),
        }
    }
}

/// Ingest failures.
///
/// The `Display` text is deliberately free of detail: these strings can end up
/// in an IPC error, and the underlying `StoreError` may carry a database path,
/// which discloses the local username (AGENTS.md rule 4). The full error is
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
/// keeps a restored history in order and its ages honest. Dedup spans all live
/// history, so importing a file twice collapses rather than doubling.
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
    ingest_into_with_capture_context(
        store,
        detector,
        keyring,
        content,
        content_type,
        created_at,
        false,
        None,
        settings,
    )
}

/// [`ingest_into`] for one item of a batch, with a persisted sensitive
/// classification that may only make the result stricter. Backups are evidence
/// that a prior detector found a secret; a newer detector failing to recognise
/// it must not re-index it.
///
/// The store and settings come from the [`RetentionBatch`], so the sweep this
/// call defers is necessarily owed on the store it just wrote to. Passing them
/// separately made the token a flag that proved nothing.
pub(crate) fn ingest_into_batched(
    batch: &RetentionBatch<'_>,
    detector: &Detector,
    keyring: &Keyring,
    content: &str,
    content_type: &str,
    created_at: i64,
    sensitive_floor: bool,
) -> Result<Ingested, IngestError> {
    ingest_text(
        batch.store(),
        detector,
        keyring,
        content,
        content_type,
        created_at,
        sensitive_floor,
        None,
        None,
        batch.settings(),
        Sweep::Deferred,
    )
}

/// Ingest a locally captured value with provenance supplied by the platform.
///
/// The app-origin classification is a floor: a known credential store must
/// keep its value out of FTS even when the content detector finds no pattern.
#[allow(clippy::too_many_arguments)]
pub fn ingest_into_with_capture_context(
    store: &Store,
    detector: &Detector,
    keyring: &Keyring,
    content: &str,
    content_type: &str,
    created_at: i64,
    sensitive_floor: bool,
    app_bundle_id: Option<&str>,
    settings: &copypaste_ipc::ConfigData,
) -> Result<Ingested, IngestError> {
    ingest_into_with_capture_source(
        store,
        detector,
        keyring,
        content,
        content_type,
        created_at,
        sensitive_floor,
        app_bundle_id,
        None,
        settings,
    )
}

/// Ingest a locally captured value with a platform-resolved application id and
/// display name. The name is cosmetic only; decisions continue to key on the
/// stable id.
#[allow(clippy::too_many_arguments)]
pub fn ingest_into_with_capture_source(
    store: &Store,
    detector: &Detector,
    keyring: &Keyring,
    content: &str,
    content_type: &str,
    created_at: i64,
    sensitive_floor: bool,
    app_bundle_id: Option<&str>,
    app_name: Option<&str>,
    settings: &copypaste_ipc::ConfigData,
) -> Result<Ingested, IngestError> {
    ingest_text(
        store,
        detector,
        keyring,
        content,
        content_type,
        created_at,
        sensitive_floor,
        app_bundle_id,
        app_name,
        settings,
        Sweep::Now,
    )
}

/// Whether this write pays for its own retention sweep.
enum Sweep {
    Now,
    Deferred,
}

#[allow(clippy::too_many_arguments)]
fn ingest_text(
    store: &Store,
    detector: &Detector,
    keyring: &Keyring,
    content: &str,
    content_type: &str,
    created_at: i64,
    sensitive_floor: bool,
    app_bundle_id: Option<&str>,
    app_name: Option<&str>,
    settings: &copypaste_ipc::ConfigData,
    sweep: Sweep,
) -> Result<Ingested, IngestError> {
    if content.trim().is_empty() {
        return Err(IngestError::Empty);
    }
    // Refusing a capture is data loss, so the cap has to be a *user's* number
    // rather than a compiled-in one — and the refusal is reported, not silent.
    if content.len() as u64 > settings.capture_limit_bytes(content_type) {
        return Err(IngestError::TooLarge);
    }

    // Classify before dedup. A password-manager capture can be identical to a
    // row first copied from an ordinary app; dedup must promote that row rather
    // than returning it with its old, searchable classification.
    // The withholding gate, not the deletion gate: this flag decides indexing,
    // sync and the preview, and `sweep_sensitive` re-derives its own verdict
    // from the plaintext before anything is removed (manifest I2, §6.2).
    let is_sensitive = sensitive_floor || detector.is_sensitive(content);
    let hash = crate::storage::compute_content_hash(content.as_bytes());

    // The AEAD binds the item id as associated data (manifest 02: "AAD must
    // bind item identity"), and `decrypt` is handed `StoredItem::id` on the way
    // back out. The id therefore has to be chosen *before* the seal — it cannot
    // be assigned by the insert, or the AAD written and the AAD read differ and
    // every row fails authentication on every later read. That is why `NewItem`
    // carries `id` rather than the store minting one.
    let item_id = uuid::Uuid::new_v4().to_string();
    let seal_id = item_id.clone();
    let indexable = !is_sensitive && copypaste_ipc::content_type::is_text(content_type);

    let ingested = store.insert_or_bump_late_sealed(
        NewItem {
            id: item_id,
            content_ciphertext: Vec::new(),
            nonce: Vec::new(),
            content_type: content_type.to_string(),
            content_hash: hash,
            is_sensitive,
            search_text: None,
            created_at,
            app_bundle_id: app_bundle_id.map(str::to_owned),
            app_name: app_name.map(str::to_owned),
            payload_metadata: None,
        },
        || -> Result<_, IngestError> {
            let (nonce, ciphertext) =
                crate::encrypt(content.as_bytes(), &keyring.item_key(), &seal_id)?;
            Ok((nonce, ciphertext, indexable.then(|| content.to_string())))
        },
    )?;

    if matches!(sweep, Sweep::Now) {
        enforce_retention(store, settings);
    }

    Ok(ingested.into())
}

/// Store a raw image or file payload.  Binary values have no string
/// representation in the ingest path: treating bytes as lossy UTF-8 is how a
/// file path or base64 stand-in reaches FTS and sync as apparent text.
#[allow(clippy::too_many_arguments)]
pub fn ingest_binary_into_with_capture_context(
    store: &Store,
    keyring: &Keyring,
    bytes: &[u8],
    content_type: &str,
    created_at: i64,
    sensitive_floor: bool,
    app_bundle_id: Option<&str>,
    payload_metadata: Option<&crate::FileMetadata>,
    settings: &copypaste_ipc::ConfigData,
) -> Result<Ingested, IngestError> {
    ingest_binary_into_with_capture_source(
        store,
        keyring,
        bytes,
        content_type,
        created_at,
        sensitive_floor,
        app_bundle_id,
        None,
        payload_metadata,
        settings,
    )
}

/// Binary counterpart of [`ingest_into_with_capture_source`].
#[allow(clippy::too_many_arguments)]
pub fn ingest_binary_into_with_capture_source(
    store: &Store,
    keyring: &Keyring,
    bytes: &[u8],
    content_type: &str,
    created_at: i64,
    sensitive_floor: bool,
    app_bundle_id: Option<&str>,
    app_name: Option<&str>,
    payload_metadata: Option<&crate::FileMetadata>,
    settings: &copypaste_ipc::ConfigData,
) -> Result<Ingested, IngestError> {
    if bytes.is_empty() {
        return Err(IngestError::Empty);
    }
    if copypaste_ipc::content_type::is_text(content_type) {
        return Err(IngestError::Empty);
    }
    if bytes.len() as u64 > settings.capture_limit_bytes(content_type) {
        return Err(IngestError::TooLarge);
    }

    // One SHA-256 pass, four spellings: the item id, the envelope header the
    // STREAM AAD covers, and the row's `content_hash`. Hashing per spelling
    // cost a 4 MiB screenshot three redundant passes on the capture path.
    let digest = crate::binary::content_digest(bytes);
    let item_id = crate::binary::item_id_from_digest(&digest);
    let ciphertext =
        crate::binary::seal_with_digest(bytes, &digest, &keyring.item_key(), &item_id)?;
    let ingested = store.insert_or_bump(NewItem {
        id: item_id,
        content_ciphertext: ciphertext,
        // A binary envelope contains a nonce per chunk.  An empty row nonce
        // distinguishes it from the text AEAD and is never a fallback path.
        nonce: Vec::new(),
        content_type: content_type.to_owned(),
        content_hash: crate::binary::content_hash(&digest),
        is_sensitive: sensitive_floor,
        search_text: None,
        created_at,
        app_bundle_id: app_bundle_id.map(str::to_owned),
        app_name: app_name.map(str::to_owned),
        payload_metadata: payload_metadata
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| IngestError::Empty)?,
    })?;

    enforce_retention(store, settings);

    Ok(ingested.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::fts_row_count;
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

        fn binary_at(&self, bytes: &[u8], created_at: i64) -> Result<Ingested, IngestError> {
            ingest_binary_into_with_capture_context(
                &self.store,
                &self.keyring,
                bytes,
                copypaste_ipc::content_type::IMAGE_PNG,
                created_at,
                false,
                None,
                None,
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
    fn text_uses_the_text_cap_and_accepts_its_boundary() {
        let mut f = fixture();
        f.settings.max_text_size_bytes = 16;
        f.settings.max_image_size_bytes = 32;
        f.settings.max_file_size_bytes = 48;
        assert!(matches!(
            f.at(&"x".repeat(17), T0),
            Err(IngestError::TooLarge)
        ));
        assert!(f.at(&"x".repeat(16), T0).is_ok());
    }

    #[test]
    fn image_and_file_payloads_use_their_own_caps() {
        let mut f = fixture();
        f.settings.max_text_size_bytes = 4;
        f.settings.max_image_size_bytes = 8;
        f.settings.max_file_size_bytes = 12;

        assert!(ingest_binary_into_with_capture_context(
            &f.store,
            &f.keyring,
            &[1; 8],
            copypaste_ipc::content_type::IMAGE_PNG,
            T0,
            false,
            None,
            None,
            &f.settings,
        )
        .is_ok());
        assert!(matches!(
            ingest_binary_into_with_capture_context(
                &f.store,
                &f.keyring,
                &[2; 9],
                copypaste_ipc::content_type::IMAGE_TIFF,
                T0 + 1,
                false,
                None,
                None,
                &f.settings,
            ),
            Err(IngestError::TooLarge)
        ));
        assert!(ingest_binary_into_with_capture_context(
            &f.store,
            &f.keyring,
            &[3; 12],
            copypaste_ipc::content_type::FILE,
            T0 + 2,
            false,
            None,
            None,
            &f.settings,
        )
        .is_ok());
        assert!(matches!(
            ingest_binary_into_with_capture_context(
                &f.store,
                &f.keyring,
                &[4; 13],
                copypaste_ipc::content_type::FILE,
                T0 + 3,
                false,
                None,
                None,
                &f.settings,
            ),
            Err(IngestError::TooLarge)
        ));
    }

    #[test]
    fn a_week_later_text_recopy_bumps_the_original_to_the_top() {
        let f = fixture();
        let first = f.at("repeated", T0).unwrap().into_item();
        let other = f.at("other", T0 + 86_400_000).unwrap().into_item();

        let recopy = f.at("repeated", T0 + 7 * 86_400_000).unwrap();
        assert!(matches!(recopy, Ingested::Duplicate(_)));
        let bumped = recopy.into_item();
        assert_eq!(bumped.id, first.id);
        assert_eq!(bumped.created_at, T0 + 7 * 86_400_000);
        assert_eq!(f.store.count().unwrap(), 2);
        assert_eq!(f.store.list(2, 0).unwrap()[0].id, first.id);
        assert!(f.store.get(&other.id).unwrap().is_some());
    }

    #[test]
    fn a_minimum_timestamp_import_deduplicates_without_overflow() {
        let f = fixture();
        let first = f.at("minimum timestamp", i64::MIN).unwrap().into_item();

        let recopy = f.at("minimum timestamp", i64::MIN).unwrap();
        assert!(matches!(recopy, Ingested::Duplicate(_)));
        let bumped = recopy.into_item();
        assert_eq!(bumped.id, first.id);
        assert_eq!(bumped.created_at, i64::MIN);
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
        // deleting (AGENTS.md rule 4).
        assert_eq!(f.plaintext(&stored), "AKIAIOSFODNN7EXAMPLE");
    }

    #[test]
    fn inert_findings_remain_detected_searchable_and_unclassified() {
        let f = fixture();
        let text = "Send to alice@example.com from 192.168.1.100:8080";
        let findings = f.detector.scan_all(text);
        assert!(findings.iter().any(|finding| finding.rule == "email"));
        assert!(findings
            .iter()
            .any(|finding| finding.rule == "ip_with_port"));
        assert!(findings
            .iter()
            .all(|finding| finding.severity == crate::sensitive::Severity::Flag));

        let stored = f.at(text, T0).unwrap().into_item();
        assert!(!stored.is_sensitive);
        assert_eq!(fts_row_count(&f.store, &stored.id), 1);
        assert_eq!(f.store.search("alice", 10).unwrap().len(), 1);
    }

    #[test]
    fn embedded_high_match_classifies_the_whole_item_but_keeps_low_findings() {
        let f = fixture();
        let text = "mail alice@example.com token AKIAIOSFODNN7EXAMPLE";
        let findings = f.detector.scan_all(text);
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|finding| finding.rule == "email"));
        assert!(findings
            .iter()
            .any(|finding| finding.rule == "aws_access_key"));

        let stored = f.at(text, T0).unwrap().into_item();
        assert!(stored.is_sensitive);
        assert_eq!(fts_row_count(&f.store, &stored.id), 0);
        assert!(f.store.search("alice", 10).unwrap().is_empty());
    }

    #[test]
    fn embedded_luhn_valid_card_authorises_sensitive_classification() {
        let f = fixture();
        let text = "please charge 4111 1111 1111 1111 today";
        let stored = f.at(text, T0).unwrap().into_item();

        assert!(stored.is_sensitive);
        assert_eq!(fts_row_count(&f.store, &stored.id), 0);
        assert!(f.store.search("charge", 10).unwrap().is_empty());
    }

    #[test]
    fn a_sensitive_recopy_promotes_and_unindexes_the_existing_row() {
        let f = fixture();
        let first = f.at("ordinary reusable value", T0).unwrap().into_item();
        assert!(!first.is_sensitive);
        assert_eq!(fts_row_count(&f.store, &first.id), 1);
        assert!(f.detector.scan("ordinary reusable value").is_none());

        let recopy = ingest_into_with_capture_context(
            &f.store,
            &f.detector,
            &f.keyring,
            "ordinary reusable value",
            copypaste_ipc::content_type::TEXT,
            T0 + 7 * 86_400_000,
            true,
            Some("com.1password.1password"),
            &f.settings,
        )
        .unwrap();

        assert!(matches!(recopy, Ingested::Duplicate(_)));
        let promoted = recopy.into_item();
        assert_eq!(promoted.id, first.id);
        assert_eq!(promoted.created_at, T0 + 7 * 86_400_000);
        assert!(promoted.is_sensitive);
        assert_eq!(fts_row_count(&f.store, &first.id), 0);
        assert_eq!(f.plaintext(&promoted), "ordinary reusable value");
        assert_eq!(f.store.count().unwrap(), 1);
    }

    #[test]
    fn a_high_confidence_secret_recopy_restarts_its_ttl() {
        const SECRET: &str = "AKIAIOSFODNN7EXAMPLE";

        let f = fixture();
        let first = f.at(SECRET, T0).unwrap().into_item();
        let recopy = f.at(SECRET, T0 + 25_000).unwrap();
        assert!(matches!(recopy, Ingested::Duplicate(_)));
        let bumped = recopy.into_item();
        assert_eq!(bumped.id, first.id);
        assert_eq!(bumped.created_at, T0 + 25_000);

        let key = f.keyring.item_key();
        let removed = crate::sensitive::sweep_sensitive(
            &f.store,
            &f.detector,
            &key,
            std::time::Duration::from_secs(30),
            T0 + 30_000,
        )
        .unwrap();
        assert_eq!(removed, 0, "the original deadline must no longer apply");
        assert!(f.store.get(&first.id).unwrap().is_some());

        let removed = crate::sensitive::sweep_sensitive(
            &f.store,
            &f.detector,
            &key,
            std::time::Duration::from_secs(30),
            T0 + 56_000,
        )
        .unwrap();
        assert_eq!(removed, 1);
        assert!(f.store.get(&first.id).unwrap().is_none());
    }

    #[test]
    fn string_typed_non_text_is_never_indexed() {
        let f = fixture();
        let stored = ingest_into(
            &f.store,
            &f.detector,
            &f.keyring,
            "encoded image stand-in",
            "image/png",
            T0,
            &f.settings,
        )
        .unwrap()
        .into_item();

        assert_eq!(stored.content_type, "image/png");
        assert!(f.store.search("encoded", 10).unwrap().is_empty());
    }

    #[test]
    fn binary_ingest_is_chunked_content_addressed_and_never_indexed() {
        let f = fixture();
        let bytes = vec![0x89; crate::CHUNK_BYTES + 19];
        let stored = ingest_binary_into_with_capture_context(
            &f.store,
            &f.keyring,
            &bytes,
            copypaste_ipc::content_type::IMAGE_PNG,
            T0,
            false,
            Some("com.apple.Preview"),
            None,
            &f.settings,
        )
        .unwrap()
        .into_item();

        assert_eq!(stored.id, crate::binary_item_id(&bytes));
        assert!(stored.nonce.is_empty());
        assert!(f.store.search("89", 10).unwrap().is_empty());
        assert_eq!(
            crate::open_binary(
                &stored.content_ciphertext,
                &f.keyring.item_key(),
                &stored.id
            )
            .unwrap(),
            bytes
        );
    }

    #[test]
    fn a_week_later_binary_recopy_bumps_the_original_to_the_top() {
        let f = fixture();
        let bytes = vec![0x89; 128];
        let first = f.binary_at(&bytes, T0).unwrap().into_item();
        let other = f
            .binary_at(&[0x42; 64], T0 + 86_400_000)
            .unwrap()
            .into_item();

        let recopy = f.binary_at(&bytes, T0 + 7 * 86_400_000).unwrap();
        assert!(matches!(recopy, Ingested::Duplicate(_)));
        let bumped = recopy.into_item();
        assert_eq!(bumped.id, first.id);
        assert_eq!(bumped.id, crate::binary_item_id(&bytes));
        assert_eq!(bumped.created_at, T0 + 7 * 86_400_000);
        assert_eq!(f.store.count().unwrap(), 2);
        assert_eq!(f.store.list(2, 0).unwrap()[0].id, first.id);
        assert!(f.store.get(&other.id).unwrap().is_some());
    }

    #[test]
    fn binary_insert_enforces_the_item_cap_and_keeps_pin_and_newest() {
        let mut f = fixture();
        f.settings.history_limit = 2;
        let pinned = f.binary_at(&[0x11; 32], T0).unwrap().into_item();
        assert!(f.store.set_pinned(&pinned.id, true).unwrap());
        let old = f.binary_at(&[0x22; 32], T0 + 1_000).unwrap().into_item();
        let newest = f.binary_at(&[0x33; 32], T0 + 2_000).unwrap().into_item();

        assert_eq!(f.store.count().unwrap(), 2);
        assert!(f.store.get(&pinned.id).unwrap().unwrap().pinned);
        assert!(f.store.get(&old.id).unwrap().is_none());
        assert!(f.store.get(&newest.id).unwrap().is_some());
        let page = f.store.list(2, 0).unwrap();
        assert_eq!(page[0].id, pinned.id);
        assert_eq!(page[1].id, newest.id);
    }

    #[test]
    fn binary_bump_enforces_byte_quota_and_keeps_pin_and_newest() {
        let mut f = fixture();
        let pinned = f.binary_at(&[0x11; 32], T0).unwrap().into_item();
        assert!(f.store.set_pinned(&pinned.id, true).unwrap());
        let old = f.binary_at(&[0x22; 32], T0 + 1_000).unwrap().into_item();
        let target = vec![0x33; 32];
        let newest = f.binary_at(&target, T0 + 2_000).unwrap().into_item();

        f.settings.storage_quota_bytes = 0;
        let recopy = f.binary_at(&target, T0 + 7 * 86_400_000).unwrap();
        assert!(matches!(recopy, Ingested::Duplicate(_)));
        assert_eq!(recopy.into_item().id, newest.id);

        assert_eq!(f.store.count().unwrap(), 2);
        assert!(f.store.get(&pinned.id).unwrap().unwrap().pinned);
        assert!(f.store.get(&old.id).unwrap().is_none());
        assert!(f.store.get(&newest.id).unwrap().is_some());
    }

    #[test]
    fn binary_insert_enforces_age_retention() {
        let now = crate::now_ms();
        let mut f = fixture();
        let old = f.at("old text", now - 10 * 86_400_000).unwrap().into_item();

        f.settings.retention_days = 7;
        let newest = f.binary_at(&[0x44; 32], now).unwrap().into_item();

        assert!(f.store.get(&old.id).unwrap().is_none());
        assert!(f.store.get(&newest.id).unwrap().is_some());
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
    fn a_duplicate_capture_runs_byte_retention_after_a_quota_change() {
        let mut f = fixture();
        f.at("older", T0).unwrap();
        f.at("newer", T0 + 120_000).unwrap();

        // Config validation rejects this value for a user-facing setting; this
        // direct fixture proves that the duplicate branch actually runs the
        // retention hook rather than returning before it.
        f.settings.storage_quota_bytes = 0;
        assert!(matches!(
            f.at("newer", T0 + 150_000).unwrap(),
            Ingested::Duplicate(_)
        ));

        assert!(f.store.search("older", 10).unwrap().is_empty());
        assert_eq!(f.plaintext(&f.store.list(1, 0).unwrap()[0]), "newer");
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
