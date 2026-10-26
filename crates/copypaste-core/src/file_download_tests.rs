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
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

struct ObservedSource {
    source: StoreSource,
    reads: AtomicUsize,
}
impl SyncSource for ObservedSource {
    fn deferred_file(
        &self,
        id: &str,
    ) -> Result<Option<Box<dyn copypaste_p2p::sync::DeferredFileSource>>, SyncError> {
        self.source.deferred_file(id)
    }
    fn observe_file_transfer(
        &self,
        id: &str,
        peer: &str,
    ) -> Option<Box<dyn copypaste_p2p::sync::FileTransferObserver>> {
        self.source.observe_file_transfer(id, peer)
    }
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
    fn local_summaries(
        &self,
        ids: &[String],
    ) -> Result<Vec<copypaste_p2p::protocol::ItemSummary>, SyncError> {
        self.source.local_summaries(ids)
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
    fn file_size(&self, id: &str, hash: &str) -> Result<u64, SyncError> {
        self.source.file_size(id, hash)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn six_megabyte_file_syncs_only_its_offer_then_downloads_verifies_and_copies() {
    exercise_download(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reference_offer_downloads_original_on_demand_and_records_both_directions() {
    exercise_download(true).await;
}
async fn exercise_download(deferred: bool) {
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
    let row = if deferred {
        crate::file_reference::import(
            &source,
            &source_key,
            original.to_str().unwrap(),
            "Hail-v1.10.0.apk",
            "application/vnd.android.package-archive",
            &config,
            None,
        )
        .unwrap()
    } else {
        file_import::import_file(
            &source,
            &source_key,
            &original,
            "Hail-v1.10.0.apk",
            "application/vnd.android.package-archive",
            original.to_str(),
            &config,
        )
        .unwrap()
    };
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
        |_| {},
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
    let downloads = Arc::new(FileDownloads::default());
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

    let source_completed = source.subscribe_file_operations();
    let progress = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = Arc::clone(&progress);
    downloads
        .save_with_progress(
            &target,
            &target_key,
            &receiver,
            "receiver",
            &config,
            crate::file_download::FileDownloadRequest {
                id: &row.id,
                destination: &destination,
                token: Some("remote-progress"),
            },
            Arc::new(move |written, total| observed.lock().unwrap().push((written, total))),
        )
        .await
        .unwrap();
    {
        let progress = progress.lock().unwrap();
        assert_eq!(
            progress.last(),
            Some(&(expected.size_bytes, expected.size_bytes))
        );
        assert!(progress.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
    if !deferred {
        assert_eq!(
            source_view.reads.load(Ordering::SeqCst) as u64,
            expected
                .size_bytes
                .div_ceil(file_transfer::CHUNK_BYTES as u64)
        );
    }
    let mut saved = std::fs::File::open(&destination).unwrap();
    let actual = file_transfer::digest(&mut saved, u64::MAX).unwrap();
    assert_eq!(actual.size_bytes, expected.size_bytes);
    if !deferred {
        assert_eq!(actual.sha256, expected.sha256);
    }
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        std::fs::read(&original).unwrap()
    );
    wait_source_terminal(source_completed).await;
    let records = target.file_operations(false, None, None).unwrap();
    assert!(records.iter().any(|record| record.phase
        == copypaste_ipc::FileOperationPhase::Completed
        && record.local_path.as_deref() == destination.to_str()));
    assert!(source
        .file_operations(false, None, None)
        .unwrap()
        .iter()
        .any(
            |record| record.kind == copypaste_ipc::FileOperationKind::Send
                && record.phase == copypaste_ipc::FileOperationPhase::Completed
        ));
    let copy = file_transfer::prepare_copy(&target, &target_key.item_key(), &remote, u64::MAX)
        .unwrap()
        .unwrap();
    let path = copy.payload.source_reference().unwrap().to_owned();
    assert_eq!(std::path::Path::new(&path), destination);
    assert!(std::path::Path::new(&path).is_file());
    drop(copy);
    assert!(
        std::path::Path::new(&path).is_file(),
        "Copy reuses the user-owned download without a disposable duplicate"
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

    let source_cancelled = source.subscribe_file_operations();
    let cancelled = dir.path().join("cancelled.apk");
    let cancellation_started = Arc::new(AtomicBool::new(false));
    let cancel_downloads = Arc::clone(&downloads);
    let cancel_id = row.id.clone();
    let cancellation_started_for_progress = Arc::clone(&cancellation_started);
    let result = downloads
        .save_with_progress(
            &target,
            &target_key,
            &receiver,
            "receiver",
            &config,
            crate::file_download::FileDownloadRequest {
                id: &row.id,
                destination: &cancelled,
                token: None,
            },
            Arc::new(move |written, _| {
                if written > 0 && !cancellation_started_for_progress.swap(true, Ordering::SeqCst) {
                    cancel_downloads.cancel(&cancel_id);
                }
            }),
        )
        .await;
    assert!(
        cancellation_started.load(Ordering::SeqCst),
        "progress must cancel a transfer after its first written bytes"
    );
    assert!(matches!(
        result,
        Err(file_transfer::FileTransferError::Cancelled)
    ));
    assert!(
        !cancelled.exists(),
        "Cancelled downloads remove their partial destination"
    );

    // Source bookkeeping is asynchronous; its terminal event also confirms
    // file admission has been released before testing a different failure.
    wait_source_terminal(source_cancelled).await;

    if deferred {
        std::fs::OpenOptions::new()
            .write(true)
            .open(&original)
            .unwrap()
            .set_len(expected.size_bytes + 1)
            .unwrap();
        let changed = dir.path().join("changed.apk");
        let error = downloads
            .save(
                &target,
                &target_key,
                &receiver,
                "receiver",
                &row.id,
                &changed,
                &config,
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("original file changed"),
            "{error}"
        );
        assert!(!changed.exists());
        assert!(target
            .file_operations(false, None, None)
            .unwrap()
            .iter()
            .any(|record| record
                .error
                .as_deref()
                .is_some_and(|error| error.contains("original file changed"))));
    }

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

#[test]
fn detached_admission_is_bounded_released_and_closed_on_shutdown() {
    let downloads = FileDownloads::default();
    let slot = downloads.reserve_background().unwrap();
    assert!(matches!(
        downloads.reserve_background(),
        Err(file_transfer::FileTransferError::Busy)
    ));
    drop(slot);
    let slot = downloads.reserve_background().unwrap();
    downloads.cancel_all();
    drop(slot);
    assert!(matches!(
        downloads.reserve_background(),
        Err(file_transfer::FileTransferError::Cancelled)
    ));
}

async fn wait_source_terminal(
    mut events: tokio::sync::broadcast::Receiver<copypaste_ipc::FileOperationUpdate>,
) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match events.recv().await {
                Ok(event)
                    if event.phase.as_ref().is_some_and(|phase| {
                        *phase != copypaste_ipc::FileOperationPhase::Running
                    }) =>
                {
                    break
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(error) => panic!("Source operation events closed: {error}"),
            }
        }
    })
    .await
    .expect("The sender must publish its terminal operation state");
}
