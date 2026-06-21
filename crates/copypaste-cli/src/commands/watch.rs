// # Why this is polling, not push — and how we minimise the cost
//
// ## The ideal (not yet possible)
// The ideal implementation would open a long-lived Unix socket connection and
// have the daemon push a JSON line each time a new item is captured (server-sent
// events over the existing socket transport). The daemon already has an internal
// broadcast channel (`new_item_tx`, `tokio::sync::broadcast`) that drives P2P
// and cloud-sync paths — a `"subscribe"` IPC verb would be a thin wrapper.
//
// However, **no such IPC method exists today**. The daemon's dispatch loop
// (copypaste-daemon/src/ipc.rs) handles only request/response methods; the
// broadcast channel is internal and never exposed over the socket. Implementing
// a streaming verb requires protocol changes in the daemon (`copypaste-daemon`)
// and possibly in the shared types (`copypaste-ipc`) — changes that are out of
// scope for the CLI crate.
//
// TODO(CopyPaste-44rq.19): replace polling with a push subscription once the
// daemon exposes a streaming/subscribe IPC verb.
//
// ## What we do instead — adaptive backoff
// Until the daemon exposes a push mechanism, polling is the only option.  To
// avoid spinning the socket on an idle clipboard, this implementation uses
// **adaptive exponential backoff**:
//
//   - After each poll that finds **no new items**, the wait interval doubles
//     up to `MAX_BACKOFF_MS` (default 8 000 ms — 8 s).
//   - After each poll that finds **at least one new item**, the interval resets
//     to `MIN_INTERVAL_MS` (100 ms) so rapid bursts still feel responsive.
//   - The user-supplied `--interval` argument sets the *initial / reset* interval
//     (clamped to `MIN_INTERVAL_MS`). A busy clipboard never waits longer than
//     that value; an idle clipboard backs off automatically.
//
// This eliminates constant-rate polling when the clipboard is idle while
// preserving low latency during active clipboard use, without any daemon changes.

use crate::commands::common::format_unix_ms;
use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::METHOD_LIST;
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Upper bound on the number of item ids `watch` remembers as "already
/// printed". `watch` runs indefinitely, so an unbounded set would grow without
/// limit on a busy clipboard. We only ever fetch the newest `LIST_LIMIT` items
/// per poll, so a cap an order of magnitude larger than that is more than
/// enough to suppress duplicate prints while bounding memory. Oldest ids are
/// evicted FIFO; re-seeing a long-evicted id at worst reprints it once.
const MAX_SEEN_IDS: usize = 4096;

/// How many recent items each poll requests from the daemon.
const LIST_LIMIT: u64 = 20;

/// Minimum polling interval (and the reset value after seeing a new item).
/// Values below this would spin the socket in a tight loop.
const MIN_INTERVAL_MS: u64 = 100;

/// Maximum interval the adaptive backoff will reach on an idle clipboard.
/// At this ceiling one poll happens every 8 s, which is negligible overhead
/// while still catching any new item within 8 s of capture.
const MAX_BACKOFF_MS: u64 = 8_000;

/// FIFO-bounded set of item ids. Membership test is O(1); when full, inserting
/// a new id evicts the oldest so memory stays capped over a long-running watch.
struct SeenIds {
    set: HashSet<String>,
    order: VecDeque<String>,
    cap: usize,
}

impl SeenIds {
    fn new(cap: usize) -> Self {
        Self {
            set: HashSet::new(),
            order: VecDeque::new(),
            cap,
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.set.contains(id)
    }

    /// Record `id` as seen, evicting the oldest entry if at capacity.
    fn insert(&mut self, id: &str) {
        if self.set.contains(id) {
            return;
        }
        if self.order.len() >= self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.set.remove(&oldest);
            }
        }
        self.set.insert(id.to_string());
        self.order.push_back(id.to_string());
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        debug_assert_eq!(self.set.len(), self.order.len());
        self.set.len()
    }
}

/// Clamp a requested polling interval up to `MIN_INTERVAL_MS` so `--interval 0`
/// (or very small values) can never busy-poll the socket.
fn clamp_interval(interval_ms: u64) -> u64 {
    interval_ms.max(MIN_INTERVAL_MS)
}

/// Compute the next backoff interval given the current one.
///
/// If `saw_new_item` is true the interval resets to `reset_ms` (the
/// user-supplied initial interval, already clamped). Otherwise it doubles,
/// capped at `MAX_BACKOFF_MS`.
fn next_interval(current_ms: u64, saw_new_item: bool, reset_ms: u64) -> u64 {
    if saw_new_item {
        reset_ms
    } else {
        (current_ms.saturating_mul(2)).min(MAX_BACKOFF_MS)
    }
}

pub fn run(socket_path: &Path, interval_ms: u64) -> Result<()> {
    // Enforce a floor so `--interval 0` (or very small values) don't create a
    // tight CPU/socket spin.
    let reset_ms = clamp_interval(interval_ms);

    let mut seen_ids = SeenIds::new(MAX_SEEN_IDS);
    let mut first_run = true;
    let mut current_interval_ms = reset_ms;

    eprintln!("watching clipboard (Ctrl+C to stop)...");

    loop {
        let saw_new = match poll_once(socket_path, &mut seen_ids, first_run) {
            Ok(new) => new,
            Err(e) => {
                eprintln!("watch: {e}");
                // On error (e.g. daemon not running) treat as "no new items"
                // so we back off rather than hammering a down daemon.
                false
            }
        };
        first_run = false;

        // Adaptive backoff: reset on activity, double on silence.
        current_interval_ms = next_interval(current_interval_ms, saw_new, reset_ms);
        thread::sleep(Duration::from_millis(current_interval_ms));
    }
}

