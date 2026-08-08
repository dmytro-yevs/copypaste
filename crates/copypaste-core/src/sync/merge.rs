//! Opening a stored version to send ([`open_version`]) and merging one that
//! arrived ([`apply_remote_version`]) — the two steps both transports share.
//!
//! The merge is re-run here rather than trusted from the caller: a peer session
//! planned from summaries taken before it started writing, and a cloud page was
//! fetched before the local user touched anything. This is the only place both
//! `origin_device_id`s and the current local row exist at once.

use copypaste_p2p::protocol::ItemSummary;
use copypaste_p2p::sync::{merge_decision, pin_state_wins, MergeDecision};
use tracing::{debug, warn};

use super::{MSG_ENCRYPT, MSG_STORE};
use crate::sensitive::Detector;
use crate::storage::{origin_or, IncomingItem, Store, StoredItem, Version};
use crate::Keyring;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenVersionError {
    MissingPayload,
    AuthenticationFailed,
    InvalidPayload,
}

fn open_error(error: &crate::CryptoError) -> OpenVersionError {
    if matches!(error, crate::CryptoError::AuthFailed) {
        OpenVersionError::AuthenticationFailed
    } else {
        OpenVersionError::InvalidPayload
    }
}

/// An unreadable row is reported to the caller, which may skip that row while
/// continuing the sync session with unrelated history.
pub fn open_version_bytes(
    keyring: &Keyring,
    row: &StoredItem,
) -> Result<Vec<u8>, OpenVersionError> {
    if row.content_ciphertext.is_empty() {
        warn!(id = %row.id, "live item has no payload; not sending it");
        return Err(OpenVersionError::MissingPayload);
    }
    if copypaste_ipc::content_type::is_binary(&row.content_type) {
        return crate::open_binary(&row.content_ciphertext, &keyring.item_key(), &row.id).map_err(
            |e| {
                warn!(id = %row.id, error = ?e, "skipping binary item that could not be opened");
                open_error(&e)
            },
        );
    }
    if row.nonce.is_empty() {
        warn!(id = %row.id, "text item has no nonce; not sending it");
        return Err(OpenVersionError::MissingPayload);
    }
    // The item id is the AAD, exactly as the read paths use it.
    match crate::decrypt(
        &row.content_ciphertext,
        &row.nonce,
        &keyring.item_key(),
        &row.id,
    ) {
        Ok(plaintext) => Ok(plaintext),
        Err(e) => {
            warn!(id = %row.id, error = ?e, "skipping an item that could not be opened");
            Err(open_error(&e))
        }
    }
}

/// Open a text version for the P2P wire. Binary callers must use
/// [`open_version_bytes`] so arbitrary bytes can never be lossily stringified.
#[must_use]
pub fn open_version(keyring: &Keyring, row: &StoredItem) -> Result<String, OpenVersionError> {
    if !copypaste_ipc::content_type::is_text(&row.content_type) {
        return Err(OpenVersionError::MissingPayload);
    }
    String::from_utf8(open_version_bytes(keyring, row)?)
        .map_err(|_| OpenVersionError::InvalidPayload)
}

