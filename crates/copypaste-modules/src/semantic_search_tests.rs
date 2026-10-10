use super::*;
use copypaste_module_sdk::{ModuleArchitecture, ModuleManifest, ModulePlatform, ModuleTarget};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    sync::atomic::AtomicUsize,
};

struct Fixture {
    _directory: tempfile::TempDir,
    manager: Arc<ModuleManager>,
    store: Store,
    launched: Arc<AtomicUsize>,
    stopped: Arc<AtomicUsize>,
    entered: std::sync::mpsc::Receiver<()>,
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
            "schema_version":3,"api_version":1,"id":"copypaste.fixture-search","title":"Fixture","description":"Test embeddings.","version":"0.1.0","app_versions":">=1.0.0, <2.0.0",
            "supported_platforms":["macos","android","windows"],"target":target,"entrypoint":entry,"unload_policy":"process",
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
        let store = Store::open(&directory.path().join("history.db"), &keyring.db_key()).unwrap();
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

#[test]
fn empty_history_does_not_load_a_model_to_mark_the_index_ready() {
    let fixture = Fixture::new(None);
    let service = fixture.service();
    until(|| service.ready.load(Ordering::Acquire));
    assert_eq!(fixture.launched.load(Ordering::SeqCst), 0);
    service.shutdown();
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
}