/// Poll the daemon once. Returns `true` if at least one previously-unseen item
/// was found (and printed), `false` if the clipboard was quiet.
fn poll_once(socket_path: &Path, seen_ids: &mut SeenIds, silent_first: bool) -> Result<bool> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_LIST,
        serde_json::json!({"limit": LIST_LIMIT, "offset": 0}),
    );
    let resp = client.call(&req)?;

    if !resp.ok {
        return Err(anyhow::anyhow!("{}", resp.error.unwrap_or_default()));
    }

    let items = resp
        .data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let mut saw_new = false;
    for item in items {
        let id = item["id"].as_str().unwrap_or("?");
        if seen_ids.contains(id) {
            continue;
        }
        seen_ids.insert(id);
        if silent_first {
            // Populate seen on first run; don't print pre-existing items.
            continue;
        }
        saw_new = true;
        let content_type = item["content_type"].as_str().unwrap_or("?");
        let wall_time = item["wall_time"].as_i64().unwrap_or(0);
        let sensitive = item["is_sensitive"].as_bool().unwrap_or(false);
        let sens = if sensitive { " [sensitive]" } else { "" };
        let ts = format_unix_ms(wall_time);
        println!("+ {ts}  {content_type:<6}  {id}{sens}");
    }
    Ok(saw_new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, u64) -> Result<()> = run;
    }

    #[test]
    fn seen_ids_is_bounded_and_evicts_fifo() {
        let mut seen = SeenIds::new(3);
        seen.insert("a");
        seen.insert("b");
        seen.insert("c");
        assert_eq!(seen.len(), 3);
        assert!(seen.contains("a"));

        // Inserting a 4th evicts the oldest ("a"), not the newer ones.
        seen.insert("d");
        assert_eq!(seen.len(), 3, "set must stay capped at 3");
        assert!(!seen.contains("a"), "oldest id should be evicted");
        assert!(seen.contains("b"));
        assert!(seen.contains("c"));
        assert!(seen.contains("d"));
    }

    #[test]
    fn seen_ids_insert_is_idempotent() {
        let mut seen = SeenIds::new(4);
        seen.insert("x");
        seen.insert("x");
        seen.insert("x");
        assert_eq!(seen.len(), 1, "re-inserting must not grow the set");
        assert!(seen.contains("x"));
    }

    #[test]
    fn seen_ids_never_exceeds_cap_under_churn() {
        let mut seen = SeenIds::new(MAX_SEEN_IDS);
        for i in 0..(MAX_SEEN_IDS * 3) {
            seen.insert(&format!("id-{i}"));
            assert!(seen.len() <= MAX_SEEN_IDS);
        }
        assert_eq!(seen.len(), MAX_SEEN_IDS);
    }

    /// `--interval 0` and values below MIN_INTERVAL_MS must be clamped up so
    /// `run` never busy-polls the socket. We test the clamping logic directly
    /// via the constant rather than calling `run` (which loops forever).
    #[test]
    fn interval_zero_is_clamped_to_minimum() {
        assert_eq!(clamp_interval(0), MIN_INTERVAL_MS);
        assert_eq!(clamp_interval(50), MIN_INTERVAL_MS);
        assert_eq!(clamp_interval(MIN_INTERVAL_MS), MIN_INTERVAL_MS);
        // Values above the floor are unchanged.
        assert_eq!(clamp_interval(500), 500);
        assert_eq!(clamp_interval(1000), 1000);
    }

    /// When a new item is seen, the interval resets to the initial (reset) value
    /// regardless of how backed-off the current interval is.
    #[test]
    fn adaptive_backoff_resets_on_new_item() {
        let reset_ms = 200_u64;
        // Start backed off to some large value.
        let backed_off = 4_000_u64;
        let next = next_interval(backed_off, true, reset_ms);
        assert_eq!(
            next, reset_ms,
            "interval must reset to {reset_ms} when a new item is seen"
        );
    }

    /// When no new items are seen, the interval doubles each step until it hits
    /// MAX_BACKOFF_MS, then stays there.
    #[test]
    fn adaptive_backoff_doubles_on_idle_and_caps() {
        let reset_ms = 200_u64;
        let mut interval = reset_ms;
        // Double until we reach the cap.
        let steps_to_cap = 6; // 200 -> 400 -> 800 -> 1600 -> 3200 -> 6400 -> 8000 (capped)
        for _ in 0..steps_to_cap {
            interval = next_interval(interval, false, reset_ms);
        }
        assert!(
            interval <= MAX_BACKOFF_MS,
            "interval must not exceed MAX_BACKOFF_MS ({MAX_BACKOFF_MS}), got {interval}"
        );
        // One more step must stay at the cap.
        let still_capped = next_interval(interval, false, reset_ms);
        assert_eq!(
            still_capped, MAX_BACKOFF_MS,
            "interval must stay at cap once reached, got {still_capped}"
        );
    }

    /// Verifies the interplay between backoff and reset: a new item mid-sequence
    /// immediately snaps the interval back to reset_ms.
    #[test]
    fn adaptive_backoff_reset_after_idle_sequence() {
        let reset_ms = 500_u64;
        // Three idle polls back off the interval.
        let after_3_idle = {
            let i = next_interval(reset_ms, false, reset_ms); // 1000
            let i = next_interval(i, false, reset_ms); // 2000
            next_interval(i, false, reset_ms) // 4000
        };
        assert_eq!(after_3_idle, 4_000, "sanity: 3 doublings of 500ms = 4000ms");

        // A new item snaps back to reset_ms.
        let after_reset = next_interval(after_3_idle, true, reset_ms);
        assert_eq!(
            after_reset, reset_ms,
            "interval must snap back to reset_ms after seeing a new item"
        );
    }
}
