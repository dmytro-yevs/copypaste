use crate::{
    file_download::FileDownloads, file_import, file_transfer, Keyring, Store, StoreSource,
};
use copypaste_p2p::{
    peers::PeerStore,
    sync::{SyncError, SyncSource},
    Node, PairingToken, Peer,
};
use std::{
    io::Write,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct ObservedSource {
    source: StoreSource,
    reads: AtomicUsize,
}
impl SyncSource for ObservedSource {
    fn device_id(&self) -> String {
        self.source.device_id()
    }
    fn device_name(&self) -> String {
        self.source.device_name()
    }
    fn summaries(
        &self,
        since: i64,
    ) -> Result<Vec<copypaste_p2p::protocol::ItemSummary>, SyncError> {
        self.source.summaries(since)
    }
    fn fetch(&self, ids: &[String]) -> Result<Vec<copypaste_p2p::protocol::SyncItem>, SyncError> {
        self.source.fetch(ids)
    }
    fn apply(&self, item: copypaste_p2p::protocol::SyncItem) -> Result<bool, SyncError> {
        self.source.apply(item)
    }
    fn file_chunk(&self, id: &str, hash: &str, index: u64) -> Result<Vec<u8>, SyncError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.source.file_chunk(id, hash, index)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn six_megabyte_file_syncs_only_its_offer_then_downloads_verifies_and_copies() {
    let dir = tempfile::tempdir().unwrap();
    let source_key = Arc::new(Keyring::from_secret(&[41; 32]));
    let target_key = Arc::new(Keyring::from_secret(&[42; 32]));
    let source = Store::open_in_memory(&source_key.db_key()).unwrap();
    let target = Store::open_in_memory(&target_key.db_key()).unwrap();
    let original = dir.path().join("Hail-v1.10.0.apk");
    if let Some(path) = std::env::var_os("COPYPASTE_TEST_FILE") {
        std::fs::copy(path, &original).unwrap();
    } else {
        let mut file = std::fs::File::create(&original).unwrap();
        let chunk = [0x5a; file_transfer::CHUNK_BYTES];
        let mut remaining = 6_158_760;
        while remaining > 0 {
            let size = remaining.min(chunk.len());
            file.write_all(&chunk[..size]).unwrap();
            remaining -= size;
        }
    }
    let config = copypaste_ipc::ConfigData::default();
    let row = file_import::import_file(
        &source,
        &source_key,
        &original,
        "Hail-v1.10.0.apk",
        "application/vnd.android.package-archive",
        original.to_str(),
        &config,
    )
    .unwrap();
    let expected = file_transfer::row_offer(&row, &source_key.item_key())
        .unwrap()
        .unwrap();
    assert!(expected.size_bytes > copypaste_ipc::MAX_CONTENT_BYTES as u64);
    let source_view = Arc::new(ObservedSource {
        source: StoreSource::new(
            source.clone(),
            Arc::clone(&source_key),
            "sender".into(),
            "Source".into(),
            config.clone(),
        ),
        reads: AtomicUsize::new(0),
    });
    let target_view = StoreSource::new(
        target.clone(),
        Arc::clone(&target_key),
        "receiver".into(),
        "Destination".into(),
        config.clone(),
    );
    let sender = Arc::new(Node::new(
        PeerStore::open(
            &dir.path().join("sender-peers"),
            &source_key.peer_store_key(),
        )
        .unwrap(),
        None,
        0,
        true,
    ));
    let receiver = Node::new(
        PeerStore::open(
            &dir.path().join("receiver-peers"),
            &target_key.peer_store_key(),
        )
        .unwrap(),
        None,
        0,
        true,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let token = PairingToken::generate();
    let peer = Peer {
        pairing_id: token.pairing_id(),
        device_id: Some("sender".into()),
        name: "Source".into(),
        psk: token.psk(),
        last_addr: Some(address),
        last_seen_ms: 0,
        profile: None,
        profile_observed_at_ms: 0,
    };
    receiver.peers().upsert(peer.clone()).unwrap();
    let mut sender_peer = peer.clone();
    sender_peer.device_id = Some("receiver".into());
    sender.peers().upsert(sender_peer).unwrap();
    let (stop, shutdown) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(copypaste_p2p::node::listen(
        Arc::clone(&sender),
        listener,
        Arc::clone(&source_view),
        |_: &str, _: &copypaste_p2p::sync::SyncOutcome| {},
        || {},
        shutdown,
    ));
    receiver.sync_one(&peer, &target_view).await.unwrap();
    assert_eq!(
        source_view.reads.load(Ordering::SeqCst),
        0,
        "History sync must never request a file chunk"
    );
    assert!(!target.has_file_transfer(&row.id).unwrap());
    let remote = target.get(&row.id).unwrap().unwrap();
    assert_eq!(
        file_transfer::row_offer(&remote, &target_key.item_key())
            .unwrap()
            .unwrap(),
        expected
    );
    assert!(matches!(
        file_transfer::prepare_copy(&target, &target_key.item_key(), &remote, u64::MAX),
        Err(file_transfer::FileTransferError::DownloadFirst)
    ));
    let wire = source_view.fetch(std::slice::from_ref(&row.id)).unwrap();
    assert!(wire[0].binary_content.len() < 256);

    let destination = dir.path().join("received.apk");
    let downloads = FileDownloads::default();
    downloads.cancel("pre-cancelled-token");
    let early = dir.path().join("pre-cancelled.apk");
    assert!(matches!(
        downloads
            .save_with_token(
                &target,
                &target_key,
                &receiver,
                "receiver",
                &config,
                crate::file_download::FileDownloadRequest {
                    id: &row.id,
                    destination: &early,
                    token: Some("pre-cancelled-token")
                }
            )
            .await,
        Err(file_transfer::FileTransferError::Cancelled)
    ));
    assert!(!early.exists());

    downloads
        .save(
            &target,
            &target_key,
            &receiver,
            "receiver",
            &row.id,
            &destination,
            &config,
        )
        .await
        .unwrap();
    assert_eq!(
        source_view.reads.load(Ordering::SeqCst) as u64,
        expected
            .size_bytes
            .div_ceil(file_transfer::CHUNK_BYTES as u64)
    );
    let mut saved = std::fs::File::open(&destination).unwrap();
    assert_eq!(
        file_transfer::digest(&mut saved, u64::MAX).unwrap(),
        expected
    );
    let copy = file_transfer::prepare_copy(&target, &target_key.item_key(), &remote, u64::MAX)
        .unwrap()
        .unwrap();
    let path = copy.payload.source_reference().unwrap().to_owned();
    assert!(std::path::Path::new(&path).is_file());
    drop(copy);
    assert!(
        !std::path::Path::new(&path).exists(),
        "The scoped plaintext is released after the native writer returns"
    );

    let limited = copypaste_ipc::ConfigData {
        max_file_size_bytes: 4 * 1024 * 1024,
        ..config.clone()
    };
    let refused = dir.path().join("too-large.apk");
    let error = downloads
        .save(
            &target,
            &target_key,
            &receiver,
            "receiver",
            &row.id,
            &refused,
            &limited,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("4194304"));
    assert!(!refused.exists());

    let cancelled = dir.path().join("cancelled.apk");
    let transfer = downloads.save(
        &target,
        &target_key,
        &receiver,
        "receiver",
        &row.id,
        &cancelled,
        &config,
    );
    let cancel = async {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !cancelled.exists() || cancelled.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
                tokio::task::yield_now().await;
            }
            downloads.cancel(&row.id);
        })
        .await
        .expect("The explicit transfer must start producing bytes");
    };
    let (result, ()) = tokio::join!(transfer, cancel);
    assert!(matches!(
        result,
        Err(file_transfer::FileTransferError::Cancelled)
    ));
    assert!(
        !cancelled.exists(),
        "Cancelled downloads remove their partial destination"
    );

    stop.send(true).unwrap();
    server.await.unwrap();
    let offline = dir.path().join("offline.apk");
    let error = downloads
        .save(
            &target,
            &target_key,
            &receiver,
            "receiver",
            &row.id,
            &offline,
            &config,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("unavailable"));
    assert!(!offline.exists());
}
