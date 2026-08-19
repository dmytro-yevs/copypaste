//! [`apply_remote_version`] for a whole page.
//!
//! Two properties the row-at-a-time path gets for free and this one builds:
//!
//! * **The decision and the write see the same history.** Deciding from a read
//!   that a local delete or a newer peer version can land behind would let an
//!   unconditional UPSERT resurrect it, so the write transaction re-reads the
//!   summaries and refuses a snapshot that has moved (B6).
//! * **The answer does not depend on how the page was cut.** A simulated winner
//!   the dedup index then refuses never becomes a row, so anything that lost to
//!   it lost to nothing; those are re-applied one at a time afterwards (B7).

use std::collections::HashMap;

use tracing::warn;

use copypaste_p2p::protocol::ItemSummary;
use copypaste_p2p::sync::pin_state_wins;

use super::merge::{
    apply_remote_p2p_version_with_pin_stamp, apply_remote_version, payload_is_refused,
    stored_summary, MergeError, P2pApply, RemoteVersion,
};
use super::prepare::{prepare_remote_version, Prepared};
use crate::sensitive::Detector;
use crate::storage::{IncomingItem, MergePageError, Store, Version};
use crate::Keyring;

#[derive(Debug, Clone, Copy)]
pub struct P2pPin {
    pub pinned: bool,
    pub pin_order: Option<f64>,
    pub pin_updated_at: i64,
}

/// How many times a moved snapshot is re-prepared before the page gives up.
///
/// One retry, not a loop: a second race under the write lock means the history
/// is being written continuously, and failing the round is safe where spinning
/// against a live writer is not.
const SNAPSHOT_ATTEMPTS: usize = 2;

/// [`apply_remote_version`] for a whole page, answering positionally.
///
/// The same comparator, seal and refusals; what changes is how often the store
/// is touched. A cloud page of 500 rows went through 1,000 pooled connection
/// checkouts and up to 500 IMMEDIATE write transactions when it was driven a
/// row at a time, on a pool four deep, so it spent most of its wall clock
/// queueing behind itself. Now: one read, then one write transaction.
///
/// # Errors
///
/// [`MergeError::Store`] or [`MergeError::Encrypt`]. A version this device
/// declines is `false`, never an error.
pub fn apply_remote_versions(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
) -> Result<Vec<bool>, MergeError> {
    Ok(apply_page(store, keyring, detector, here, incoming, None)?
        .into_iter()
        .map(ApplyFlags::any)
        .collect())
}

pub fn apply_remote_p2p_versions(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
    pins: &[P2pPin],
) -> Result<Vec<P2pApply>, MergeError> {
    debug_assert_eq!(incoming.len(), pins.len());
    let applied = apply_page(store, keyring, detector, here, incoming, Some(pins))?;
    Ok(applied
        .into_iter()
        .map(|flags| P2pApply {
            content: flags.content,
            pin: flags.pin,
        })
        .collect())
}

#[derive(Clone, Copy)]
struct ApplyFlags {
    content: bool,
    pin: bool,
}

impl ApplyFlags {
    fn any(self) -> bool {
        self.content || self.pin
    }
}

