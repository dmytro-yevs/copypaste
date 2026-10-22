use super::*;
use copypaste_module_sdk::{ModuleArchitecture, ModuleManifest, ModulePlatform, ModuleTarget};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    sync::atomic::AtomicUsize,
    thread,
};

struct Fixture {
    _directory: tempfile::TempDir,
    manager: Arc<ModuleManager>,
    store: Store,
    launched: Arc<AtomicUsize>,
    stopped: Arc<AtomicUsize>,
    entered: std::sync::mpsc::Receiver<()>,
    model_path: PathBuf,
    database_path: PathBuf,
    database_key: [u8; 32],
}

impl Fixture {
    fn new(text: Option<&str>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("modules");
        let keys = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let target = ModuleTarget::current().unwrap_or(ModuleTarget {
            platform: ModulePlatform::Macos,
            architecture: ModuleArchitecture::Aarch64,
        });
        let extension = if cfg!(windows) {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };
        let entry = format!("bin/module.{extension}");
        let hash = hex::encode(Sha256::digest(b"fixture"));
        let manifest = serde_json::json!({
            "schema_version":5,"api_version":1,"id":"copypaste.fixture-search","title":"Fixture","description":"Test embeddings.","version":"0.1.0","app_versions":">=1.0.0, <2.0.0",
            "supported_platforms":["macos","android","windows","linux"],"target":target,"entrypoint":entry,"unload_policy":"process",
            "files":[{"path":entry,"sha256":hash,"size_bytes":7}],
            "commands":[{"id":"embed","title":"Embed","description":"Fixture.","arguments":[
                {"id":"text","title":"Text","kind":"text","default":"","required":true},
                {"id":"role","title":"Role","kind":"text","default":"","required":true}
            ]}],
            "preferences":[{"id":"languages","title":"Languages","kind":"choices","options":[{"id":"en","title":"English"}],"default":["en"],"required":true}],
            "search_provider":{"command":"embed","language_field":"languages","models":[{
                "id":"fixture","title":"Fixture","languages":["en"],"dimensions":2,"max_tokens":16,"query_prefix":"","passage_prefix":"","minimum_similarity":0.8,
                "files":[
                    {"path":"model.onnx","url":"https://huggingface.co/fixture/resolve/0000000000000000000000000000000000000000/model.onnx","sha256":hash,"size_bytes":7},
                    {"path":"tokenizer.json","url":"https://huggingface.co/fixture/resolve/0000000000000000000000000000000000000000/tokenizer.json","sha256":hash,"size_bytes":7}
                ]
            }]}
        });
        let decoded: ModuleManifest = serde_json::from_value(manifest.clone()).unwrap();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let signature = minisign::sign(
            Some(&keys.pk),
            &keys.sk,
            Cursor::new(&bytes),
            Some("file:manifest.json"),
            None,
        )
        .unwrap()
        .to_string();
        let package = directory.path().join("fixture.cpmodule");
        let mut archive = zip::ZipWriter::new(fs::File::create(&package).unwrap());
        for (path, bytes) in [
            ("manifest.json", bytes.as_slice()),
            ("manifest.json.sig", signature.as_bytes()),
            (entry.as_str(), b"fixture"),
        ] {
            archive
                .start_file(path, zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
        let model = &decoded.search_provider.as_ref().unwrap().models[0];
        let assets = root
            .join("data/copypaste.fixture-search/models")
            .join(model.resource_directory_name());
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("model.onnx"), b"fixture").unwrap();
        fs::write(assets.join("tokenizer.json"), b"fixture").unwrap();
        let manager =
            Arc::new(ModuleManager::open(&root, "1.0.23", target, &keys.pk.to_base64()).unwrap());
        manager.install(&package).unwrap();
        manager
            .set_enabled("copypaste.fixture-search", true)
            .unwrap();
        let (launcher, launched, stopped, entered) = crate::inference::launcher_fixture();
        manager.set_inference_launcher(launcher);
        let keyring = copypaste_core::Keyring::from_secret(&[19; 32]);
        let database_path = directory.path().join("history.db");
        let database_key = *keyring.db_key();
        let store = Store::open(&database_path, &database_key).unwrap();
        if let Some(text) = text {
            let id = "fixture-item";
            let (nonce, ciphertext) =
                copypaste_core::encrypt(text.as_bytes(), &keyring.item_key(), id).unwrap();
            store
                .insert(copypaste_core::NewItem {
                    id: id.into(),
                    content_ciphertext: ciphertext,
                    nonce,
                    content_type: "text".into(),
                    content_hash: copypaste_core::compute_content_hash(text.as_bytes()),
                    search_text: Some(text.into()),
                    created_at: 1,
                    app_bundle_id: None,
                    app_name: None,
                    payload_metadata: None,
                })
                .unwrap();
        }
        Self {
            _directory: directory,
            manager,
            store,
            launched,
            stopped,
            entered,
            model_path: assets.join("model.onnx"),
            database_path,
            database_key,
        }
    }
    fn service(&self) -> Arc<SemanticSearch> {
        SemanticSearch::start(
            Arc::clone(&self.manager),
            &SearchBinding {
                store: self.store.clone(),
                changed: Arc::new(|| {}),
            },
        )
    }
}

#[test]
fn corrupted_model_is_not_reported_ready_and_keeps_lexical_search_available() {
    let fixture = Fixture::new(Some("lexical fixture"));
    // Keep the manifest-sized file so readiness must verify integrity rather
    // than treating a same-length corrupt model as usable.
    fs::write(&fixture.model_path, b"corrupt").unwrap();

    let module = fixture.manager.list().unwrap().remove(0);
    assert!(!module.search_models[0].available);
    assert!(fixture.manager.search_configuration().unwrap().is_none());

    let service = fixture.service();
    assert_eq!(
        service
            .query(&query("lexical"), None, 10, usize::MAX, None)
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(fixture.launched.load(Ordering::SeqCst), 0);
    service.shutdown();
}

fn query(text: &str) -> HistoryQuery {
    HistoryQuery {
        search: Some(text.into()),
        sort: copypaste_core::HistorySort::Relevance,
        ..Default::default()
    }
}

fn until(mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
}

fn until_for(limit: Duration, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < limit);
        thread::yield_now();
    }
}

