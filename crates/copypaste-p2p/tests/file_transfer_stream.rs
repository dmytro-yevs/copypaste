//! Regression coverage for the streamed encrypted file transport.
//!
//! The source generates bounded chunks on demand. It never stores the fixture
//! as one file-sized allocation, so this test exercises the real transport
//! without turning test memory into a throughput variable.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::watch;

use copypaste_p2p::node::listen;
use copypaste_p2p::peers::{Peer, PeerStore};
use copypaste_p2p::protocol::{ItemSummary, SyncItem, FILE_CHUNK_BYTES};
use copypaste_p2p::sync::{SyncError, SyncSource};
use copypaste_p2p::{Node, PairingToken};

const ITEM_ID: &str = "stream-test-file";
const STORE_KEY: [u8; 32] = [9; 32];

struct GeneratedFile {
    size: usize,
    hash: String,
    size_calls: AtomicUsize,
    chunk_calls: AtomicUsize,
}

impl GeneratedFile {
    fn new(size: usize) -> Self {
        let mut hasher = Sha256::new();
        for index in 0..size.div_ceil(FILE_CHUNK_BYTES) {
            hasher.update(chunk(index as u64, expected_len(size, index as u64)));
        }
        Self {
            size,
            hash: hex::encode(hasher.finalize()),
            size_calls: AtomicUsize::new(0),
            chunk_calls: AtomicUsize::new(0),
        }
    }
}

impl SyncSource for GeneratedFile {
    fn file_size(&self, id: &str, hash: &str) -> Result<u64, SyncError> {
        self.size_calls.fetch_add(1, Ordering::SeqCst);
        if id == ITEM_ID && hash == self.hash {
            Ok(self.size as u64)
        } else {
            Err(SyncError::Source("The file is unavailable.".into()))
        }
    }

    fn file_chunk(&self, id: &str, hash: &str, index: u64) -> Result<Vec<u8>, SyncError> {
        self.chunk_calls.fetch_add(1, Ordering::SeqCst);
        if id != ITEM_ID
            || hash != self.hash
            || index as usize >= self.size.div_ceil(FILE_CHUNK_BYTES)
        {
            return Err(SyncError::Source("The file is unavailable.".into()));
        }
        Ok(chunk(index, expected_len(self.size, index)))
    }

    fn device_id(&self) -> String {
        "stream-test-source".into()
    }

    fn device_name(&self) -> String {
        "Stream test source".into()
    }

    fn summaries(&self, _: i64) -> Result<Vec<ItemSummary>, SyncError> {
        Ok(Vec::new())
    }

    fn local_summaries(&self, _: &[String]) -> Result<Vec<ItemSummary>, SyncError> {
        Ok(Vec::new())
    }

    fn fetch(&self, _: &[String]) -> Result<Vec<SyncItem>, SyncError> {
        Ok(Vec::new())
    }

    fn apply(&self, _: SyncItem) -> Result<bool, SyncError> {
        Ok(false)
    }
}

struct Fixture {
    client: Node,
    peer: Peer,
    source: Arc<GeneratedFile>,
    shutdown: watch::Sender<bool>,
    listener: tokio::task::JoinHandle<()>,
    _stores: (tempfile::TempDir, tempfile::TempDir),
}

impl Fixture {
    async fn new(size: usize) -> Self {
        let source = Arc::new(GeneratedFile::new(size));
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        let psk = token.psk();
        let server_dir = tempfile::tempdir().expect("server tempdir");
        let client_dir = tempfile::tempdir().expect("client tempdir");
        let server_store =
            PeerStore::open(&server_dir.path().join("peers"), &STORE_KEY).expect("server peers");
        let client_store =
            PeerStore::open(&client_dir.path().join("peers"), &STORE_KEY).expect("client peers");
        let server = Arc::new(Node::new(server_store, None, 0, false));
        let client = Node::new(client_store, None, 0, false);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("listener address");

        server
            .peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "stream test client".into(),
                psk,
                last_addr: None,
                last_seen_ms: 0,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .expect("server pairing");
        let peer = Peer {
            pairing_id,
            device_id: None,
            name: "stream test server".into(),
            psk,
            last_addr: Some(addr),
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        };
        client.peers().upsert(peer.clone()).expect("client pairing");
        let (shutdown, receiver) = watch::channel(false);
        let listener = tokio::spawn(listen(
            Arc::clone(&server),
            listener,
            Arc::clone(&source),
            |_, _| {},
            || {},
            receiver,
        ));
        Self {
            client,
            peer,
            source,
            shutdown,
            listener,
            _stores: (server_dir, client_dir),
        }
    }

    async fn close(self) {
        let _ = self.shutdown.send(true);
        self.listener.await.expect("listener task");
    }
}

#[tokio::test]
async fn streams_one_request_through_noise_and_verifies_the_complete_payload() {
    let size = FILE_CHUNK_BYTES * 4 + 1;
    let fixture = Fixture::new(size).await;
    let mut received = 0usize;
    let mut hasher = Sha256::new();
    let result = fixture
        .client
        .download_file(
            &fixture.peer,
            ITEM_ID,
            &fixture.source.hash,
            size as u64,
            |bytes| {
                received += bytes.len();
                hasher.update(bytes);
                Ok(())
            },
        )
        .await
        .expect("download");

    assert_eq!(received, size);
    assert_eq!(result, fixture.source.hash);
    assert_eq!(hex::encode(hasher.finalize()), fixture.source.hash);
    assert_eq!(fixture.source.size_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        fixture.source.chunk_calls.load(Ordering::SeqCst),
        size.div_ceil(FILE_CHUNK_BYTES),
    );
    fixture.close().await;
}

#[tokio::test]
async fn rejects_a_size_mismatch_before_reading_any_chunk() {
    let fixture = Fixture::new(FILE_CHUNK_BYTES * 2).await;
    let error = fixture
        .client
        .download_file(
            &fixture.peer,
            ITEM_ID,
            &fixture.source.hash,
            (fixture.source.size - 1) as u64,
            |_| Ok(()),
        )
        .await
        .expect_err("mismatched size must fail");

    assert!(error.to_string().contains("size"));
    assert_eq!(fixture.source.size_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.source.chunk_calls.load(Ordering::SeqCst), 0);
    fixture.close().await;
}

fn expected_len(size: usize, index: u64) -> usize {
    size.saturating_sub(index as usize * FILE_CHUNK_BYTES)
        .min(FILE_CHUNK_BYTES)
}

fn chunk(index: u64, len: usize) -> Vec<u8> {
    let start = index as usize * FILE_CHUNK_BYTES;
    (0..len)
        .map(|offset| ((start + offset).wrapping_mul(29).wrapping_add(11) & 0xff) as u8)
        .collect()
}