fn apply_page(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
    pins: Option<&[P2pPin]>,
) -> Result<Vec<ApplyFlags>, MergeError> {
    if incoming.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<&str> = incoming.iter().map(|item| item.item_id).collect();
    let mut snapshot = store.version_summaries(&ids).map_err(|e| {
        warn!(error = ?e, "could not read the local versions of an incoming page");
        MergeError::Store
    })?;

    for _ in 0..SNAPSHOT_ATTEMPTS {
        let (prepared, slots, pin_only) =
            prepare(keyring, detector, here, incoming, pins, &snapshot)?;
        let writes: Vec<IncomingItem<'_>> = prepared.iter().map(Prepared::as_incoming).collect();
        match store.merge_page(&ids, &snapshot, &writes) {
            Ok(written) => {
                if written.len() != prepared.len() {
                    warn!("the database did not answer for every row of a merged page");
                    return Err(MergeError::Store);
                }
                let mut applied: Vec<ApplyFlags> = slots
                    .iter()
                    .map(|slot| {
                        let content = slot.is_some_and(|index| written[index]);
                        ApplyFlags {
                            content,
                            pin: content && pins.is_some(),
                        }
                    })
                    .collect();
                for index in pin_only {
                    if applied[index].any() {
                        continue;
                    }
                    let pin = pins.expect("pin-only rows require P2P pins")[index];
                    applied[index].pin = store
                        .apply_pin_state(
                            incoming[index].item_id,
                            pin.pinned,
                            pin.pin_order,
                            pin.pin_updated_at,
                        )
                        .map_err(|e| {
                            warn!(error = ?e, "could not store incoming P2P pin state");
                            MergeError::Store
                        })?;
                }
                redo_after_refusals(
                    store,
                    keyring,
                    detector,
                    here,
                    incoming,
                    pins,
                    &slots,
                    &written,
                    &mut applied,
                )?;
                return Ok(applied);
            }
            Err(MergePageError::SnapshotChanged(authoritative)) => {
                warn!("a concurrent write changed the merge snapshot; re-preparing");
                snapshot = authoritative;
            }
            Err(MergePageError::Store(e)) => {
                warn!(error = ?e, "could not merge an incoming page");
                return Err(MergeError::Store);
            }
        }
    }

    warn!("the merge snapshot changed on every attempt");
    Err(MergeError::Store)
}

/// Decide the whole page against `local`, threading each id's own decisions
/// forward. `slots[i]` is `None` for a version that lost outright, otherwise
/// its index in the returned writes.
fn prepare(
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
    pins: Option<&[P2pPin]>,
    local: &HashMap<String, Version>,
) -> Result<(Vec<Prepared>, Vec<Option<usize>>, Vec<usize>), MergeError> {
    let mut local = local.clone();
    let mut prepared: Vec<Prepared> = Vec::new();
    let mut slots: Vec<Option<usize>> = Vec::with_capacity(incoming.len());
    let mut pin_only: Vec<usize> = Vec::new();
    for (index, item) in incoming.iter().enumerate() {
        if payload_is_refused(item) {
            slots.push(None);
            continue;
        }
        let pin_state = pins.map(|pins| {
            let pin = pins[index];
            let remote_pin_wins = !item.deleted
                && local.get(item.item_id).is_none_or(|row| {
                    pin_state_wins(
                        &stored_summary(row, here),
                        &ItemSummary {
                            item_id: item.item_id.to_string(),
                            created_at: item.created_at,
                            deleted: item.deleted,
                            content_hash: String::new(),
                            origin_device_id: item.origin_device_id.to_string(),
                            pinned: pin.pinned,
                            pin_order: pin.pin_order,
                            pin_updated_at: pin.pin_updated_at,
                        },
                    )
                });
            (
                pin.pinned,
                pin.pin_order,
                pin.pin_updated_at,
                remote_pin_wins,
            )
        });
        match prepare_remote_version(
            keyring,
            detector,
            here,
            item,
            pin_state,
            local.get(item.item_id),
        )? {
            Some(ready) => {
                local.insert(item.item_id.to_string(), ready.as_version(here));
                slots.push(Some(prepared.len()));
                prepared.push(ready);
            }
            None => {
                if pin_state.is_some_and(|state| state.3) {
                    pin_only.push(index);
                }
                slots.push(None);
            }
        }
    }
    Ok((prepared, slots, pin_only))
}