#[test]
fn empty_history_does_not_load_a_model_to_mark_the_index_ready() {
    let fixture = Fixture::new(None);
    let service = fixture.service();
    until(|| service.ready.load(Ordering::Acquire));
    assert_eq!(fixture.launched.load(Ordering::SeqCst), 0);
    service.shutdown();
}

#[test]
fn history_notifications_only_schedule_changed_semantic_input() {
    let fixture = Fixture::new(Some("semantic fixture"));
    let service = fixture.service();
    until(|| service.ready.load(Ordering::Acquire));
    // Scheduling is checked independently of the worker consuming the revision.
    service.shutdown();
    let before = fixture.store.resource_revisions().unwrap();
    let scheduled = service.wake.0.lock().unwrap().background_generation;

    assert!(fixture.store.set_pinned("fixture-item", true).unwrap());
    assert_eq!(fixture.store.resource_revisions().unwrap(), before);
    service.history_changed();
    assert_eq!(
        service.wake.0.lock().unwrap().background_generation,
        scheduled
    );

    assert!(fixture.store.delete("fixture-item").unwrap());
    service.history_changed();
    assert_eq!(
        service.wake.0.lock().unwrap().background_generation,
        scheduled.wrapping_add(1)
    );
}

#[test]
fn external_store_changes_are_observed_by_maintenance_wake() {
    let fixture = Fixture::new(Some("external semantic fixture"));
    let service = fixture.service();
    until(|| service.ready.load(Ordering::Acquire));
    let other = Store::open(&fixture.database_path, &fixture.database_key).unwrap();
    assert!(other.delete("fixture-item").unwrap());
    let expected = SemanticRevision::from(other.resource_revisions().unwrap());

    until_for(Duration::from_secs(6), || {
        service
            .wake
            .0
            .lock()
            .unwrap()
            .observed_revision
            .as_ref()
            .is_some_and(|observed| {
                observed.generation == expected.generation && observed.semantic >= expected.semantic
            })
    });
    service.shutdown();
}

