//! Deciding one incoming version and sealing the winner.
//!
//! Split from [`super::merge`] by what each half owns. This one is pure: the
//! comparator, the AEAD and the shape of the row that will be written, with no
//! store in sight. `merge` owns the store — reading what is there now, and
//! writing what this returns. They move for different reasons: this changes
//! when the row shape or the seal changes, `merge` when the write does.

use copypaste_p2p::sync::{merge_decision, MergeDecision};
use tracing::{debug, warn};

use super::merge::{remote_summary, stored_summary, MergeError, RemoteVersion};
use crate::sensitive::Detector;
use crate::storage::{origin_or, IncomingItem, Version};
use crate::Keyring;

/// One version the merge has decided to take, sealed and ready to write.
///
/// Owned rather than borrowed so a page of them can be prepared before the
/// write transaction opens: the seal is CPU and the write is a lock, and
/// interleaving them is what made a 500-row page take 500 write locks.
pub(super) struct Prepared {
    id: String,
    content_ciphertext: Option<Vec<u8>>,
    nonce: Option<Vec<u8>>,
    content_type: String,
    content_hash: String,
    created_at: i64,
    deleted: bool,
    is_sensitive: bool,
    origin_device_id: String,
    app_bundle_id: Option<String>,
    app_name: Option<String>,
    pinned: bool,
    pin_order: Option<f64>,
    pin_updated_at: i64,
    search_text: Option<String>,
    payload_metadata: Option<String>,
}

impl Prepared {
    /// This decision as the comparator would read it back, for a second
    /// version of the same id later in the same page.
    pub(super) fn as_version(&self, here: &str) -> Version {
        Version {
            id: self.id.clone(),
            created_at: self.created_at,
            content_hash: self.content_hash.clone(),
            deleted: self.deleted,
            pinned: self.pinned,
            pin_order: self.pin_order,
            pin_updated_at: self.pin_updated_at,
            origin_device_id: origin_or(&self.origin_device_id, here).to_string(),
            is_sensitive: self.is_sensitive,
        }
    }

    pub(super) fn as_incoming(&self) -> IncomingItem<'_> {
        IncomingItem {
            id: &self.id,
            content_ciphertext: self.content_ciphertext.as_deref(),
            nonce: self.nonce.as_deref(),
            content_type: &self.content_type,
            content_hash: &self.content_hash,
            created_at: self.created_at,
            deleted: self.deleted,
            is_sensitive: self.is_sensitive,
            origin_device_id: &self.origin_device_id,
            app_bundle_id: self.app_bundle_id.as_deref(),
            app_name: self.app_name.as_deref(),
            pinned: self.pinned,
            pin_order: self.pin_order,
            pin_updated_at: self.pin_updated_at,
            search_text: self.search_text.as_deref(),
            payload_metadata: self.payload_metadata.as_deref(),
        }
    }
}