/// Re-apply the versions that were decided against a winner the store then
/// refused (B7).
///
/// The dedup index refuses a row whose content already belongs to a different
/// id from the same origin in the same bucket. That refusal is per row, so the
/// page keeps its other writes — but a later version of the *refused* id was
/// compared against a winner that never became a row, and on its own page it
/// might well have been admitted. Only the versions that came out `false` are
/// redone: a `true` is a row that actually landed, and re-offering it would
/// have it decline against itself.
fn redo_after_refusals(
    store: &Store,
    keyring: &Keyring,
    detector: &Detector,
    here: &str,
    incoming: &[RemoteVersion<'_>],
    pins: Option<&[P2pPin]>,
    slots: &[Option<usize>],
    written: &[bool],
    applied: &mut [ApplyFlags],
) -> Result<(), MergeError> {
    let mut refused_at: HashMap<&str, usize> = HashMap::new();
    for (index, slot) in slots.iter().enumerate() {
        if slot.is_some_and(|slot| !written[slot]) {
            refused_at.entry(incoming[index].item_id).or_insert(index);
        }
    }
    if refused_at.is_empty() {
        return Ok(());
    }
    for (index, item) in incoming.iter().enumerate() {
        if applied[index].any()
            || refused_at
                .get(item.item_id)
                .is_none_or(|first| index <= *first)
        {
            continue;
        }
        applied[index] = match pins {
            Some(pins) => {
                let pin = pins[index];
                let outcome = apply_remote_p2p_version_with_pin_stamp(
                    store,
                    keyring,
                    detector,
                    here,
                    item,
                    pin.pinned,
                    pin.pin_order,
                    pin.pin_updated_at,
                )?;
                ApplyFlags {
                    content: outcome.content,
                    pin: outcome.pin,
                }
            }
            None => ApplyFlags {
                content: apply_remote_version(store, keyring, detector, here, item)?,
                pin: false,
            },
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{fixture_named, version, Fixture};
    use super::*;

    fn apply_page(f: &Fixture, page: &[RemoteVersion<'_>]) -> Vec<bool> {
        apply_remote_versions(&f.store, &f.keyring, &f.detector, &f.here, page).expect("merge")
    }

    /// Occupy the dedup key `(content_hash, created_at / 60000, origin)` under
    /// a different id, which is what makes the store refuse a later row with
    /// the same content in the same minute from the same device.
    fn occupy(f: &Fixture) {
        assert!(apply_page(f, &[version("occupant", "collides", 5_000)])[0]);
    }

    /// B7, reproduced. The first version of `target` wins the comparator and is
    /// then refused by the dedup index, so it never becomes a row — but the
    /// second version was declined against it. Sending the two on separate
    /// pages admits the second, and one page has to answer the same way or the
    /// history a device ends up with depends on how the transport cut its
    /// pages.
    #[test]
    fn a_version_declined_against_a_refused_winner_is_re_applied() {
        let batched = fixture_named("batched");
        occupy(&batched);
        let answers = apply_page(
            &batched,
            &[
                version("target", "collides", 9_000),
                version("target", "admissible", 8_000),
            ],
        );

        assert_eq!(
            answers,
            vec![false, true],
            "the refused winner must not decide for the version behind it"
        );
        let stored = batched
            .store
            .version("target")
            .expect("read")
            .expect("the admissible version was lost to a row that never existed");
        assert_eq!(stored.created_at, 8_000);
    }

    /// The same versions, one per page. This is the baseline the batched answer
    /// has to match; asserting it here keeps the comparison honest if the
    /// comparator itself ever changes.
    #[test]
    fn one_page_and_two_pages_reach_the_same_history() {
        let batched = fixture_named("same");
        occupy(&batched);
        let together = apply_page(
            &batched,
            &[
                version("target", "collides", 9_000),
                version("target", "admissible", 8_000),
            ],
        );

        let apart = fixture_named("same");
        occupy(&apart);
        let mut separately = apply_page(&apart, &[version("target", "collides", 9_000)]);
        separately.extend(apply_page(
            &apart,
            &[version("target", "admissible", 8_000)],
        ));

        assert_eq!(together, separately);
        assert_eq!(
            batched
                .store
                .version("target")
                .expect("read")
                .map(|row| row.created_at),
            apart
                .store
                .version("target")
                .expect("read")
                .map(|row| row.created_at)
        );
    }

    /// A page with no refusal must not pay for the repair path, and a version
    /// that legitimately lost to one that *did* land must stay lost.
    #[test]
    fn a_page_that_was_not_refused_is_answered_by_the_batch_alone() {
        let f = fixture_named("clean");
        let answers = apply_page(
            &f,
            &[
                version("item", "older", 1_000),
                version("item", "newer", 2_000),
                version("other", "unrelated", 3_000),
            ],
        );
        assert_eq!(answers, vec![true, true, true]);
        assert_eq!(
            f.store.version("item").expect("read").unwrap().created_at,
            2_000
        );
    }
}
