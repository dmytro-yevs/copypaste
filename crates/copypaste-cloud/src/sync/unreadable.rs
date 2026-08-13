//! Which local rows would not open for upload, and how they are revisited.
//!
//! INV-N7 has the floor pass an undecryptable row and retry it by id, because
//! holding the floor below it stalls every later row. That retry list needs a
//! bound, and the bound is the whole difficulty: past it, ids stop being
//! recorded while the floor keeps moving, and a row repaired later is never
//! offered again. So rows the list has no room for leave a durable [`Sweep`]
//! behind (INV-N7a) — a keyset cursor walking the region they were last seen
//! in, a page a round, wrapping when it reaches the floor.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tracing::warn;

/// A keyset position in the upload scan.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct UploadFloor {
    pub created_at: i64,
    pub item_id: Option<String>,
}

/// How many unreadable ids are retried by direct lookup each round. Keeps the
/// retry query within SQLite's parameter ceiling; rows past it are covered by
/// [`Sweep`], not dropped.
const MAX_UNREADABLE_TRACKED: usize = 256;

/// Where the re-walk of untracked unreadable rows stands.
///
/// `next` only moves forward until it reaches the upload floor, then wraps to
/// `from`. That is what bounds the cycle at `ceil(size / UPLOAD_SCAN_LIMIT)`
/// rounds; persisting it is what makes a restart resume rather than restart.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sweep {
    /// The oldest position a row was left behind at. Exclusive, as `next` is:
    /// the row itself is the first thing a walk from here reads.
    pub from: UploadFloor,
    pub next: UploadFloor,
    /// Rows of the cycle under way that the id list had no room for.
    pub seen: u32,
    /// What the last completed cycle counted.
    pub settled: u32,
}

impl Sweep {
    /// What the walk can still prove (INV-N7b). Only at a cycle boundary
    /// (`next == from`) is `settled` a statement about the region now: past it
    /// the region may have been repaired under the walk, so the cycle under way
    /// speaks for itself rather than carrying the last one's larger figure.
    fn proven(&self) -> u32 {
        if self.next == self.from {
            self.settled
        } else {
            self.seen
        }
    }
}

/// Local rows this device could not open for upload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnreadableUploads {
    /// Retried by direct lookup at the head of every scan (INV-N7).
    pub ids: BTreeSet<String>,
    /// The client-visible count, exact except while a [`Sweep`] cycle is still
    /// walking a backlog larger than `ids` (INV-N7b).
    pub total: u32,
    /// Absent when the unreadable rows all fit in `ids`, as they ordinarily do.
    pub sweep: Option<Sweep>,
    /// The persisted record was malformed, so its ids are lost and the caller
    /// must rescan from the beginning of history or strand them (B2). Derived
    /// at decode and never persisted: it describes one round's recovery.
    pub reset_floor: bool,
}

impl UnreadableUploads {
    /// Parse the persisted form. A missing record is "none seen"; a malformed
    /// one signals `reset_floor`, because the ids it held are gone and the
    /// floor has already moved past their rows.
    #[must_use]
    pub fn decode(raw: Option<&str>) -> Self {
        let Some(raw) = raw.filter(|value| !value.is_empty()) else {
            return Self::default();
        };
        serde_json::from_str::<Persisted>(raw).map_or_else(
            |_| {
                warn!("the unreadable-upload record could not be read; rescanning from the start");
                Self {
                    reset_floor: true,
                    ..Self::default()
                }
            },
            |persisted| Self {
                ids: persisted.ids.into_iter().collect(),
                total: persisted.total,
                sweep: persisted.sweep,
                reset_floor: false,
            },
        )
    }

    /// Ids and positions only — a row's content never reaches this.
    #[must_use]
    pub fn encode(&self) -> String {
        serde_json::to_string(&Persisted {
            ids: self.ids.iter().cloned().collect(),
            total: self.total,
            sweep: self.sweep.clone(),
        })
        .unwrap_or_default()
    }

    /// `false` when the bound is full, and then the caller owes the row a
    /// place in the sweep.
    pub(super) fn track(&mut self, id: &str) -> bool {
        if self.ids.len() >= MAX_UNREADABLE_TRACKED {
            return false;
        }
        self.ids.insert(id.to_owned());
        true
    }

    /// `behind` is the position *before* the row, so a walk resuming from
    /// there reads that row first.
    pub(super) fn sweep_from(&mut self, behind: &UploadFloor) {
        match &mut self.sweep {
            // The walk already reaches at least this far back.
            Some(sweep) if sweep.from <= *behind => {}
            Some(sweep) => {
                sweep.next.clone_from(behind);
                sweep.from.clone_from(behind);
            }
            None => {
                self.sweep = Some(Sweep {
                    from: behind.clone(),
                    next: behind.clone(),
                    seen: 0,
                    settled: 0,
                });
            }
        }
    }

    /// Which list owns a row the walk found unreadable. The id list has first
    /// claim: counting one it also holds reports the row twice (INV-N7b).
    pub(super) fn record_walked(&mut self, id: &str, sweep: &mut Sweep) {
        if !self.track(id) {
            sweep.seen = sweep.seen.saturating_add(1);
        }
    }

