use copypaste_core::sync::{apply_remote_version, InstantClipboard, RemoteVersion};
use copypaste_core::{Keyring, PayloadMetadata, Store};
use copypaste_ipc::{ClipboardPrivacy, ConfigData};

fn record(
    store: &Store,
    keys: &Keyring,
    text: &str,
    stamp: i64,
    secret: bool,
) -> copypaste_core::StoredItem {
    let metadata = secret.then_some(PayloadMetadata {
        privacy: ClipboardPrivacy {
            secret: true,
            transient: false,
        },
        ..Default::default()
    });
    let settings = ConfigData {
        skip_secret: false,
        ..Default::default()
    };
    copypaste_core::ingest::ingest_into_with_capture_source_metadata_with_current_retention(
        store,
        keys,
        text,
        "text",
        stamp,
        None,
        None,
        metadata.as_ref(),
        &settings,
        || settings.clone(),
    )
    .unwrap()
    .into_item()
}

#[test]
fn secret_dedup_upgrades_remove_indexes_and_never_declassify() {
    let keys = Keyring::from_secret(&[91; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let stamp = copypaste_core::now_ms();
    let ordinary = record(
        &store,
        &keys,
        "synthetic confidentiality canary",
        stamp,
        false,
    );
    assert_eq!(store.search("confidentiality", 10).unwrap().len(), 1);
    let in_flight = store.semantic_work("model:test").unwrap().unwrap();
    let secret = record(
        &store,
        &keys,
        "synthetic confidentiality canary",
        stamp,
        true,
    );
    assert_eq!(ordinary.id, secret.id);
    assert!(secret.created_at > ordinary.created_at);
    assert!(secret.clipboard_privacy().secret);
    assert!(store.search("confidentiality", 10).unwrap().is_empty());
    assert!(store.semantic_work("model:test").unwrap().is_none());
    assert!(!store
        .save_semantic_work("module:test", "model:test", &in_flight, &[vec![1.0]])
        .unwrap());
    let recopied = record(
        &store,
        &keys,
        "synthetic confidentiality canary",
        stamp + 100,
        false,
    );
    assert!(recopied.clipboard_privacy().secret);
    assert!(store.search("confidentiality", 10).unwrap().is_empty());
    assert_eq!(store.count().unwrap(), 1);
}

#[test]
fn privacy_survives_sync_and_blocks_automatic_clipboard_writes() {
    let keys = Keyring::from_secret(&[92; 32]);
    let sender = Store::open_in_memory(&keys.db_key()).unwrap();
    let receiver_keys = Keyring::from_secret(&[93; 32]);
    let receiver = Store::open_in_memory(&receiver_keys.db_key()).unwrap();
    let stamp = copypaste_core::now_ms();
    let item = record(&sender, &keys, "synthetic remote secret", stamp, true);
    let metadata = sender.payload_metadata_for_sync(&item).unwrap().unwrap();
    let mut incoming = RemoteVersion {
        item_id: &item.id,
        content: "synthetic remote secret",
        binary_content: None,
        payload_metadata: Some(&metadata),
        content_type: "text",
        created_at: item.created_at,
        deleted: false,
        content_hash: None,
        origin_device_id: "sender",
        app_bundle_id: None,
        app_name: None,
    };
    assert!(apply_remote_version(&receiver, &receiver_keys, "receiver", &incoming).unwrap());
    let received = receiver.get(&item.id).unwrap().unwrap();
    assert!(received.clipboard_privacy().secret);
    assert!(receiver.search("remote", 10).unwrap().is_empty());
    assert!(receiver.semantic_work("model:test").unwrap().is_none());
    InstantClipboard::default().apply(
        &receiver,
        &receiver_keys,
        &received,
        ConfigData::default,
        |_| panic!("a secret must never be automatically copied"),
    );
    // A newer ordinary version of the same content cannot downgrade classification.
    incoming.created_at += 100;
    incoming.payload_metadata = None;
    assert!(apply_remote_version(&receiver, &receiver_keys, "receiver", &incoming).unwrap());
    assert!(
        receiver
            .get(&item.id)
            .unwrap()
            .unwrap()
            .clipboard_privacy()
            .secret
    );
    assert!(receiver.search("remote", 10).unwrap().is_empty());
}

#[test]
fn corrupt_privacy_preferences_fail_closed() {
    let (config, health) =
        copypaste_core::settings_record::read(r#"{"skip_secret":null,"skip_transient":"invalid"}"#);
    assert!(config.skip_secret && config.skip_transient);
    assert_eq!(health.unreadable_fields, ["skip_secret", "skip_transient"]);
}

#[test]
fn icon_normalization_and_sync_hydration_preserve_classification() {
    let keys = Keyring::from_secret(&[94; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let mut image = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1, 1)
        .write_to(&mut image, image::ImageFormat::Png)
        .unwrap();
    let metadata = PayloadMetadata {
        privacy: ClipboardPrivacy {
            secret: true,
            transient: true,
        },
        file: None,
        source_app_icon: copypaste_core::SourceAppIconMetadata::new(&image.into_inner(), 1, 1),
    };
    let settings = ConfigData {
        skip_secret: false,
        skip_transient: false,
        ..Default::default()
    };
    let stored =
        copypaste_core::ingest::ingest_into_with_capture_source_metadata_with_current_retention(
            &store,
            &keys,
            "synthetic icon secret",
            "text",
            copypaste_core::now_ms(),
            Some("com.example.synthetic"),
            Some("Synthetic"),
            Some(&metadata),
            &settings,
            || settings.clone(),
        )
        .unwrap()
        .into_item();
    assert!(stored.source_icon_id.is_some());
    assert!(stored.clipboard_privacy().secret && stored.clipboard_privacy().transient);
    let sync = store.payload_metadata_for_sync(&stored).unwrap().unwrap();
    let hydrated = PayloadMetadata::from_json(&sync, "text").unwrap();
    assert_eq!(hydrated.privacy, metadata.privacy);
    assert!(hydrated.source_app_icon.is_some());
}

struct SyncDevice {
    store: Store,
    keys: std::sync::Arc<Keyring>,
    source: copypaste_core::StoreSource,
}

impl SyncDevice {
    fn new(secret: u8, name: &str) -> Self {
        let keys = std::sync::Arc::new(Keyring::from_secret(&[secret; 32]));
        let store = Store::open_in_memory(&keys.db_key()).unwrap();
        let identity = store.device_identity(name).unwrap();
        let source = copypaste_core::StoreSource::new(
            store.clone(),
            std::sync::Arc::clone(&keys),
            identity.device_id,
            identity.device_name,
            ConfigData::default(),
        );
        Self {
            store,
            keys,
            source,
        }
    }
}

async fn peer_round(sender: &SyncDevice, receiver: &SyncDevice) {
    use copypaste_p2p::node::NoiseChannel;
    use copypaste_p2p::sync::{run_initiator, run_responder, SyncCursor};
    use copypaste_p2p::Session;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let psk = [63; 32];
    let (sent, received) = tokio::join!(
        async {
            let session = Session::connect(address, &psk).await.unwrap();
            let mut channel = NoiseChannel::new(session);
            let result =
                run_initiator(&mut channel, &sender.source, None, SyncCursor::default()).await;
            channel.close().await;
            result.unwrap()
        },
        async {
            let (stream, _) = listener.accept().await.unwrap();
            let session = Session::accept(stream, &psk).await.unwrap();
            let mut channel = NoiseChannel::new(session);
            let result =
                run_responder(&mut channel, &receiver.source, None, SyncCursor::default()).await;
            channel.close().await;
            result.unwrap()
        }
    );
    assert_eq!(sent.stats.skipped_unavailable, 0);
    assert_eq!(received.stats.skipped_unavailable, 0);
    assert_eq!(sent.stats.sent, 5);
    assert_eq!(received.stats.received, 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn confidential_and_legacy_text_sync_through_encrypted_peers_without_refusals() {
    let sender = SyncDevice::new(101, "sender");
    let relay = SyncDevice::new(102, "relay");
    let receiver = SyncDevice::new(103, "receiver");
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1, 1)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let icon = copypaste_core::SourceAppIconMetadata::new(&png.into_inner(), 1, 1).unwrap();
    let private = ClipboardPrivacy {
        secret: true,
        transient: false,
    };
    let both = ClipboardPrivacy {
        secret: true,
        transient: true,
    };
    let transient = ClipboardPrivacy {
        secret: false,
        transient: true,
    };
    let encoded = |privacy, source_app_icon| {
        PayloadMetadata {
            privacy,
            source_app_icon,
            ..Default::default()
        }
        .to_json("text")
        .unwrap()
    };
    let cases = [
        ("private", encoded(private, None), private, false),
        ("icon", encoded(both, Some(icon)), both, true),
        ("transient", encoded(transient, None), transient, false),
        (
            "legacy",
            r#"{"legacy_private":true}"#.into(),
            private,
            false,
        ),
        (
            "malformed",
            "unrecognized old metadata".into(),
            private,
            false,
        ),
    ];
    let mut originals = Vec::new();
    for (index, (id, metadata, _, _)) in cases.iter().enumerate() {
        let text = format!("synthetic {id} record");
        let (nonce, content_ciphertext) =
            copypaste_core::encrypt(text.as_bytes(), &sender.keys.item_key(), id).unwrap();
        sender
            .store
            .insert(copypaste_core::NewItem {
                id: (*id).into(),
                nonce,
                content_ciphertext,
                content_type: "text".into(),
                content_hash: copypaste_core::compute_content_hash(text.as_bytes()),
                search_text: None,
                created_at: 1000 + index as i64,
                app_bundle_id: Some(format!("com.example.{id}")),
                app_name: Some("Synthetic".into()),
                payload_metadata: Some(metadata.clone()),
            })
            .unwrap();
        originals.push(sender.store.get(id).unwrap().unwrap().payload_metadata);
    }
    peer_round(&sender, &relay).await;
    peer_round(&relay, &receiver).await;
    for (index, (id, _, privacy, has_icon)) in cases.iter().enumerate() {
        for target in [&relay, &receiver] {
            let row = target
                .store
                .get(id)
                .unwrap()
                .expect("the record must synchronize");
            assert_eq!(row.clipboard_privacy(), *privacy);
            let clear = copypaste_core::decrypt(
                &row.content_ciphertext,
                &row.nonce,
                &target.keys.item_key(),
                id,
            )
            .unwrap();
            assert_eq!(
                clear.as_slice(),
                format!("synthetic {id} record").as_bytes()
            );
            let metadata = target
                .store
                .payload_metadata_for_sync(&row)
                .unwrap()
                .unwrap();
            let metadata = PayloadMetadata::from_json(&metadata, "text").unwrap();
            assert_eq!(metadata.source_app_icon.is_some(), *has_icon);
            if privacy.secret {
                assert!(target.store.search(id, 10).unwrap().is_empty());
            }
        }
        assert_eq!(
            sender.store.get(id).unwrap().unwrap().payload_metadata,
            originals[index]
        );
    }
}