/// One version of one item, arriving from another device.
///
/// Plaintext, because the caller has already opened whatever the transport
/// sealed it with.
pub struct RemoteVersion<'a> {
    pub item_id: &'a str,
    /// UTF-8 payload for text. Empty for a tombstone and for a binary version,
    /// whose bytes are carried in `binary_content`.
    pub content: &'a str,
    /// Raw bytes for an image or file. This is deliberately not a base64
    /// string: callers must choose the binary transport field explicitly.
    pub binary_content: Option<&'a [u8]>,
    /// JSON-encoded file metadata, private to the encrypted local store and
    /// an explicit field on authenticated transports.
    pub payload_metadata: Option<&'a str>,
    pub content_type: &'a str,
    pub created_at: i64,
    pub deleted: bool,
    /// The sender's hash, **read only for a tombstone**.
    ///
    /// A live version's hash is recomputed from `content` here whatever this
    /// says, so no transport can choose merge key 2 for a version it also
    /// supplies the content of. The peer session already drops a live item
    /// whose content does not hash to what it claimed; this makes the property
    /// hold for a caller that is not that session, which is where B-2 could
    /// come back.
    ///
    /// A tombstone is the one thing that cannot be recomputed: it carries the
    /// hash of the item it deletes and no content to hash (manifest 05 T-4).
    /// Taking it on trust is bounded rather than merely tolerated — a forged
    /// value either deletes exactly what an honest one would, or deletes less,
    /// never more, and the direction is what matters under rule 4.
    /// `a_forged_tombstone_hash_can_only_delete_less_than_an_honest_one` pins
    /// it.
    ///
    /// The cloud row deliberately carries no hash at all — one sitting next to
    /// the ciphertext would let the backend confirm a guess at clipboard
    /// content, which is the property the client-side encryption exists to
    /// hold — so it arrives as `None` and a tombstone's is inherited from the
    /// local row instead.
    pub content_hash: Option<&'a str>,
    /// The device that produced *this version*, preserved across hops.
    pub origin_device_id: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeError {
    Store,
    Encrypt,
}

/// The independent outcomes of a P2P merge. Pin state is intentionally
/// separate from content because the cloud transport does not carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P2pApply {
    pub content: bool,
    pub pin: bool,
}

impl P2pApply {
    #[must_use]
    pub fn any(self) -> bool {
        self.content || self.pin
    }
}

impl MergeError {
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            MergeError::Store => MSG_STORE,
            MergeError::Encrypt => MSG_ENCRYPT,
        }
    }
}

/// Merge one remote version into the local history. `Ok(false)` means the local
/// copy won — which is what makes replaying a session or a page a no-op.
///
/// `here` is this device's id, substituted for the empty origin a locally
/// captured row carries ([`origin_or`]). Passing anything else makes every
/// local row compare as a stranger's on merge key 4.
pub fn apply_remote_version(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &RemoteVersion<'_>,
) -> Result<bool, MergeError> {
    if payload_is_refused(incoming) {
        return Ok(false);
    }
    let local = local_version(store, incoming.item_id)?;
    apply_remote_version_with_pin_state(
        store,
        keyring,
        detector,
        here,
        incoming,
        None,
        local.as_ref(),
    )
}

fn local_version(store: &Store, item_id: &str) -> Result<Option<Version>, MergeError> {
    store.version_summary(item_id).map_err(|e| {
        warn!(error = ?e, "could not read the local version of an incoming item");
        MergeError::Store
    })
}

/// Shapes that are never stored, whichever transport carried them. Checked by
/// the entry points so the local row is read exactly once per incoming item.
fn payload_is_refused(incoming: &RemoteVersion<'_>) -> bool {
    if !incoming.deleted
        && copypaste_ipc::content_type::is_binary(incoming.content_type)
        && incoming.binary_content.is_none()
    {
        return true;
    }
    if !incoming.deleted && incoming.content_type == copypaste_ipc::content_type::FILE {
        incoming
            .payload_metadata
            .and_then(crate::FileMetadata::from_json)
            .is_none()
    } else {
        // Metadata belongs only to a file payload. Keeping it on a text/image
        // row would preserve unactionable, user-controlled cleartext forever.
        incoming.payload_metadata.is_some()
    }
}

