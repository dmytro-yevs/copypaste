//! Fixtures shared by the sync test modules.
//!
//! Test-only. The comparator tests, the planner tests and the session tests all
//! build the same shapes, and the session tests need a store and a channel that
//! behave the way the daemon's must — so those live here once rather than three
//! times.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use tokio::sync::mpsc;

use super::{
    merge_decision, run_initiator, run_responder, MergeDecision, SyncChannel, SyncError,
    SyncOutcome, SyncSource,
};
use crate::protocol::{content_hash, ItemSummary, SyncItem, SyncMessage};

pub(super) fn summary(id: &str, created_at: i64, hash: &str, deleted: bool) -> ItemSummary {
    ItemSummary {
        item_id: id.into(),
        created_at,
        deleted,
        content_hash: hash.into(),
        origin_device_id: "test-device".into(),
        pinned: false,
        pin_order: None,
    }
}

pub(crate) fn item(id: &str, created_at: i64, content: &str, origin: &str) -> SyncItem {
    SyncItem {
        item_id: id.into(),
        content: content.into(),
        content_type: "text".into(),
        created_at,
        deleted: false,
        // A real digest, because the receiving session recomputes it: a fixture
        // with a stand-in hash would be dropped exactly as a hostile peer's is.
        content_hash: content_hash(content),
        origin_device_id: origin.into(),
        pinned: false,
        pin_order: None,
    }
}

pub(super) fn tombstone(id: &str, created_at: i64, hash: &str, origin: &str) -> SyncItem {
    SyncItem {
        item_id: id.into(),
        content: String::new(),
        content_type: "text".into(),
        created_at,
        deleted: true,
        content_hash: hash.into(),
        origin_device_id: origin.into(),
        pinned: false,
        pin_order: None,
    }
}

/// An in-memory store that behaves the way the daemon's `Store` must:
/// `apply` re-runs the comparator against what it holds, using the true
/// origins, and reports whether anything changed.
pub(crate) struct TestSource {
    device_id: String,
    device_name: String,
    items: Mutex<HashMap<String, SyncItem>>,
    sensitive: Mutex<HashSet<String>>,
    /// Items the source hands back even though they were not requested —
    /// used to prove the session refuses them.
    smuggle: Mutex<Vec<SyncItem>>,
}

impl TestSource {
    pub(crate) fn new(device_id: &str, items: Vec<SyncItem>) -> Self {
        Self {
            device_id: device_id.into(),
            device_name: format!("{device_id} name"),
            items: Mutex::new(items.into_iter().map(|i| (i.item_id.clone(), i)).collect()),
            sensitive: Mutex::new(HashSet::new()),
            smuggle: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn mark_sensitive(&self, id: &str) {
        self.sensitive.lock().unwrap().insert(id.into());
    }

    /// Arrange for `fetch` to return this item whatever it was asked for.
    pub(crate) fn smuggle(&self, item: SyncItem) {
        self.smuggle.lock().unwrap().push(item);
    }

    pub(crate) fn snapshot(&self) -> Vec<SyncItem> {
        let mut v: Vec<_> = self.items.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| a.item_id.cmp(&b.item_id));
        v
    }

    pub(crate) fn get(&self, id: &str) -> Option<SyncItem> {
        self.items.lock().unwrap().get(id).cloned()
    }
}

impl SyncSource for TestSource {
    fn device_id(&self) -> String {
        self.device_id.clone()
    }

    fn device_name(&self) -> String {
        self.device_name.clone()
    }

    fn summaries(&self) -> Result<Vec<ItemSummary>, SyncError> {
        let sensitive = self.sensitive.lock().unwrap();
        let mut v: Vec<_> = self
            .items
            .lock()
            .unwrap()
            .values()
            .filter(|i| !sensitive.contains(&i.item_id))
            .map(|i| i.summary())
            .collect();
        v.sort_by(|a, b| a.item_id.cmp(&b.item_id));
        Ok(v)
    }

    fn fetch(&self, ids: &[String]) -> Result<Vec<SyncItem>, SyncError> {
        let sensitive = self.sensitive.lock().unwrap();
        let items = self.items.lock().unwrap();
        let mut out: Vec<_> = ids
            .iter()
            .filter(|id| !sensitive.contains(*id))
            .filter_map(|id| items.get(id).cloned())
            .collect();
        out.extend(self.smuggle.lock().unwrap().iter().cloned());
        Ok(out)
    }

    fn apply(&self, incoming: SyncItem) -> Result<bool, SyncError> {
        let mut items = self.items.lock().unwrap();
        if let Some(local) = items.get(&incoming.item_id) {
            let decision = merge_decision(
                &local.summary(),
                &local.origin_device_id,
                &incoming.summary(),
                &incoming.origin_device_id,
            );
            if decision == MergeDecision::KeepLocal {
                return Ok(false);
            }
        }
        items.insert(incoming.item_id.clone(), incoming);
        Ok(true)
    }
}

/// One end of an in-memory duplex. Messages go through the real encoder and
/// decoder, so every bound in `protocol` is exercised by every session test.
pub(super) struct MemChannel {
    tx: mpsc::Sender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
}

fn duplex() -> (MemChannel, MemChannel) {
    let (a_tx, b_rx) = mpsc::channel(8);
    let (b_tx, a_rx) = mpsc::channel(8);
    (
        MemChannel { tx: a_tx, rx: a_rx },
        MemChannel { tx: b_tx, rx: b_rx },
    )
}

impl SyncChannel for MemChannel {
    async fn send(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
        let bytes = msg.encode()?;
        self.tx
            .send(bytes)
            .await
            .map_err(|_| SyncError::Channel("peer went away".into()))
    }

    async fn recv(&mut self) -> Result<SyncMessage, SyncError> {
        let bytes = self
            .rx
            .recv()
            .await
            .ok_or_else(|| SyncError::Channel("peer went away".into()))?;
        Ok(SyncMessage::decode(&bytes)?)
    }
}

/// A channel that replays a fixed script and records what it was sent —
/// for the cases a well-behaved peer cannot produce.
pub(super) struct ScriptChannel {
    script: std::collections::VecDeque<SyncMessage>,
    pub(super) sent: Vec<SyncMessage>,
}

impl ScriptChannel {
    pub(super) fn new(script: Vec<SyncMessage>) -> Self {
        Self {
            script: script.into(),
            sent: Vec::new(),
        }
    }
}

impl SyncChannel for ScriptChannel {
    async fn send(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
        self.sent.push(msg);
        Ok(())
    }

    async fn recv(&mut self) -> Result<SyncMessage, SyncError> {
        self.script
            .pop_front()
            .ok_or_else(|| SyncError::Channel("script exhausted".into()))
    }
}

/// Runs both halves over one duplex.
///
/// Each half drops its end when it finishes, so a half that fails leaves the
/// other with a closed channel rather than a wait that never ends — a test
/// that hangs says far less than one that fails.
pub(super) async fn try_session(
    a: &TestSource,
    b: &TestSource,
) -> (
    Result<SyncOutcome, SyncError>,
    Result<SyncOutcome, SyncError>,
) {
    let (ca, cb) = duplex();
    tokio::join!(
        async move {
            let mut ca = ca;
            let out = run_initiator(&mut ca, a).await;
            drop(ca);
            out
        },
        async move {
            let mut cb = cb;
            let out = run_responder(&mut cb, b).await;
            drop(cb);
            out
        }
    )
}

pub(super) async fn session(a: &TestSource, b: &TestSource) -> (SyncOutcome, SyncOutcome) {
    let (ra, rb) = try_session(a, b).await;
    (ra.expect("initiator"), rb.expect("responder"))
}