    /// What is retried by id, plus what the walk has proven (INV-N7b).
    pub(super) fn settle_total(&mut self) {
        let backlog = self.sweep.as_ref().map_or(0, Sweep::proven);
        self.total = u32::try_from(self.ids.len())
            .unwrap_or(u32::MAX)
            .saturating_add(backlog);
    }
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    ids: Vec<String>,
    total: u32,
    #[serde(default)]
    sweep: Option<Sweep>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(created_at: i64, item_id: &str) -> UploadFloor {
        UploadFloor {
            created_at,
            item_id: Some(item_id.to_owned()),
        }
    }

    #[test]
    fn a_record_round_trips_and_never_carries_content() {
        let mut record = UnreadableUploads::default();
        record.track("item-a");
        record.track("item-b");
        record.sweep_from(&at(500, "item-c"));
        record.settle_total();

        let encoded = record.encode();
        assert!(!encoded.contains('/'), "{encoded}");
        assert_eq!(UnreadableUploads::decode(Some(&encoded)), record);
    }

    #[test]
    fn a_missing_record_decodes_to_default() {
        assert_eq!(
            UnreadableUploads::decode(None),
            UnreadableUploads::default()
        );
        assert_eq!(
            UnreadableUploads::decode(Some("")),
            UnreadableUploads::default()
        );
    }

    /// B2. A record that will not parse has lost ids whose rows are already
    /// behind the floor, so the round must rescan from the beginning.
    #[test]
    fn a_malformed_record_asks_for_a_rescan_from_the_start() {
        let decoded = UnreadableUploads::decode(Some("not json"));
        assert!(decoded.reset_floor);
        assert!(decoded.ids.is_empty());
        assert_eq!(decoded.total, 0);
        assert!(UnreadableUploads::decode(Some("{\"ids\":")).reset_floor);
    }

    /// The signal describes one round's recovery. Persisting it would make
    /// every later round rescan the whole history for nothing.
    #[test]
    fn the_rescan_signal_is_never_persisted() {
        let mut record = UnreadableUploads::default();
        record.track("a");
        record.reset_floor = true;

        let restored = UnreadableUploads::decode(Some(&record.encode()));
        assert!(!restored.reset_floor);
        assert!(restored.ids.contains("a"));
    }

    /// B3. Past the bound `track` refuses, and refusing is what obliges the
    /// caller to leave a sweep behind rather than let the floor pass the row.
    #[test]
    fn the_retry_bound_refuses_rather_than_evicting() {
        let mut record = UnreadableUploads::default();
        for n in 0..MAX_UNREADABLE_TRACKED {
            assert!(record.track(&format!("item-{n:04}")));
        }
        assert!(!record.track("one-too-many"));
        assert_eq!(record.ids.len(), MAX_UNREADABLE_TRACKED);
        assert!(!record.ids.contains("one-too-many"));
    }

    /// The walk must reach back to the *oldest* row left behind; a later
    /// position would strand everything between the two.
    #[test]
    fn the_sweep_start_only_ever_moves_further_back() {
        let mut record = UnreadableUploads::default();
        record.sweep_from(&at(900, "later"));
        record.sweep_from(&at(100, "earlier"));
        assert_eq!(record.sweep.as_ref().unwrap().from, at(100, "earlier"));
        assert_eq!(record.sweep.as_ref().unwrap().next, at(100, "earlier"));

        record.sweep.as_mut().unwrap().next = at(400, "walked");
        record.sweep_from(&at(900, "later"));
        assert_eq!(
            record.sweep.as_ref().unwrap().next,
            at(400, "walked"),
            "a position already covered must not rewind the walk"
        );
    }

    /// INV-N7b. A row the id list has room for is not also counted by the
    /// cycle, so the two halves of the figure never hold the same row.
    #[test]
    fn a_walked_row_is_counted_once_by_whichever_list_owns_it() {
        let mut record = UnreadableUploads::default();
        let mut sweep = Sweep {
            from: at(0, "a"),
            next: at(10, "b"),
            ..Sweep::default()
        };

        record.record_walked("taken", &mut sweep);
        assert_eq!((record.ids.len(), sweep.seen), (1, 0));

        for n in 0..MAX_UNREADABLE_TRACKED {
            record.track(&format!("filler-{n:04}"));
        }
        record.record_walked("no-room", &mut sweep);
        assert_eq!(sweep.seen, 1);
        assert!(!record.ids.contains("no-room"));

        record.sweep = Some(sweep);
        record.settle_total();
        assert_eq!(record.total, MAX_UNREADABLE_TRACKED as u32 + 1);
    }

    /// INV-N7b. A completed cycle's figure is evidence about the region as it
    /// was, so it stands only until the next cycle reads its first page; from
    /// there the figure is what that cycle has verified, and it climbs back as
    /// the walk goes on. Reporting the older, larger figure through a repair
    /// is the over-count this replaced.
    #[test]
    fn a_walking_cycle_reports_what_it_has_verified_not_the_last_one() {
        let mut record = UnreadableUploads::default();
        record.track("tracked");
        record.sweep = Some(Sweep {
            from: at(0, "a"),
            next: at(0, "a"),
            seen: 0,
            settled: 40,
        });
        record.settle_total();
        assert_eq!(record.total, 41, "a completed cycle is exact");

        record.sweep.as_mut().unwrap().next = at(10, "b");
        record.settle_total();
        assert_eq!(
            record.total, 1,
            "a cycle that has proven nothing yet says so"
        );

        record.sweep.as_mut().unwrap().seen = 60;
        record.settle_total();
        assert_eq!(record.total, 61);
    }
}