#[test]
fn background_notifications_preserve_fragment_yield() {
    let fixture = Fixture::new(None);
    let service = fixture.service();
    until(|| service.ready.load(Ordering::Acquire));
    let (background, urgent) = {
        let wake = service.wake.0.lock().unwrap();
        (wake.background_generation, wake.urgent_generation)
    };
    let notifier = Arc::clone(&service);
    let (sent, received) = std::sync::mpsc::channel();
    let signal = thread::spawn(move || {
        let mut wake = notifier.wake.0.lock().unwrap();
        wake.background_generation = wake.background_generation.wrapping_add(1);
        drop(wake);
        notifier.wake.1.notify_all();
        sent.send(()).unwrap();
    });
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    service.wait_for_wake(Duration::from_millis(15), true, background, urgent);
    assert!(started.elapsed() >= Duration::from_millis(13));
    signal.join().unwrap();
    service.shutdown();
}

#[test]
fn urgent_notification_preempts_fragment_yield() {
    let fixture = Fixture::new(None);
    let service = fixture.service();
    until(|| service.ready.load(Ordering::Acquire));
    let (background, urgent) = {
        let wake = service.wake.0.lock().unwrap();
        (wake.background_generation, wake.urgent_generation)
    };
    let notifier = Arc::clone(&service);
    let (sent, received) = std::sync::mpsc::channel();
    let signal = thread::spawn(move || {
        notifier.notify();
        sent.send(()).unwrap();
    });
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    service.wait_for_wake(Duration::from_millis(15), true, background, urgent);
    assert!(started.elapsed() < Duration::from_millis(5));
    signal.join().unwrap();
    service.shutdown();
}

#[test]
fn no_work_settles_only_the_examined_revision_and_never_an_error() {
    let examined = SemanticRevision {
        generation: "before".into(),
        semantic: 4,
    };
    let concurrent_write = SemanticRevision {
        generation: "after".into(),
        semantic: 5,
    };
    assert_eq!(
        settled_after_no_work(examined.clone(), Ok(())),
        Some(examined.clone())
    );
    assert_ne!(
        settled_after_no_work(examined.clone(), Ok(())),
        Some(concurrent_write)
    );
    assert_eq!(
        settled_after_no_work(examined, Err(StoreError::InvalidHistoryQuery)),
        None
    );
}

#[test]
fn lexical_results_remain_immediate_while_inference_is_blocked_and_disable_cancels_it() {
    let fixture = Fixture::new(Some("block lexical fixture"));
    let service = fixture.service();
    let started = Instant::now();
    assert_eq!(
        service
            .query(&query("block"), None, 10, usize::MAX, None)
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    fixture
        .entered
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    for _ in 0..3 {
        assert_eq!(
            service
                .query(&query("block"), None, 10, usize::MAX, None)
                .unwrap()
                .items
                .len(),
            1
        );
    }
    assert!(
        service.pending.lock().unwrap().queued.is_empty(),
        "active query must not be submitted twice"
    );
    fixture
        .manager
        .set_enabled("copypaste.fixture-search", false)
        .unwrap();
    assert_eq!(fixture.stopped.load(Ordering::SeqCst), 1);
    service.notify();
    until(|| {
        !service.ready.load(Ordering::Acquire) && service.pending.lock().unwrap().queued.is_empty()
    });
    assert!(service.queries.lock().unwrap().is_empty());
    service.shutdown();
}

#[test]
fn cached_query_vectors_survive_worker_exit_and_preserve_semantic_results() {
    let fixture = Fixture::new(Some("Apartment lease rent"));
    let service = fixture.service();
    let scope = fixture
        .manager
        .search_configuration()
        .unwrap()
        .unwrap()
        .scope;
    until(|| fixture.store.semantic_work(&scope).unwrap().is_none());
    let query = query("housing payment");
    assert!(service
        .query(&query, None, 10, usize::MAX, None)
        .unwrap()
        .items
        .is_empty());
    until(|| {
        service
            .queries
            .lock()
            .unwrap()
            .iter()
            .any(|entry| entry.text == "housing payment" && entry.vector.is_some())
    });
    assert_eq!(
        service
            .query(&query, None, 10, usize::MAX, None)
            .unwrap()
            .items
            .len(),
        1
    );
    fixture.manager.stop_inference();
    let launches = fixture.launched.load(Ordering::SeqCst);
    assert_eq!(
        service
            .query(&query, None, 10, usize::MAX, None)
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(fixture.launched.load(Ordering::SeqCst), launches);
    service.shutdown();
    assert!(service.queries.lock().unwrap().is_empty());
}
