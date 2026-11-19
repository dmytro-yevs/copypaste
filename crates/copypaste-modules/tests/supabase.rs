//! Signed native package qualification against an isolated Supabase HTTP fixture.
use copypaste_core::{Keyring, NewItem, Store, StoreSource};
use copypaste_module_sdk::{ModuleArchitecture, ModuleOutput, ModulePlatform, ModuleTarget};
use copypaste_modules::{ModuleManager, SyncServices};
use copypaste_sync::{
    host::{UNREADABLE_UPLOADS, UPLOAD_FLOOR},
    store::StoreView,
};
use minisign::KeyPair;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, Request, Respond, ResponseTemplate,
};
use zip::{write::SimpleFileOptions, ZipWriter};

fn target() -> ModuleTarget {
    ModuleTarget::current().unwrap_or(ModuleTarget {
        platform: ModulePlatform::Macos,
        architecture: ModuleArchitecture::X86_64,
    })
}
#[path = "support/native_supabase.rs"]
mod native_supabase;
use native_supabase::library;

struct Device {
    group: Option<Arc<copypaste_p2p::Node>>,
    _directory: tempfile::TempDir,
    manager: Arc<ModuleManager>,
    store: Store,
    keyring: Arc<Keyring>,
    services: Arc<SyncServices>,
    enabled: Arc<AtomicBool>,
}
impl Device {
    fn new(endpoint: &str) -> Self {
        Self::with_group(endpoint, false)
    }
    fn with_group(endpoint: &str, grouped: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let secret: [u8; 32] = Sha256::digest(directory.path().to_string_lossy().as_bytes()).into();
        let keyring = Arc::new(Keyring::from_secret(&secret));
        let store = Store::open(&directory.path().join("history.db"), &keyring.db_key()).unwrap();
        let identity = store.device_identity("sync test").unwrap();
        let group = grouped.then(|| {
            Arc::new(copypaste_p2p::Node::new(
                copypaste_p2p::PeerStore::open(&directory.path().join("peers.json"), &secret)
                    .unwrap(),
                None,
                0,
                true,
            ))
        });
        if let Some(node) = &group {
            node.peers()
                .group_snapshot(
                    &identity.device_id,
                    "sync test",
                    copypaste_p2p::DeviceProfile::current(),
                    None,
                )
                .unwrap();
        }
        let group_id = identity.device_id.clone();
        let enabled = Arc::new(AtomicBool::new(true));
        let gate = Arc::clone(&enabled);
        let source = StoreSource::new(
            store.clone(),
            Arc::clone(&keyring),
            identity.device_id.clone(),
            identity.device_name,
            Default::default(),
        );
        let group_gate = group.clone();
        let mut services = SyncServices::new(
            store.clone(),
            StoreView::new(source, identity.device_id),
            move || {
                gate.load(Ordering::Acquire)
                    && group_gate
                        .as_ref()
                        .is_none_or(|node| node.peers().group_active())
            },
            |_| {},
        );
        if let Some(node) = &group {
            let export = Arc::clone(node);
            let import = Arc::clone(node);
            let revision = Arc::clone(node);
            services = services.with_group(
                move || {
                    export
                        .peers()
                        .group_snapshot(
                            &group_id,
                            "sync test",
                            copypaste_p2p::DeviceProfile::current(),
                            None,
                        )
                        .and_then(|snapshot| snapshot.encode())
                        .map(Some)
                        .map_err(|error| error.to_string())
                },
                move |id, json| match import.import_cloud_group(id, json) {
                    Ok(_) | Err(copypaste_p2p::group::GroupImportError::Rejected) => Ok(()),
                    Err(error) => Err(error.to_string()),
                },
                move || revision.peers().group_revision(),
            );
        }
        let services = Arc::new(services);
        let signer = KeyPair::generate_unencrypted_keypair().unwrap();
        let manager = Arc::new(
            ModuleManager::open(
                &directory.path().join("modules"),
                env!("CARGO_PKG_VERSION"),
                target(),
                &signer.pk.to_base64(),
            )
            .unwrap(),
        );
        manager.set_sync_services(Arc::clone(&services)).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(root.join("modules/supabase/module.json")).unwrap())
                .unwrap();
        let library = fs::read(library()).unwrap();
        let entrypoint = format!(
            "bin/module.{}",
            if cfg!(target_os = "windows") {
                "dll"
            } else if cfg!(target_os = "macos") {
                "dylib"
            } else {
                "so"
            }
        );
        manifest["target"] = serde_json::to_value(target()).unwrap();
        manifest["entrypoint"] = json!(entrypoint);
        manifest["files"] = json!([{"path":entrypoint, "sha256":hex::encode(Sha256::digest(&library)),"size_bytes":library.len()}]);
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let signature = minisign::sign(
            Some(&signer.pk),
            &signer.sk,
            Cursor::new(&bytes),
            Some("file:manifest.json"),
            None,
        )
        .unwrap()
        .to_string();
        let package = directory.path().join("supabase.cpmodule");
        let mut zip = ZipWriter::new(fs::File::create(&package).unwrap());
        for (name, bytes) in [
            ("manifest.json", bytes.as_slice()),
            ("manifest.json.sig", signature.as_bytes()),
            (entrypoint.as_str(), library.as_slice()),
        ] {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        let installed = manager.install(&package).unwrap();
        assert!(
            !installed.enabled,
            "a network provider requires explicit activation"
        );
        assert!(!installed.commands.iter().any(|c| c.id == "provider-tick"));
        manager
            .set_preferences(
                "copypaste.supabase",
                BTreeMap::from([
                    ("url".into(), json!(endpoint)),
                    ("anon_key".into(), json!("test-publishable-key")),
                ]),
            )
            .unwrap();
        manager
            .set_enabled("copypaste.supabase", true, &[])
            .unwrap();
        Self {
            group,
            _directory: directory,
            manager,
            store,
            keyring,
            services,
            enabled,
        }
    }
    fn invoke(&self, command: &str) -> ModuleOutput {
        let args = if command == "sign-in" {
            BTreeMap::from([
                ("email".into(), json!("module@example.com")),
                ("password".into(), json!("account-password")),
                ("passphrase".into(), json!("shared sync passphrase")),
            ])
        } else {
            BTreeMap::new()
        };
        self.manager
            .invoke("copypaste.supabase", command, args)
            .unwrap()
    }
    fn put(&self, id: &str, bytes: &[u8], content_type: &str, stamp: i64) {
        let (nonce, encrypted) = if copypaste_ipc::content_type::is_binary(content_type) {
            (
                Vec::new(),
                copypaste_core::seal_binary(bytes, &self.keyring.item_key(), id).unwrap(),
            )
        } else {
            copypaste_core::encrypt(bytes, &self.keyring.item_key(), id).unwrap()
        };
        self.store
            .insert(NewItem {
                id: id.into(),
                content_ciphertext: encrypted,
                nonce,
                content_type: content_type.into(),
                content_hash: copypaste_core::compute_content_hash(bytes),
                search_text: None,
                created_at: stamp,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        self.services
            .note_version("copypaste.supabase", stamp)
            .unwrap();
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        let _ = self.manager.set_enabled("copypaste.supabase", false, &[]);
    }
}
/// The clipboard table as PostgREST serves it: a write places its rows at the
/// backend's own clock, and a read honours the id filter, the keyset, the
/// order, the limit and the column list.
#[derive(Clone, Default)]
struct Rows(Arc<Mutex<BTreeMap<String, Value>>>, Arc<AtomicU64>);
impl Rows {
    /// The next `updated_at` as Postgres spells a `timestamptz` in JSON, at
    /// least a microsecond after the previous write.
    fn placed(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let at = now.max(self.1.load(Ordering::Acquire) + 1);
        self.1.store(at, Ordering::Release);
        let (seconds, micros) = (at / 1_000_000, at % 1_000_000);
        // Days since the epoch to a civil date, shifted to years starting in March.
        let days = seconds / 86_400 + 719_468;
        let day_of_era = days % 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        let year = year_of_era + days / 146_097 * 400 + u64::from(month <= 2);
        let second = seconds % 86_400;
        format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{micros:06}+00:00",
            second / 3_600,
            second % 3_600 / 60,
            second % 60
        )
    }
}
impl Respond for Rows {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut rows = self.0.lock().unwrap();
        if request.method.as_str() == "POST" {
            let placed = self.placed();
            let items: Vec<Value> = serde_json::from_slice(&request.body).unwrap();
            for mut item in items {
                item["updated_at"] = json!(placed);
                rows.insert(item["item_id"].as_str().unwrap().into(), item);
            }
            return ResponseTemplate::new(201);
        }
        let query: BTreeMap<String, String> = request.url.query_pairs().into_owned().collect();
        // Positions share one layout and zone, so they order as text up to the offset.
        let instant = |text: &str| text.get(..26).unwrap_or(text).to_owned();
        let key = |row: &Value| {
            (
                instant(row["updated_at"].as_str().unwrap()),
                row["item_id"].as_str().unwrap().to_owned(),
            )
        };
        let mut items: Vec<Value> = rows.values().cloned().collect();
        if let Some(ids) = query
            .get("item_id")
            .and_then(|ids| ids.strip_prefix("in.(")?.strip_suffix(')'))
        {
            items.retain(|row| ids.split(',').any(|id| row["item_id"] == id));
        }
        if let Some(from) = query
            .get("updated_at")
            .and_then(|from| from.strip_prefix("gte."))
        {
            items.retain(|row| key(row).0 >= instant(from));
        }
        if let Some(after) = query.get("or").and_then(|after| {
            let (position, rest) = after.strip_prefix("(updated_at.gt.")?.split_once(",and(")?;
            let item_id = rest.rsplit_once(",item_id.gt.")?.1.strip_suffix("))")?;
            Some((instant(position), item_id.to_owned()))
        }) {
            items.retain(|row| key(row) > after);
        }
        items.sort_by_key(key);
        if let Some(limit) = query.get("limit").and_then(|limit| limit.parse().ok()) {
            items.truncate(limit);
        }
        if let Some(columns) = query.get("select") {
            for item in &mut items {
                item.as_object_mut()
                    .unwrap()
                    .retain(|column, _| columns.split(',').any(|name| name == column));
            }
        }
        ResponseTemplate::new(200).set_body_json(items)
    }
}
async fn backend(delay: Option<Duration>) -> (MockServer, Rows) {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/auth/v1/token")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token":"test-access-token","refresh_token":"test-refresh-token","expires_in":3600,"user":{"id":"test-user"}}))).mount(&server).await;
    let rows = Rows::default();
    Mock::given(method("POST"))
        .and(path("/rest/v1/clipboard_items"))
        .respond_with(rows.clone())
        .mount(&server)
        .await;
    if let Some(delay) = delay {
        Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([]))
                    .set_delay(delay),
            )
            .mount(&server)
            .await;
    } else {
        Mock::given(method("GET"))
            .and(path("/rest/v1/clipboard_items"))
            .respond_with(rows.clone())
            .mount(&server)
            .await;
    }
    (server, rows)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn signed_native_provider_synchronizes_the_group_through_host_callbacks_without_lan() {
    #[derive(Clone, Default)]
    struct GroupRows(Arc<Mutex<BTreeMap<String, Value>>>);
    impl Respond for GroupRows {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let mut rows = self.0.lock().unwrap();
            if request.method.as_str() == "POST" {
                let row: Value = serde_json::from_slice(&request.body).unwrap();
                rows.insert(row["device_id"].as_str().unwrap().into(), row);
                ResponseTemplate::new(201)
            } else {
                let catalog = request
                    .url
                    .query_pairs()
                    .any(|(key, value)| key == "select" && value == "device_id,ciphertext_digest");
                let filter = request
                    .url
                    .query_pairs()
                    .find(|(key, _)| key == "device_id")
                    .map(|(_, value)| value.into_owned());
                let ids = filter
                    .as_deref()
                    .and_then(|value| value.strip_prefix("in.("))
                    .and_then(|value| value.strip_suffix(')'));
                let page: Vec<Value> = rows
                    .values()
                    .filter(|row| {
                        ids.is_none_or(|ids| ids.split(',').any(|id| row["device_id"] == id))
                    })
                    .map(|row| {
                        if catalog {
                            {
                                let mut hash = Sha256::new();
                                hash.update(row["nonce"].as_str().unwrap().as_bytes());
                                hash.update(row["ciphertext"].as_str().unwrap().as_bytes());
                                json!({"device_id":row["device_id"],"ciphertext_digest":hex::encode(hash.finalize())})
                            }
                        } else {
                            row.clone()
                        }
                    })
                    .collect();
                ResponseTemplate::new(200).set_body_json(page)
            }
        }
    }
    let (server, clipboard) = backend(None).await;
    let groups = GroupRows::default();
    Mock::given(path("/rest/v1/device_group_states"))
        .respond_with(groups.clone())
        .mount(&server)
        .await;
    let a = Device::with_group(&server.uri(), true);
    let b = Device::with_group(&server.uri(), true);
    let c = Device::with_group(&server.uri(), true);
    for device in [&a, &b, &c] {
        device.invoke("sign-in");
        device.invoke("sync-now");
    }
    for device in [&a, &b, &c] {
        device.invoke("sync-now");
    }
    for device in [&a, &b, &c] {
        assert_eq!(device.group.as_ref().unwrap().peers().len(), 2);
        assert_eq!(
            device.store.count().unwrap(),
            0,
            "membership must not appear in clipboard history"
        );
    }
    assert_eq!(groups.0.lock().unwrap().len(), 3);
    assert!(clipboard.0.lock().unwrap().is_empty());
    a.put(
        "group-text",
        b"group history still works",
        "text",
        copypaste_core::now_ms() - 1000,
    );
    a.invoke("sync-now");
    c.invoke("sync-now");
    assert!(c.store.get("group-text").unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn signed_native_provider_converges_and_preserves_state_through_disable_and_removal() {
    let (server, rows) = backend(None).await;
    let a = Device::new(&server.uri());
    let b = Device::new(&server.uri());
    a.put(
        "text-item",
        b"only encrypted clipboard content",
        "text",
        copypaste_core::now_ms() - 1000,
    );
    let image = include_bytes!("../../../scripts/modules/fixtures/english.png");
    a.put(
        "image-item",
        image,
        "image/png",
        copypaste_core::now_ms() - 900,
    );
    a.invoke("sign-in");
    a.invoke("sync-now");
    b.invoke("sign-in");
    b.invoke("sync-now");
    let source = StoreSource::new(
        b.store.clone(),
        Arc::clone(&b.keyring),
        "receiver".into(),
        "receiver".into(),
        Default::default(),
    );
    assert_eq!(
        source
            .open_bytes(&b.store.version("text-item").unwrap().unwrap())
            .unwrap()
            .as_slice(),
        b"only encrypted clipboard content"
    );
    assert_eq!(
        source
            .open_bytes(&b.store.get("image-item").unwrap().unwrap())
            .unwrap()
            .as_slice(),
        image
    );
    assert!(!serde_json::to_string(&*rows.0.lock().unwrap())
        .unwrap()
        .contains("only encrypted clipboard content"));
    b.put(
        "from-b",
        b"AKIAIOSFODNN7EXAMPLE",
        "text",
        copypaste_core::now_ms() - 500,
    );
    b.invoke("sync-now");
    a.invoke("sync-now");
    assert!(
        a.store.get("from-b").unwrap().is_some(),
        "both directions converge"
    );
    let ModuleOutput::Data { data } = a.invoke("sync-now") else {
        panic!("sync counters")
    };
    assert_eq!(data["sync"]["uploaded"], 0);
    assert_eq!(data["sync"]["applied"], 0);
    a.store.delete("text-item").unwrap();
    let stamp = a.store.version("text-item").unwrap().unwrap().created_at;
    a.services
        .note_version("copypaste.supabase", stamp)
        .unwrap();
    a.invoke("sync-now");
    b.invoke("sync-now");
    assert!(
        b.store.version("text-item").unwrap().unwrap().deleted,
        "tombstones propagate"
    );
    b.invoke("sync-now");
    assert!(
        b.store.version("text-item").unwrap().unwrap().deleted,
        "replay cannot resurrect a deletion"
    );
    assert!(!serde_json::to_string(&*rows.0.lock().unwrap())
        .unwrap()
        .contains("AKIAIOSFODNN7EXAMPLE"));
    a.manager
        .set_enabled("copypaste.supabase", false, &[])
        .unwrap();
    assert!(a
        .manager
        .invoke("copypaste.supabase", "sync-now", BTreeMap::new())
        .is_err());
    assert!(a
        .store
        .state("module:copypaste.supabase:cloud_refresh_token")
        .unwrap()
        .is_some());
    a.manager
        .set_enabled("copypaste.supabase", true, &[])
        .unwrap();
    let ModuleOutput::Data { data } = a.invoke("status") else {
        panic!("typed account status")
    };
    assert_eq!(data["status"]["signed_in"], true);
    a.manager.remove("copypaste.supabase").unwrap();
    assert!(a
        .store
        .state("module:copypaste.supabase:cloud_refresh_token")
        .unwrap()
        .is_none());
    assert!(a
        .store
        .state(&format!("module:copypaste.supabase:{UPLOAD_FLOOR}"))
        .unwrap()
        .is_none());
    assert!(a
        .store
        .state(&format!("module:copypaste.supabase:{UNREADABLE_UPLOADS}"))
        .unwrap()
        .is_none());
    assert_eq!(
        a.store.count().unwrap(),
        2,
        "removal preserves local history"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disabling_an_active_request_cancels_native_work_before_acknowledgement() {
    let (server, _) = backend(Some(Duration::from_secs(30))).await;
    let device = Device::new(&server.uri());
    device.invoke("sign-in");
    let manager = Arc::clone(&device.manager);
    let explicit = std::thread::spawn(move || {
        manager.invoke("copypaste.supabase", "sync-now", BTreeMap::new())
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .any(|r| r.method.as_str() == "GET")
    {
        assert!(
            Instant::now() < deadline,
            "an actual data request must be in flight"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let started = Instant::now();
    device
        .manager
        .set_enabled("copypaste.supabase", false, &[])
        .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "disable must cancel the network request"
    );
    assert!(explicit.join().unwrap().is_err());
    device.enabled.store(false, Ordering::Release);
    assert!(device
        .store
        .state("module:copypaste.supabase:cloud_last_sync_ms")
        .unwrap()
        .is_none());
}
