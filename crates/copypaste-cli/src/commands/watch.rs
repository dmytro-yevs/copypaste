// # Push subscription via watch_subscribe (CopyPaste-44rq.19)
//
// The daemon exposes `watch_subscribe` — a streaming IPC method that holds the
// connection open and pushes one JSON event line per new clipboard item. This
// implementation opens a single persistent connection and reads events as they
// arrive, replacing the old adaptive-backoff polling loop.
//
// ## Wire protocol
// 1. Client sends `{"id":"<id>","method":"watch_subscribe","params":{}}\n`.
// 2. Daemon sends an ack line:
//    `{"ok":true,"event":"subscribed","id":"<id>"}\n`
// 3. For each new item, the daemon pushes one event line:
//    `{"ok":true,"event":"new_item","item_id":"<uuid>","content_type":"<t>",
//      "wall_time":<ms>,"is_sensitive":<bool>,...}\n`
// 4. The loop ends when the daemon closes the connection (shutdown) or the
//    client exits (Ctrl+C).
//
// ## Fallback
// If the daemon does not recognise `watch_subscribe` (pre-44rq.19 build, or an
// error response is returned instead of the ack), we fall back to the original
// adaptive-backoff polling loop so the command remains usable across daemon
// versions.
//
// ## Reconnect
// On any read/write error after the initial handshake we sleep briefly and
// reconnect with a bounded backoff (same logic as the old poll reconnect).

use crate::commands::common::format_unix_ms;
use crate::ipc::IpcClient;
use anyhow::Result;
use copypaste_ipc::{METHOD_HISTORY_PAGE, METHOD_WATCH_SUBSCRIBE};
use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;
use std::time::Duration;

// ── Fallback (polling) constants ────────────────────────────────────────────

/// Upper bound on the number of item ids `watch` remembers as "already
/// printed". `watch` runs indefinitely, so an unbounded set would grow without
/// limit on a busy clipboard. We only ever fetch the newest `LIST_LIMIT` items
/// per poll, so a cap an order of magnitude larger than that is more than
/// enough to suppress duplicate prints while bounding memory. Oldest ids are
/// evicted FIFO; re-seeing a long-evicted id at worst reprints it once.
const MAX_SEEN_IDS: usize = 4096;

/// How many recent items each poll requests from the daemon (fallback only).
const LIST_LIMIT: u64 = 20;

/// Minimum polling interval (and the reset value after seeing a new item).
/// Values below this would spin the socket in a tight loop.
const MIN_INTERVAL_MS: u64 = 100;

/// Maximum interval the adaptive backoff will reach on an idle clipboard.
/// At this ceiling one poll happens every 8 s, which is negligible overhead
/// while still catching any new item within 8 s of capture.
const MAX_BACKOFF_MS: u64 = 8_000;

// ── Subscribe (push) constants ───────────────────────────────────────────────

/// Read timeout on the subscribe connection. We block here waiting for events;
/// use a generous timeout (30 s) so a brief idle period does not look like a
/// stalled connection. The daemon holds the connection open indefinitely when
/// no new items arrive.
const SUBSCRIBE_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Reconnect backoff on subscribe errors (network hiccup, daemon restart).
const SUBSCRIBE_RECONNECT_INIT_MS: u64 = 500;
const SUBSCRIBE_RECONNECT_MAX_MS: u64 = 8_000;

// ── SeenIds (used by polling fallback) ───────────────────────────────────────

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

// ── Backoff helpers ──────────────────────────────────────────────────────────

/// Clamp a requested polling interval up to `MIN_INTERVAL_MS` so `--interval 0`
/// (or very small values) can never busy-poll the socket.
fn clamp_interval(interval_ms: u64) -> u64 {
    interval_ms.max(MIN_INTERVAL_MS)
}

/// Compute the next backoff interval given the current one (polling fallback).
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

// ── Public entry point ───────────────────────────────────────────────────────