/// P2P's version of [`apply_remote_version`]. Pin state is authenticated by
/// the Noise record that carried the item, so a winning P2P pin version
/// replaces pin state without restamping content for cloud.
pub fn apply_remote_p2p_version_with_pin_stamp(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &RemoteVersion<'_>,
    pinned: bool,
    pin_order: Option<f64>,
    pin_updated_at: i64,
) -> Result<P2pApply, MergeError> {
    let local = local_version(store, incoming.item_id)?;
    let remote_pin_wins = !incoming.deleted
        && local.as_ref().is_none_or(|local| {
            pin_state_wins(
                &ItemSummary {
                    item_id: local.id.clone(),
                    created_at: local.created_at,
                    deleted: local.deleted,
                    content_hash: local.content_hash.clone(),
                    origin_device_id: origin_or(&local.origin_device_id, here).to_string(),
                    pinned: local.pinned,
                    pin_order: local.pin_order,
                    pin_updated_at: local.pin_updated_at,
                },
                &ItemSummary {
                    item_id: incoming.item_id.to_string(),
                    created_at: incoming.created_at,
                    deleted: incoming.deleted,
                    content_hash: String::new(),
                    origin_device_id: incoming.origin_device_id.to_string(),
                    pinned,
                    pin_order,
                    pin_updated_at,
                },
            )
        });

    let content = if payload_is_refused(incoming) {
        false
    } else {
        apply_remote_version_with_pin_state(
            store,
            keyring,
            detector,
            here,
            incoming,
            Some((pinned, pin_order, pin_updated_at, remote_pin_wins)),
            local.as_ref(),
        )?
    };
    let pin = if !content && remote_pin_wins {
        store
            .apply_pin_state(incoming.item_id, pinned, pin_order, pin_updated_at)
            .map_err(|e| {
                warn!(error = ?e, "could not store incoming P2P pin state");
                MergeError::Store
            })?
    } else {
        false
    };
    Ok(P2pApply { content, pin })
}

