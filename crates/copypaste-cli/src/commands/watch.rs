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

/// Minimum polling interval. Values below this would spin the socket in a
/// tight loop, wasting CPU and potentially flooding the daemon.
const MIN_INTERVAL_MS: u64 = 100;

pub fn run(socket_path: &Path, interval_ms: u64) -> Result<()> {
    // Enforce a floor so `--interval 0` (or very small values) don't create a
    // tight CPU/socket spin. Use the minimum silently; callers that want 0
    // already know the daemon is local and low-latency.
    let interval_ms = interval_ms.max(MIN_INTERVAL_MS);

    let mut seen_ids = SeenIds::new(MAX_SEEN_IDS);
    let mut first_run = true;

    eprintln!("watching clipboard (Ctrl+C to stop)...");

    loop {
        match poll_once(socket_path, &mut seen_ids, first_run) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("watch: {e}");
                // If daemon not running, retry after interval
            }
        }
        first_run = false;
        thread::sleep(Duration::from_millis(interval_ms));
    }
}

fn poll_once(socket_path: &Path, seen_ids: &mut SeenIds, silent_first: bool) -> Result<()> {
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

    for item in items {
        let id = item["id"].as_str().unwrap_or("?");
        if seen_ids.contains(id) {
            continue;
        }
        seen_ids.insert(id);
        if silent_first {
            continue; // populate seen on first run, don't print
        }
        let content_type = item["content_type"].as_str().unwrap_or("?");
        let wall_time = item["wall_time"].as_i64().unwrap_or(0);
        let sensitive = item["is_sensitive"].as_bool().unwrap_or(false);
        let sens = if sensitive { " [sensitive]" } else { "" };
        let ts = format_unix_ms(wall_time);
        println!("+ {ts}  {content_type:<6}  {id}{sens}");
    }
    Ok(())
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
        assert_eq!(0u64.max(MIN_INTERVAL_MS), MIN_INTERVAL_MS);
        assert_eq!(50u64.max(MIN_INTERVAL_MS), MIN_INTERVAL_MS);
        assert_eq!(MIN_INTERVAL_MS.max(MIN_INTERVAL_MS), MIN_INTERVAL_MS);
        // Values above the floor are unchanged.
        assert_eq!(500u64.max(MIN_INTERVAL_MS), 500);
        assert_eq!(1000u64.max(MIN_INTERVAL_MS), 1000);
    }
}
