//! Opening a stored version to send ([`open_version`]) and merging one that
//! arrived ([`apply_remote_version`]) — the two steps both transports share.
//!
//! The merge is re-run here rather than trusted from the caller: a peer session
//! planned from summaries taken before it started writing, and a cloud page was
//! fetched before the local user touched anything. This is the only place both
//! `origin_device_id`s and the current local row exist at once.

use copypaste_p2p::protocol::ItemSummary;
use copypaste_p2p::sync::{merge_decision, MergeDecision};
use tracing::{debug, info, warn};

use super::{MSG_ENCRYPT, MSG_STORE};
use crate::sensitive::Detector;
use crate::storage::{origin_or, IncomingItem, Store, StoredItem};
use crate::Keyring;

/// Open a stored version under the local item key, ready to leave the device.
///
/// `None` means "do not send this one": a tombstone carries no payload by rule
/// (manifest 05 T-4) and is handled by the caller, while an unreadable row or a
/// live row with no ciphertext must not fail a whole session — the rest of the
/// history is still worth syncing.
#[must_use]
pub fn open_version(keyring: &Keyring, row: &StoredItem) -> Option<String> {
    if row.content_ciphertext.is_empty() || row.nonce.is_empty() {
        warn!(id = %row.id, "live item has no payload; not sending it");
        return None;
    }
    // The item id is the AAD, exactly as the read paths use it.
    match crate::decrypt(
        &row.content_ciphertext,
        &row.nonce,
        &keyring.item_key(),
        &row.id,
    ) {
        Ok(plaintext) => Some(String::from_utf8_lossy(&plaintext).into_owned()),
        Err(e) => {
            warn!(id = %row.id, error = ?e, "skipping an item that failed to decrypt");
            None
        }
    }
}

