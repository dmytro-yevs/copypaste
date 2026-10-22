//! End-to-end encrypted file transfer over loopback.
//!
//! This drives `Node::download_file` and the regular listener through Noise
//! over TCP. The source synthesises one 64 KiB block at a time and the receiver
//! hashes and discards it, so the 32 MiB fixture does not become a whole-file
//! memory benchmark. Loopback deliberately removes Wi-Fi, Android and disk
//! variables; its result is a transport regression signal, not device evidence.

use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::watch;

use copypaste_p2p::node::listen;
use copypaste_p2p::peers::{Peer, PeerStore};
use copypaste_p2p::protocol::{ItemSummary, SyncItem, FILE_CHUNK_BYTES};
use copypaste_p2p::sync::{SyncError, SyncSource};
use copypaste_p2p::{Node, PairingToken};

const TRANSFER_BYTES: usize = 32 * 1024 * 1024;
const STORE_KEY: [u8; 32] = [7; 32];
const ITEM_ID: &str = "benchmark-file";

struct GeneratedFile {
    size: usize,
    hash: String,
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
        }
    }
}

impl SyncSource for GeneratedFile {
    fn file_size(&self, id: &str, hash: &str) -> Result<u64, SyncError> {
        if id == ITEM_ID && hash == self.hash {
            Ok(self.size as u64)
        } else {
            Err(SyncError::Source("The file is unavailable.".into()))
        }
    }

    fn file_chunk(&self, id: &str, hash: &str, index: u64) -> Result<Vec<u8>, SyncError> {
        if id != ITEM_ID
            || hash != self.hash
            || index as usize >= self.size.div_ceil(FILE_CHUNK_BYTES)
        {
            return Err(SyncError::Source("The file is unavailable.".into()));
        }
        Ok(chunk(index, expected_len(self.size, index)))
    }

    fn device_id(&self) -> String {
        "benchmark-source".into()
    }

    fn device_name(&self) -> String {
        "Benchmark source".into()
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
    _stores: (tempfile::TempDir, tempfile::TempDir),
}

impl Fixture {
    async fn new() -> Self {
        let source = Arc::new(GeneratedFile::new(TRANSFER_BYTES));
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
                name: "benchmark client".into(),
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
            name: "benchmark server".into(),
            psk,
            last_addr: Some(addr),
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        };
        client.peers().upsert(peer.clone()).expect("client pairing");
        let (shutdown, receiver) = watch::channel(false);
        tokio::spawn(listen(
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
            _stores: (server_dir, client_dir),
        }
    }

    async fn transfer(&self) {
        let mut received = 0usize;
        let mut hasher = Sha256::new();
        let hash = self
            .client
            .download_file(
                &self.peer,
                ITEM_ID,
                &self.source.hash,
                self.source.size as u64,
                |bytes| {
                    received += bytes.len();
                    hasher.update(bytes);
                    Ok(())
                },
            )
            .await
            .expect("download");
        assert_eq!(received, self.source.size, "received whole payload");
        assert_eq!(hash, self.source.hash, "source hash");
        assert_eq!(
            hex::encode(hasher.finalize()),
            self.source.hash,
            "payload hash"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

fn expected_len(size: usize, index: u64) -> usize {
    size.saturating_sub(index as usize * FILE_CHUNK_BYTES)
        .min(FILE_CHUNK_BYTES)
}

fn chunk(index: u64, len: usize) -> Vec<u8> {
    let start = index as usize * FILE_CHUNK_BYTES;
    (0..len)
        .map(|offset| ((start + offset).wrapping_mul(31).wrapping_add(17) & 0xff) as u8)
        .collect()
}

fn encrypted_loopback_file_transfer(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let fixture = runtime.block_on(Fixture::new());
    let mut group = c.benchmark_group("p2p/file_transfer/encrypted_loopback");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Bytes(TRANSFER_BYTES as u64));
    group.bench_function("32MiB", |bench| {
        bench.iter(|| runtime.block_on(fixture.transfer()));
    });
    group.finish();
}

criterion_group!(benches, encrypted_loopback_file_transfer);
criterion_main!(benches);