/// Run the comparator and seal the winner. `None` means the local copy won and
/// nothing is to be written (INV-I1).
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_remote_version(
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &RemoteVersion<'_>,
    pin_state: Option<(bool, Option<f64>, i64, bool)>,
    local: Option<&Version>,
) -> Result<Option<Prepared>, MergeError> {
    let content = incoming
        .binary_content
        .unwrap_or(incoming.content.as_bytes());
    if !incoming.deleted && content.len() > copypaste_ipc::MAX_CONTENT_BYTES {
        return Err(MergeError::TooLarge);
    }
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

    let remote = remote_summary(
        incoming,
        content_hash.to_string(),
        pin_state.map(|state| (state.0, state.1, state.2)),
    );

    if let Some(local) = local {
        let mine = stored_summary(local, here);
        if merge_decision(
            &mine,
            origin_or(&local.origin_device_id, here),
            &remote,
            incoming.origin_device_id,
        ) == MergeDecision::KeepLocal
        {
            debug!(id = %incoming.item_id, "local version wins; not applying");
            return Ok(None);
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
    let (nonce, ciphertext) = match sealed {
        Some((nonce, ciphertext)) => (Some(nonce), Some(ciphertext)),
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

    Ok(Some(Prepared {
        id: incoming.item_id.to_string(),
        content_ciphertext: ciphertext,
        nonce,
        content_type: incoming.content_type.to_string(),
        content_hash: content_hash.to_string(),
        created_at: incoming.created_at,
        deleted: incoming.deleted,
        is_sensitive,
        origin_device_id: incoming.origin_device_id.to_string(),
        app_bundle_id: incoming.app_bundle_id.map(str::to_string),
        app_name: incoming.app_name.map(str::to_string),
        pinned,
        pin_order,
        pin_updated_at,
        search_text: if is_sensitive
            || incoming.deleted
            || copypaste_ipc::content_type::is_binary(incoming.content_type)
        {
            None
        } else {
            Some(incoming.content.to_string())
        },
        payload_metadata: if incoming.deleted {
            None
        } else {
            incoming.payload_metadata.map(str::to_string)
        },
    }))
}

#[cfg(test)]
mod limit_mirror_tests {
    const ANDROID_CLIP_QUEUE: &str = include_str!(
        "../../../copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/ClipQueue.kt"
    );
    const DECLARATION: &str = "const val MAX_TEXT_BYTES = ";

    fn kotlin_max_text_bytes(source: &str) -> Option<usize> {
        let mut candidates = source.lines().map(str::trim).filter(|line| {
            line.contains("MAX_TEXT_BYTES") && line.split_whitespace().any(|word| word == "const")
        });
        let expression = candidates.next()?.strip_prefix(DECLARATION)?;
        let value = parse_positive_product(expression)?;

        candidates.next().is_none().then_some(value)
    }

    fn parse_positive_product(expression: &str) -> Option<usize> {
        let mut factors = expression.split('*').map(str::trim);
        let first = parse_positive_integer(factors.next()?)?;
        let mut value = first;
        let mut has_product = false;

        for factor in factors {
            has_product = true;
            value = value.checked_mul(parse_positive_integer(factor)?)?;
        }

        has_product.then_some(value)
    }

    fn parse_positive_integer(value: &str) -> Option<usize> {
        (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| value.parse().ok())
            .flatten()
            .filter(|value| *value > 0)
    }

    #[test]
    fn content_limits_match_across_ipc_p2p_and_android_capture() {
        assert_eq!(
            copypaste_ipc::MAX_CONTENT_BYTES,
            copypaste_p2p::protocol::MAX_CONTENT_BYTES
        );
        assert_eq!(
            kotlin_max_text_bytes(ANDROID_CLIP_QUEUE),
            Some(copypaste_ipc::MAX_CONTENT_BYTES)
        );
    }

    #[test]
    fn kotlin_limit_guard_rejects_unknown_or_ambiguous_source() {
        for source in [
            "",
            "const val MAX_TEXT_BYTES = 4 * 1024 * 1024\nconst val MAX_TEXT_BYTES = 4 * 1024 * 1024",
            "const val MAX_TEXT_BYTES = 4 * 1024 * 1024\nconst val MAX_TEXT_BYTES=4 * 1024 * 1024",
            "const val MAX_TEXT_BYTES = 4 * 1024 * 1024\nconst\tval MAX_TEXT_BYTES = 4 * 1024 * 1024",
            "const val MAX_TEXT_BYTES = 4194304",
            "const val MAX_TEXT_BYTES = 4 * 1024 * nope",
            "const val MAX_TEXT_BYTES = 18446744073709551615 * 2",
            "const val MAX_TEXT_BYTES = 0 * 1024 * 1024",
        ] {
            assert_eq!(kotlin_max_text_bytes(source), None, "{source}");
        }
    }

    #[test]
    fn kotlin_limit_guard_detects_a_changed_value() {
        let changed = "const val MAX_TEXT_BYTES = 3 * 1024 * 1024";

        assert_ne!(
            kotlin_max_text_bytes(changed),
            Some(copypaste_ipc::MAX_CONTENT_BYTES)
        );
    }
}