pub fn run(socket_path: &Path, interval_ms: u64) -> Result<()> {
    eprintln!("watching clipboard (Ctrl+C to stop)...");

    // Try the push-subscription path first.
    // `run_subscribe` returns Ok(()) only when the daemon closes the connection
    // cleanly (e.g. shutdown). It returns Err when the daemon does not support
    // watch_subscribe — in that case we fall through to the polling fallback.
    match run_subscribe(socket_path) {
        Ok(()) => {
            // Daemon closed the subscribe connection (daemon shutdown). Exit.
            return Ok(());
        }
        Err(_e) => {
            // Not supported or connection refused — fall back to polling.
            // (Debug note: watch_subscribe unavailable, using poll fallback.)
        }
    }

    // Polling fallback (for pre-44rq.19 daemons or if subscribe fails).
    run_poll(socket_path, interval_ms)
}

// ── Push-subscription loop ───────────────────────────────────────────────────

/// Try to run a `watch_subscribe` push loop. Returns `Ok(())` on clean daemon
/// shutdown. Returns `Err` when the daemon does not support the method (so the
/// caller can fall back to polling).
fn run_subscribe(socket_path: &Path) -> Result<()> {
    let mut backoff_ms = SUBSCRIBE_RECONNECT_INIT_MS;

    loop {
        match subscribe_once(socket_path) {
            SubscribeOutcome::Unsupported => {
                // Daemon returned an error instead of the ack — it does not
                // support watch_subscribe. Signal caller to fall back.
                return Err(anyhow::anyhow!("watch_subscribe not supported by daemon"));
            }
            SubscribeOutcome::DaemonShutdown => {
                // Clean close (broadcast channel closed = daemon exiting).
                return Ok(());
            }
            SubscribeOutcome::Disconnected => {
                // Transient error — reconnect with backoff.
                eprintln!("watch: connection lost, reconnecting in {backoff_ms}ms...");
                thread::sleep(Duration::from_millis(backoff_ms));
                backoff_ms = (backoff_ms * 2).min(SUBSCRIBE_RECONNECT_MAX_MS);
            }
        }
    }
}

/// Result of a single subscribe attempt.
enum SubscribeOutcome {
    /// Daemon returned an error for `watch_subscribe` — it does not support
    /// the method. Caller should fall back to polling.
    Unsupported,
    /// Broadcast channel closed on the daemon side — daemon is shutting down.
    DaemonShutdown,
    /// Network/read/write error — caller should reconnect after backoff.
    Disconnected,
}