/// One version of one item, arriving from another device.
///
/// Plaintext, because the caller has already opened whatever the transport
/// sealed it with.
pub struct RemoteVersion<'a> {
    pub item_id: &'a str,
    /// Empty for a tombstone.
    pub content: &'a str,
    pub content_type: &'a str,
    pub created_at: i64,
    pub deleted: bool,
    /// The sender's hash, when the transport carries one.
    ///
    /// The peer protocol does; the cloud row deliberately does **not** — a hash
    /// of the plaintext sitting next to the ciphertext would let the backend
    /// confirm a guess at clipboard content, which is the whole property the
    /// client-side encryption exists to hold. `None` is recomputed below from
    /// the content with the store's own hash function, so the two transports
    /// still compare the same value for the same bytes.
    pub content_hash: Option<&'a str>,
    /// The device that produced *this version*, preserved across hops.
    pub origin_device_id: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeError {
    Store,
    Encrypt,
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

/// A pin outranks another device's delete.
///
/// `Store::delete_all` — the local "clear history" gesture — deliberately spares
/// pinned rows, with `delete_all_leaves_pinned_items_intact` pinning that
/// behaviour. Without this check the two paths disagree: clearing history on one
/// device tombstones its unpinned rows, those tombstones travel, and every
/// *pinned* item on every paired device is destroyed by them. Rule 4 puts data
/// loss above everything, and a pin is the most explicit "keep this" a user can
/// give.
///
/// The cost, stated rather than hidden: the two devices then disagree about the
/// item — live and pinned here, gone there. That divergence is stable rather
/// than oscillating (the refusal never produces a *newer* version to push back)
/// and it is resolvable by an ordinary gesture: unpin, and the next round takes
/// the delete. A deleted item that cannot be brought back is not resolvable by
/// anything.
///
/// This is not the manifest's rule, and the difference is deliberate.
/// Manifest 05 §3.6 treats `pinned` as an ordinary LWW field that travels on the
/// wire, so a remote delete arriving with a later stamp legitimately carried the
/// writer's pin state with it. v2 does not sync pin state at all — it is local,
/// and `Store::upsert` keeps it that way — so a remote delete cannot have known
/// about this pin, and honouring it would be acting on information the sender
/// never had.
fn refuses_delete(local: &StoredItem, incoming: &RemoteVersion<'_>) -> bool {
    incoming.deleted && local.pinned && !local.deleted
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
    let local = store.version(incoming.item_id).map_err(|e| {
        warn!(error = ?e, "could not read the local version of an incoming item");
        MergeError::Store
    })?;

    // A tombstone with no hash of its own inherits the one it is deleting: the
    // store keeps `content_hash` on a tombstone deliberately, so inheriting it
    // makes the delete tie its own live version on keys 1 and 2 and win on key
    // 3 (`deleted`). Inventing a different hash there would decide the delete
    // on the wrong key.
    let computed;
    let content_hash: &str = match incoming.content_hash {
        Some(hash) => hash,
        None if incoming.deleted => local.as_ref().map_or("", |l| l.content_hash.as_str()),
        None => {
            computed = crate::storage::compute_content_hash(incoming.content.as_bytes());
            &computed
        }
    };

    let remote = ItemSummary {
        item_id: incoming.item_id.to_string(),
        created_at: incoming.created_at,
        deleted: incoming.deleted,
        content_hash: content_hash.to_string(),
    };

    if let Some(local) = &local {
        let mine = ItemSummary {
            item_id: local.id.clone(),
            created_at: local.created_at,
            deleted: local.deleted,
            content_hash: local.content_hash.clone(),
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
        if refuses_delete(local, incoming) {
            info!(
                id = %incoming.item_id,
                "not deleting a pinned item on another device's say-so; unpin it to allow that"
            );
            return Ok(false);
        }
    }

    let is_sensitive = if incoming.deleted {
        local.as_ref().is_some_and(|l| l.is_sensitive)
    } else {
        detector.is_sensitive(incoming.content)
    };

    let sealed = if incoming.deleted {
        None
    } else {
        let key = keyring.item_key();
        Some(
            crate::encrypt(incoming.content.as_bytes(), &key, incoming.item_id).map_err(|e| {
                warn!(error = ?e, "could not seal an incoming item");
                MergeError::Encrypt
            })?,
        )
    };
    let (nonce, ciphertext) = match &sealed {
        Some((nonce, ciphertext)) => (Some(nonce.as_slice()), Some(ciphertext.as_slice())),
        None => (None, None),
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
            search_text: if is_sensitive || incoming.deleted {
                None
            } else {
                Some(incoming.content)
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
    use crate::sync::testkit::{fixture, version, Fixture};

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

    /// The sync-path mirror of `delete_all_leaves_pinned_items_intact`: one
    /// device running `clear` must not destroy another device's pinned items.
    #[test]
    fn a_remote_delete_does_not_destroy_a_pinned_item() {
        let f = fixture();
        f.apply(&version("keeper", "worth keeping", 1_000));
        assert!(f.store.set_pinned("keeper", true).unwrap());

        let applied = f.apply(&RemoteVersion {
            content: "",
            deleted: true,
            // Later than the version it deletes, so the ordering alone would
            // take it: only the pin stops this.
            ..version("keeper", "", 9_000)
        });

        assert!(!applied, "a remote delete beat an explicit local keep");
        let row = f.store.get("keeper").unwrap().expect("still live");
        assert!(row.pinned, "the pin was cleared by a refused delete");

        // Unpinning is what allows it, and the delete is still there to be
        // taken on the next round rather than being lost.
        assert!(f.store.set_pinned("keeper", false).unwrap());
        assert!(f.apply(&RemoteVersion {
            content: "",
            deleted: true,
            ..version("keeper", "", 9_000)
        }));
        assert!(f.store.get("keeper").unwrap().is_none());
    }

    /// The refusal is about *deletes*: an ordinary newer version still lands,
    /// and the pin survives it (`Store::upsert` keeps pin state local).
    #[test]
    fn a_pin_does_not_block_an_ordinary_update() {
        let f = fixture();
        f.apply(&version("keeper", "first", 1_000));
        f.store.set_pinned("keeper", true).unwrap();

        assert!(f.apply(&version("keeper", "second", 9_000)));
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
            })
            .unwrap();
        assert_eq!(mine.origin_device_id, "", "a capture records no origin");

        assert!(!f.apply(&RemoteVersion {
            origin_device_id: &f.here,
            ..version("mine", "mine", 1_000)
        }));
    }

    #[test]
    fn error_messages_contain_no_paths() {
        for message in [MSG_STORE, MSG_ENCRYPT] {
            assert!(!message.contains('/'), "{message}");
        }
    }
}