/// `local` is read by the entry points rather than here: both of them already
/// need it, and `Store::version` materialises the stored ciphertext, which this
/// path never looks at.
#[allow(clippy::too_many_arguments)]
fn apply_remote_version_with_pin_state(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &RemoteVersion<'_>,
    pin_state: Option<(bool, Option<f64>, i64, bool)>,
    local: Option<&Version>,
) -> Result<bool, MergeError> {
    let content = incoming
        .binary_content
        .unwrap_or(incoming.content.as_bytes());
    // One digest for a binary payload: the same bytes are both merge key 2 and
    // the envelope header the seal below writes.
    let digest = (!incoming.deleted
        && copypaste_ipc::content_type::is_binary(incoming.content_type))
    .then(|| crate::binary::content_digest(content));

    // A tombstone with no hash of its own inherits the one it is deleting: the
    // store keeps `content_hash` on a tombstone deliberately, so inheriting it
    // makes the delete tie its own live version on keys 1 and 2 and win on key
    // 3 (`deleted`). Inventing a different hash there would decide the delete
    // on the wrong key.
    //
    // A live version's hash is never taken from the sender (B-2 / security
    // review F-3): it is merge key 2 and it is the dedup key, and the content
    // that would prove it is right here. See `RemoteVersion::content_hash`.
    let computed;
    let content_hash: &str = match incoming.content_hash {
        Some(hash) if incoming.deleted => hash,
        None if incoming.deleted => local.map_or("", |l| l.content_hash.as_str()),
        _ => {
            computed = match &digest {
                Some(digest) => crate::binary::content_hash(digest),
                None => crate::storage::compute_content_hash(content),
            };
            &computed
        }
    };

    let remote = ItemSummary {
        item_id: incoming.item_id.to_string(),
        created_at: incoming.created_at,
        deleted: incoming.deleted,
        content_hash: content_hash.to_string(),
        origin_device_id: incoming.origin_device_id.to_string(),
        pinned: pin_state.is_some_and(|state| state.0),
        pin_order: pin_state.and_then(|state| state.1),
        pin_updated_at: pin_state.map_or(0, |state| state.2),
    };

    if let Some(local) = local {
        let mine = ItemSummary {
            item_id: local.id.clone(),
            created_at: local.created_at,
            deleted: local.deleted,
            content_hash: local.content_hash.clone(),
            origin_device_id: origin_or(&local.origin_device_id, here).to_string(),
            pinned: local.pinned,
            pin_order: local.pin_order,
            pin_updated_at: local.pin_updated_at,
        };
        if merge_decision(
            &mine,
            origin_or(&local.origin_device_id, here),
            &remote,
            incoming.origin_device_id,
        ) == MergeDecision::KeepLocal
        {
            debug!(id = %incoming.item_id, "local version wins; not applying");
            return Ok(false);
        }
    }

    let is_sensitive = if incoming.deleted {
        local.is_some_and(|l| l.is_sensitive)
    } else {
        copypaste_ipc::content_type::is_text(incoming.content_type)
            && detector.is_sensitive(incoming.content)
    };

    let sealed = if incoming.deleted {
        None
    } else {
        let key = keyring.item_key();
        if let Some(digest) = &digest {
            let ciphertext =
                crate::binary::seal_with_digest(content, digest, &key, incoming.item_id).map_err(
                    |e| {
                        warn!(error = ?e, "could not seal incoming binary item");
                        MergeError::Encrypt
                    },
                )?;
            Some((Vec::new(), ciphertext))
        } else {
            Some(
                crate::encrypt(content, &key, incoming.item_id).map_err(|e| {
                    warn!(error = ?e, "could not seal an incoming item");
                    MergeError::Encrypt
                })?,
            )
        }
    };
    let (nonce, ciphertext) = match &sealed {
        Some((nonce, ciphertext)) => (Some(nonce.as_slice()), Some(ciphertext.as_slice())),
        None => (None, None),
    };
    // The P2P wire always supplies both fields. Cloud deliberately does not
    // carry pin state, so its `None` preserves the receiver's local choice.
    let (pinned, pin_order, pin_updated_at) = if incoming.deleted {
        (false, None, 0)
    } else {
        match pin_state {
            Some((pinned, pin_order, pin_updated_at, true)) => (pinned, pin_order, pin_updated_at),
            Some((_, _, _, false)) | None => local.map_or((false, None, 0), |item| {
                (item.pinned, item.pin_order, item.pin_updated_at)
            }),
        }
    };

    let stored = store
        .upsert(&IncomingItem {
            id: incoming.item_id,
            content_ciphertext: ciphertext,
            nonce,
            content_type: incoming.content_type,
            content_hash,
            created_at: incoming.created_at,
            deleted: incoming.deleted,
            is_sensitive,
            origin_device_id: incoming.origin_device_id,
            pinned,
            pin_order,
            pin_updated_at,
            search_text: if is_sensitive
                || incoming.deleted
                || copypaste_ipc::content_type::is_binary(incoming.content_type)
            {
                None
            } else {
                Some(incoming.content)
            },
            payload_metadata: if incoming.deleted {
                None
            } else {
                incoming.payload_metadata
            },
        })
        .map_err(|e| {
            warn!(error = ?e, "could not store an incoming item");
            MergeError::Store
        })?;

    if stored {
        debug!(id = %incoming.item_id, deleted = incoming.deleted, "applied a remote version");
    }
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::testkit::{fixture, fixture_named, version, Fixture};

    impl Fixture {
        fn apply(&self, incoming: &RemoteVersion<'_>) -> bool {
            apply_remote_version(
                &self.store,
                &self.keyring,
                &self.detector,
                &self.here,
                incoming,
            )
            .expect("merge")
        }

        fn apply_p2p(
            &self,
            incoming: &RemoteVersion<'_>,
            pinned: bool,
            pin_order: Option<f64>,
        ) -> bool {
            apply_remote_p2p_version_with_pin_stamp(
                &self.store,
                &self.keyring,
                &self.detector,
                &self.here,
                incoming,
                pinned,
                pin_order,
                incoming.created_at,
            )
            .expect("merge")
            .any()
        }
    }

    /// The hash a transport omits must be the one the store would have written,
    /// or key 2 of the ordering separates versions that are in fact identical.
    #[test]
    fn an_omitted_hash_is_recomputed_to_the_stores_own_value() {
        let f = fixture();
        assert!(f.apply(&version("a", "shared text", 1_000)));

        let local = f.store.version("a").unwrap().unwrap();
        assert_eq!(
            local.content_hash,
            crate::storage::compute_content_hash(b"shared text")
        );
        // ...and the same version arriving a second time is absorbed.
        assert!(!f.apply(&version("a", "shared text", 1_000)));
    }

    #[test]
    fn a_remote_file_with_a_path_in_its_metadata_is_not_persisted() {
        let f = fixture();
        let payload = b"not actually a PDF";
        let incoming = RemoteVersion {
            item_id: "file-a",
            content: "",
            binary_content: Some(payload),
            payload_metadata: Some(
                r#"{"filename":"../outside.pdf","mime_type":"application/pdf"}"#,
            ),
            content_type: copypaste_ipc::content_type::FILE,
            created_at: 1_000,
            deleted: false,
            content_hash: None,
            origin_device_id: "device-a",
        };

        assert!(!f.apply(&incoming));
        assert!(f.store.get("file-a").unwrap().is_none());
    }

    #[test]
    fn a_non_text_remote_version_is_refused_before_storage() {
        let f = fixture();
        let applied = f.apply(&RemoteVersion {
            content_type: "image/png",
            ..version("image", "encoded image stand-in", 1_000)
        });

        assert!(!applied);
        assert!(f.store.version("image").unwrap().is_none());
    }

    /// The same pair of versions must be decided identically whether the hash
    /// came off the wire (peer) or was recomputed (cloud). This is INV-C2.
    #[test]
    fn both_transports_reach_the_same_decision_for_the_same_pair() {
        let f = fixture();
        let hash = crate::storage::compute_content_hash(b"first");
        f.apply(&version("a", "first", 1_000));

        // Cloud shape: no hash, recomputed.
        let cloud = f.apply(&version("a", "first", 1_000));
        // Peer shape: the sender's hash, which is the same value.
        let peer = f.apply(&RemoteVersion {
            content_hash: Some(&hash),
            ..version("a", "first", 1_000)
        });
        assert_eq!(cloud, peer);
        assert!(!cloud, "a tie must keep the local copy (INV-I1)");
    }

    /// B-2 / security review F-3. A peer that names merge key 2 freely can pick
    /// a hash colliding with an item the receiver already holds, and dedup then
    /// refuses the insert — a chosen clipping silently never lands. The peer
    /// session drops a live item whose content does not hash to what it claimed;
    /// this is the layer below it, so a *caller* that is not that session cannot
    /// reintroduce the gap.
    #[test]
    fn a_live_versions_hash_is_recomputed_and_never_taken_from_the_sender() {
        let f = fixture();
        assert!(f.apply(&RemoteVersion {
            content_hash: Some("0000000000000000000000000000000000000000000000000000000000000000"),
            ..version("a", "the real content", 1_000)
        }));

        let stored = f.store.version("a").unwrap().unwrap();
        assert_eq!(
            stored.content_hash,
            crate::storage::compute_content_hash(b"the real content"),
            "the sender's hash was stored, so it chose merge key 2 and the dedup key"
        );
    }

    /// The loss B-2 names, end to end against a real store: `idx_items_dedup` is
    /// unique over `(content_hash, created_at / 60000)`, and `Store::upsert`
    /// answers a violation with `Ok(false)` — no error, no row. A peer that
    /// could name the hash could therefore aim one at a bucket the receiver
    /// already occupies and a chosen clipping would silently never land.
    #[test]
    fn a_hash_aimed_at_a_bucket_we_already_hold_cannot_suppress_the_item() {
        let f = fixture();
        assert!(f.apply(&version("already-here", "the decoy", 1_000)));
        let occupied = f.store.version("already-here").unwrap().unwrap();

        // Same minute bucket, different item, and the collision is exactly what
        // the sender is asking for.
        assert!(f.apply(&RemoteVersion {
            content_hash: Some(&occupied.content_hash),
            ..version("targeted", "the clipping the peer wants suppressed", 1_500)
        }));

        assert!(
            f.store.get("targeted").unwrap().is_some(),
            "a chosen item was suppressed by a hash the sender picked"
        );
        assert!(f.store.get("already-here").unwrap().is_some());
    }

    /// The one hash still taken on trust, and the bound on what it buys.
    ///
    /// A tombstone has no content to hash, so it cannot be recomputed. What
    /// makes that safe is the direction of the effect: a forged hash either
    /// deletes exactly what an honest one would (`>` the local hash, or equal,
    /// where key 3 `deleted` decides) or deletes *less* (`<`, where key 2 keeps
    /// the local copy). It can never destroy something an honest tombstone
    /// would have left alone, which is the only direction rule 4 cares about.
    #[test]
    fn a_forged_tombstone_hash_can_only_delete_less_than_an_honest_one() {
        let outcome = |forged: &str| {
            let f = fixture();
            f.apply(&version("a", "doomed", 1_000));
            assert_ne!(forged, f.store.version("a").unwrap().unwrap().content_hash);
            // The same instant as the version it deletes: the only tie key 2
            // can break, and the whole of the attacker's room.
            f.apply(&RemoteVersion {
                content: "",
                deleted: true,
                content_hash: Some(forged),
                ..version("a", "", 1_000)
            });
            f.store.get("a").unwrap().is_none()
        };

        assert!(
            outcome("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "a hash above the local one deletes, exactly as an honest one does"
        );
        assert!(
            !outcome("0000000000000000000000000000000000000000000000000000000000000000"),
            "a hash below it must only ever delete less, never more"
        );
    }

    /// `CopyPaste-ojhe` / INV-N2: a delete that ties its own live version on
    /// time and content must win on `deleted`, whichever transport carried it.
    #[test]
    fn a_tombstone_with_no_hash_of_its_own_still_beats_the_live_version() {
        let f = fixture();
        f.apply(&version("a", "doomed", 1_000));

        let applied = f.apply(&RemoteVersion {
            content: "",
            deleted: true,
            // Same instant: only key 3 can decide this.
            ..version("a", "", 1_000)
        });

        assert!(applied, "the delete lost to the version it deletes");
        assert!(f.store.get("a").unwrap().is_none());
        assert!(f.store.version("a").unwrap().unwrap().deleted);
    }

    #[test]
    fn a_tombstone_for_an_unknown_item_is_persisted() {
        // T-3 / CopyPaste-bfiu: without the tombstone there is nothing for a
        // later-arriving create to lose against.
        let f = fixture();
        assert!(f.apply(&RemoteVersion {
            content: "",
            deleted: true,
            ..version("never-seen", "", 5_000)
        }));
        assert!(f.store.version("never-seen").unwrap().unwrap().deleted);

        // ...and the create that arrives late does not resurrect it.
        assert!(!f.apply(&version("never-seen", "back", 1_000)));
        assert!(f.store.get("never-seen").unwrap().is_none());
    }

    /// P2P carries unpin state with the delete, so a later remote version
    /// replaces a local pin rather than leaving two devices permanently apart.
    #[test]
    fn a_p2p_delete_replaces_a_pinned_item() {
        let f = fixture();
        f.apply(&version("keeper", "worth keeping", 1_000));
        assert!(f.store.set_pinned("keeper", true).unwrap());
        let stamp = f.store.version("keeper").unwrap().unwrap().created_at + 1;

        let applied = f.apply_p2p(
            &RemoteVersion {
                content: "",
                deleted: true,
                ..version("keeper", "", stamp)
            },
            false,
            None,
        );

        assert!(applied, "the newer P2P delete was not applied");
        assert!(f.store.get("keeper").unwrap().is_none());
        let row = f.store.version("keeper").unwrap().unwrap();
        assert!(!row.pinned);
        assert_eq!(row.pin_order, None);
    }

    #[test]
    fn a_newer_p2p_pin_does_not_replace_older_content() {
        let f = fixture_named("beta");
        let local = RemoteVersion {
            item_id: "shared",
            content: "new content",
            binary_content: None,
            payload_metadata: None,
            content_type: "text",
            created_at: 200,
            deleted: false,
            content_hash: None,
            origin_device_id: "device-a",
        };
        assert!(f.apply(&local));

        let stale = RemoteVersion {
            item_id: "shared",
            content: "stale content",
            binary_content: None,
            payload_metadata: None,
            content_type: "text",
            created_at: 100,
            deleted: false,
            content_hash: None,
            origin_device_id: "device-b",
        };
        let outcome = apply_remote_p2p_version_with_pin_stamp(
            &f.store,
            &f.keyring,
            &f.detector,
            &f.here,
            &stale,
            true,
            Some(1.0),
            300,
        )
        .unwrap();
        assert!(!outcome.content);
        assert!(outcome.pin);
        let stored = f.store.version("shared").unwrap().unwrap();
        assert_eq!(
            open_version(&f.keyring, &stored).as_deref(),
            Ok("new content")
        );
        assert!(stored.pinned);
        assert_eq!(stored.pin_updated_at, 300);
    }

    /// Cloud omits pin state deliberately, so the shared merge preserves the
    /// receiver's local pin while still accepting a newer content version.
    #[test]
    fn a_cloud_update_keeps_its_local_pin() {
        let f = fixture();
        f.apply(&version("keeper", "first", 1_000));
        f.store.set_pinned("keeper", true).unwrap();
        let stamp = f.store.version("keeper").unwrap().unwrap().created_at + 1;

        assert!(f.apply(&version("keeper", "second", stamp)));
        assert!(f.store.get("keeper").unwrap().unwrap().pinned);
    }

    #[test]
    fn an_incoming_secret_is_flagged_here_and_kept_out_of_the_index() {
        let f = fixture();
        f.apply(&version("leaky", "AKIAIOSFODNN7EXAMPLE", 1_000));

        let row = f.store.get("leaky").unwrap().expect("stored");
        assert!(row.is_sensitive, "the local detector must have the say");
        assert!(f
            .store
            .search("AKIAIOSFODNN7EXAMPLE", 10)
            .unwrap()
            .is_empty());
    }

    /// A locally captured row stores no origin, so the substitution is what
    /// makes this device's own item come back as a tie rather than as a
    /// stranger's version (INV-I2).
    #[test]
    fn this_devices_own_version_coming_back_is_a_tie() {
        let f = fixture();
        let mine = f
            .store
            .insert(crate::NewItem {
                id: "mine".into(),
                content_ciphertext: vec![1],
                nonce: vec![2],
                content_type: "text".into(),
                content_hash: crate::storage::compute_content_hash(b"mine"),
                is_sensitive: false,
                search_text: None,
                created_at: 1_000,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        assert_eq!(mine.origin_device_id, "", "a capture records no origin");

        assert!(!f.apply(&RemoteVersion {
            origin_device_id: &f.here,
            ..version("mine", "mine", 1_000)
        }));
    }

    #[test]
    fn a_tombstone_does_not_clear_the_sensitive_flag() {
        let f = fixture();
        f.apply(&version("leaky", "AKIAIOSFODNN7EXAMPLE", 1_000));
        let stamp = f.store.version("leaky").unwrap().unwrap().created_at + 1;

        assert!(f.apply(&RemoteVersion {
            content: "",
            deleted: true,
            ..version("leaky", "", stamp)
        }));

        let row = f
            .store
            .version_summary("leaky")
            .unwrap()
            .expect("tombstone");
        assert!(row.deleted);
        assert!(
            row.is_sensitive,
            "a delete cleared the sensitive flag; the secret becomes indexable again"
        );
        assert!(f
            .store
            .search("AKIAIOSFODNN7EXAMPLE", 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn error_messages_contain_no_paths() {
        for message in [MSG_STORE, MSG_ENCRYPT] {
            assert!(!message.contains('/'), "{message}");
        }
    }
}