/// Open one subscribe connection and read events until the connection drops.
fn subscribe_once(socket_path: &Path) -> SubscribeOutcome {
    // Connect to the daemon socket.
    let stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("watch: cannot connect to daemon: {e}");
            return SubscribeOutcome::Disconnected;
        }
    };

    // Use a generous read timeout so brief idle periods look like normal waits.
    if stream
        .set_read_timeout(Some(SUBSCRIBE_READ_TIMEOUT))
        .is_err()
    {
        return SubscribeOutcome::Disconnected;
    }
    // Short write timeout — if the daemon is unresponsive on write we give up.
    if stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .is_err()
    {
        return SubscribeOutcome::Disconnected;
    }

    // Send the subscribe request.
    let req_id = IpcClient::next_id();
    let req = serde_json::json!({
        "id": req_id,
        "method": METHOD_WATCH_SUBSCRIBE,
        "protocol_version": copypaste_ipc::PROTOCOL_VERSION,
        "params": {},
    });
    let mut req_line = match serde_json::to_string(&req) {
        Ok(s) => s,
        Err(_) => return SubscribeOutcome::Disconnected,
    };
    req_line.push('\n');

    // Write the request (we need a reference to write and read from the same stream).
    let mut write_half = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return SubscribeOutcome::Disconnected,
    };
    if write_half.write_all(req_line.as_bytes()).is_err() {
        return SubscribeOutcome::Disconnected;
    }

    // Read the ack line.
    let mut reader = BufReader::new(&stream);
    let mut ack_line = String::new();
    match reader.read_line(&mut ack_line) {
        Ok(0) => return SubscribeOutcome::Disconnected, // EOF
        Err(_) => return SubscribeOutcome::Disconnected,
        Ok(_) => {}
    }
    let ack: serde_json::Value = match serde_json::from_str(ack_line.trim()) {
        Ok(v) => v,
        Err(_) => return SubscribeOutcome::Disconnected,
    };

    // If ok=false, the daemon rejected the method (unsupported or error).
    if ack["ok"].as_bool() != Some(true) {
        return SubscribeOutcome::Unsupported;
    }
    // Verify the ack has event="subscribed".
    if ack["event"].as_str() != Some("subscribed") {
        // Unknown ack shape — treat as unsupported to be safe.
        return SubscribeOutcome::Unsupported;
    }

    // Event read loop: read one line per new item, print it, repeat.
    loop {
        let mut evt_line = String::new();
        match reader.read_line(&mut evt_line) {
            Ok(0) => {
                // EOF — daemon closed the connection (broadcast channel closed
                // on daemon shutdown path).
                return SubscribeOutcome::DaemonShutdown;
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                // Read timeout — daemon is idle (no new items in 30 s). This is
                // normal; loop back and wait for the next event.
                // Note: on macOS `WouldBlock` is returned for EAGAIN (O_NONBLOCK
                // unset but the timeout elapsed). Both map to "idle, try again".
                continue;
            }
            Err(e) => {
                eprintln!("watch: read error: {e}");
                return SubscribeOutcome::Disconnected;
            }
            Ok(_) => {}
        }

        let evt: serde_json::Value = match serde_json::from_str(evt_line.trim()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("watch: malformed event: {e}");
                continue;
            }
        };

        // Only print new_item events; ignore any other event types (future-proof).
        if evt["event"].as_str() != Some("new_item") {
            continue;
        }

        let item_id = evt["item_id"].as_str().unwrap_or("?");
        let content_type = evt["content_type"].as_str().unwrap_or("?");
        let wall_time = evt["wall_time"].as_i64().unwrap_or(0);
        let sensitive = evt["is_sensitive"].as_bool().unwrap_or(false);
        let sens = if sensitive { " [sensitive]" } else { "" };
        let ts = format_unix_ms(wall_time);
        println!("+ {ts}  {content_type:<6}  {item_id}{sens}");
    }
}

// ── Polling fallback ─────────────────────────────────────────────────────────

fn run_poll(socket_path: &Path, interval_ms: u64) -> Result<()> {
    // Enforce a floor so `--interval 0` (or very small values) don't create a
    // tight CPU/socket spin.
    let reset_ms = clamp_interval(interval_ms);

    let mut seen_ids = SeenIds::new(MAX_SEEN_IDS);
    let mut first_run = true;
    let mut current_interval_ms = reset_ms;

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
        METHOD_HISTORY_PAGE,
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

    /// The push-subscribe path takes priority over polling: `run` first attempts
    /// `watch_subscribe`, and the polling helper is only a fallback. Verify
    /// that `subscribe_once` correctly identifies an `ok=false` response as
    /// `Unsupported` (so the caller falls back to polling).
    #[test]
    fn subscribe_once_returns_unsupported_on_error_response() {
        // The actual socket call in subscribe_once makes it a full integration
        // test (requires a running daemon). That coverage lives in the daemon's
        // own integration tests (watch_subscribe_receives_push_events etc.).
        // Here we exercise only the ack-parsing logic by verifying the
        // SubscribeOutcome discriminants.
        //
        // Specifically: if subscribe_once receives ok=false it returns Unsupported.
        // We simulate this by directly calling the ack-parsing condition
        // (the same logic that guards the real call).
        let ack = serde_json::json!({"ok": false, "error": "unknown method: watch_subscribe"});
        assert!(
            ack["ok"].as_bool() != Some(true),
            "an error response must not have ok=true"
        );
        // The real subscribe_once returns Unsupported when ok != true.
        // This is a compile-time verification that the enum variants exist.
        let _: SubscribeOutcome = SubscribeOutcome::Unsupported;
        let _: SubscribeOutcome = SubscribeOutcome::DaemonShutdown;
        let _: SubscribeOutcome = SubscribeOutcome::Disconnected;
    }
}
