//! Real StoreSource sessions exercise encrypted spill on both runtime flavors.
use std::sync::Arc;

use copypaste_core::{Keyring, NewItem, Store, StoreSource};
use copypaste_p2p::protocol::SyncMessage;
use copypaste_p2p::sync::{run_initiator, run_responder, SyncChannel, SyncCursor, SyncError};
use tokio::sync::mpsc;

struct Channel {
    tx: mpsc::Sender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
}
impl SyncChannel for Channel {
    async fn send(&mut self, message: SyncMessage) -> Result<(), SyncError> {
        self.tx
            .send(message.encode()?)
            .await
            .map_err(|_| SyncError::Channel("closed".into()))
    }
    async fn recv(&mut self) -> Result<SyncMessage, SyncError> {
        SyncMessage::decode(
            &self
                .rx
                .recv()
                .await
                .ok_or_else(|| SyncError::Channel("closed".into()))?,
        )
        .map_err(Into::into)
    }
}
fn channels() -> (Channel, Channel) {
    let (a_tx, a_rx) = mpsc::channel(8);
    let (b_tx, b_rx) = mpsc::channel(8);
    (
        Channel { tx: a_tx, rx: b_rx },
        Channel { tx: b_tx, rx: a_rx },
    )
}

async fn large_session() {
    let root = tempfile::tempdir().unwrap();
    let keys_a = Arc::new(Keyring::from_secret(&[91; 32]));
    let keys_b = Arc::new(Keyring::from_secret(&[92; 32]));
    let a = Store::open(&root.path().join("a.db"), &keys_a.db_key()).unwrap();
    let b = Store::open(&root.path().join("b.db"), &keys_b.db_key()).unwrap();
    let now = copypaste_core::now_ms();
    for i in 0..5000 {
        let id = format!("item-{i:05}");
        let content = format!("content-{i}");
        let (nonce, content_ciphertext) =
            copypaste_core::encrypt(content.as_bytes(), &keys_a.item_key(), &id).unwrap();
        a.insert(NewItem {
            id,
            nonce,
            content_ciphertext,
            content_type: "text".into(),
            content_hash: copypaste_core::compute_content_hash(content.as_bytes()),
            search_text: None,
            created_at: now - 10_000 + i,
            app_bundle_id: None,
            app_name: None,
            payload_metadata: None,
        })
        .unwrap();
    }
    let source_a = StoreSource::new(a, keys_a, "device-a".into(), "A".into(), Default::default());
    let source_b = StoreSource::new(
        b.clone(),
        keys_b,
        "device-b".into(),
        "B".into(),
        Default::default(),
    );
    let mut left = SyncCursor::default();
    let mut right = SyncCursor::default();
    for _ in 0..6 {
        let (mut a, mut b) = channels();
        let (result_a, result_b) = tokio::join!(
            run_initiator(&mut a, &source_a, None, left),
            run_responder(&mut b, &source_b, None, right)
        );
        left = result_a.unwrap().cursor;
        right = result_b.unwrap().cursor;
    }
    assert!(root.path().join(".copypaste-sync-workspaces").is_dir());
    assert_eq!(b.count().unwrap(), 5000);
}

#[tokio::test(flavor = "current_thread")]
async fn encrypted_spill_session_converges_current_thread() {
    large_session().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_spill_session_converges_multi_thread() {
    large_session().await;
}
