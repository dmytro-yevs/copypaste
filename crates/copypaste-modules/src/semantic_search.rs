//! One optional background indexer and foreground query cache per host.

use crate::{manager::SearchConfiguration, ModuleError, ModuleManager};
use copypaste_core::{
    HistoryCursor, HistoryPage, HistoryQuery, SemanticMatches, Store, StoreError,
};
use copypaste_module_sdk::{validate_embeddings, ModuleOutput};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct CachedQuery {
    scope: String,
    text: String,
    vector: Option<Vec<f32>>,
    completed: Instant,
}

#[derive(Clone)]
struct PendingQuery {
    scope: String,
    text: String,
}
#[derive(Default)]
struct QueryQueue {
    queued: VecDeque<PendingQuery>,
    active: Option<PendingQuery>,
}
const QUERY_CAPACITY: usize = 32;
pub(crate) type ChangeCallback = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct SearchBinding {
    pub store: Store,
    pub changed: ChangeCallback,
}

pub(crate) struct SemanticSearch {
    store: Store,
    manager: Arc<ModuleManager>,
    changed: ChangeCallback,
    stopped: AtomicBool,
    ready: AtomicBool,
    wake: (Mutex<()>, Condvar),
    worker: Mutex<Option<JoinHandle<()>>>,
    queries: Mutex<VecDeque<CachedQuery>>,
    pending: Mutex<QueryQueue>,
}

impl SemanticSearch {
    pub fn start(manager: Arc<ModuleManager>, binding: &SearchBinding) -> Arc<Self> {
        let owner = Arc::new(Self {
            store: binding.store.clone(),
            manager,
            changed: Arc::clone(&binding.changed),
            stopped: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            wake: (Mutex::new(()), Condvar::new()),
            worker: Mutex::new(None),
            queries: Mutex::new(VecDeque::new()),
            pending: Mutex::new(QueryQueue::default()),
        });
        let service = Arc::clone(&owner);
        let worker = std::thread::Builder::new()
            .name("semantic-index".into())
            .spawn(move || service.index_loop());
        if let Ok(worker) = worker {
            *owner
                .worker
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(worker);
        }
        owner
    }

    pub fn notify(&self) {
        self.wake.1.notify_one();
    }

    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        self.wake.1.notify_all();
        self.manager.stop_inference();
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            let _ = worker.join();
        }
    }

    fn embeddings(
        &self,
        config: &SearchConfiguration,
        role: &str,
        text: &str,
    ) -> Result<Vec<Vec<f32>>, ModuleError> {
        let result = self.manager.invoke(
            &config.id,
            &config.command,
            BTreeMap::from([
                ("text".into(), serde_json::Value::String(text.into())),
                ("role".into(), serde_json::Value::String(role.into())),
            ]),
        )?;
        let ModuleOutput::Embeddings { model_id, vectors } = result else {
            return Err(ModuleError::Invalid(
                "The search module returned an invalid embedding.".into(),
            ));
        };
        if model_id != config.model.id || !validate_embeddings(&vectors, config.model.dimensions) {
            return Err(ModuleError::Invalid(
                "The search module returned an invalid embedding.".into(),
            ));
        }
        Ok(vectors)
    }

    pub fn query(
        &self,
        query: &HistoryQuery,
        after: Option<&HistoryCursor>,
        limit: u32,
        budget: usize,
        device: Option<&str>,
    ) -> Result<HistoryPage, StoreError> {
        let config = self.manager.search_configuration().ok().flatten();
        let Some(config) = config.filter(|_| {
            query
                .search
                .as_deref()
                .is_some_and(|text| text.chars().any(char::is_alphanumeric))
        }) else {
            return self
                .store
                .query_history_bounded_for_device(query, after, limit, budget, device);
        };
        let text = query
            .search
            .as_deref()
            .unwrap_or_default()
            .chars()
            .take(1024)
            .collect::<String>();
        let (vector, retry) = {
            let cache = self
                .queries
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match cache
                .iter()
                .find(|entry| entry.scope == config.scope && entry.text == text)
            {
                Some(entry) => (
                    entry.vector.clone(),
                    entry.vector.is_none() && entry.completed.elapsed() >= Duration::from_secs(5),
                ),
                None => (None, true),
            }
        };
        if retry {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if !pending
                .queued
                .iter()
                .any(|entry| entry.scope == config.scope && entry.text == text)
                && !pending
                    .active
                    .as_ref()
                    .is_some_and(|entry| entry.scope == config.scope && entry.text == text)
            {
                pending.queued.push_front(PendingQuery {
                    scope: config.scope.clone(),
                    text,
                });
                let capacity = QUERY_CAPACITY - usize::from(pending.active.is_some());
                pending.queued.truncate(capacity);
            }
            drop(pending);
            self.notify();
        }
        // Foreground reads never load a model or wait for inference. Cached
        // query vectors remain usable after the inference process has exited.
        let matches = vector
            .and_then(|vector| {
                self.store
                    .semantic_matches(&config.scope, &vector, config.model.minimum_similarity)
                    .ok()
            })
            .unwrap_or_else(|| SemanticMatches {
                scope: config.scope.clone(),
                scores: Vec::new(),
            });
        // Revalidate admission after inference. Disabled modules contribute no
        // late results, while ordinary lexical search remains available.
        self.manager
            .search_lease(&config, || {
                Ok(self.store.query_history_with_semantic(
                    query,
                    after,
                    limit,
                    budget,
                    device,
                    Some(&matches),
                ))
            })
            .unwrap_or_else(|_| {
                self.store
                    .query_history_bounded_for_device(query, after, limit, budget, device)
            })
    }

    fn index_loop(&self) {
        let mut current = String::new();
        let mut dirty = false;
        let mut last_prune = Instant::now();
        while !self.stopped.load(Ordering::Acquire) {
            let config = self.manager.search_configuration().ok().flatten();
            let mut did_work = false;
            if let Some(config) = config {
                if current != config.scope {
                    self.ready.store(false, Ordering::Release);
                    current = config.scope.clone();
                    self.queries
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .clear();
                    let _ = self.store.prune_semantic_index(&config.id, &config.scope);
                }
                let pending = {
                    let mut pending = self
                        .pending
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    pending.queued.retain(|entry| entry.scope == config.scope);
                    let next = pending.queued.pop_front();
                    pending.active = next.clone();
                    next
                };
                if let Some(query) = pending {
                    let vector = self
                        .embeddings(&config, "query", &query.text)
                        .ok()
                        .filter(|vectors| vectors.len() == 1)
                        .and_then(|mut vectors| vectors.pop());
                    let published = self
                        .manager
                        .search_lease(&config, || {
                            let mut cache = self
                                .queries
                                .lock()
                                .unwrap_or_else(|error| error.into_inner());
                            cache.retain(|entry| {
                                entry.scope != config.scope || entry.text != query.text
                            });
                            cache.push_back(CachedQuery {
                                scope: config.scope.clone(),
                                text: query.text,
                                vector,
                                completed: Instant::now(),
                            });
                            while cache.len() > QUERY_CAPACITY {
                                cache.pop_front();
                            }
                            Ok(())
                        })
                        .is_ok();
                    self.pending
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .active = None;
                    if published {
                        (self.changed)();
                    }
                    did_work = true;
                } else if let Ok(Some(work)) = self.store.semantic_work(&config.scope) {
                    let result =
                        self.embeddings(&config, "passage", &work.text)
                            .and_then(|vectors| {
                                self.manager.search_lease(&config, || {
                                    self.store
                                        .save_semantic_work(
                                            &config.id,
                                            &config.scope,
                                            &work,
                                            &vectors,
                                        )
                                        .map_err(|_| ModuleError::State)
                                })
                            });
                    if matches!(result, Ok(true)) {
                        did_work = true;
                        dirty = true;
                        if !self.ready.swap(true, Ordering::AcqRel) {
                            (self.changed)();
                            dirty = false;
                        }
                    }
                } else {
                    // Index coverage does not depend on a resident inference
                    // process. Empty/already indexed history requires no warm-up.
                    self.ready.store(true, Ordering::Release);
                }
                if !did_work {
                    if dirty {
                        (self.changed)();
                        dirty = false;
                    }
                    if last_prune.elapsed() >= Duration::from_secs(5) {
                        let _ = self.store.prune_semantic_index(&config.id, &config.scope);
                        last_prune = Instant::now();
                    }
                }
            } else {
                self.ready.store(false, Ordering::Release);
                current.clear();
                *self
                    .pending
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = QueryQueue::default();
                self.queries
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clear();
            }
            // Yield admission to foreground search between bounded fragments.
            if self
                .pending
                .lock()
                .is_ok_and(|pending| !pending.queued.is_empty())
            {
                continue;
            }
            if let Ok(lock) = self.wake.0.lock() {
                let _ = self.wake.1.wait_timeout(
                    lock,
                    if did_work {
                        Duration::from_millis(15)
                    } else {
                        Duration::from_secs(5)
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use copypaste_module_sdk::{ModuleManifest, ModuleTarget};
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Cursor, Write},
        path::PathBuf,
    };

    #[test]
    #[ignore = "requires explicitly staged native library, ONNX runtime, and both model profiles"]
    fn signed_semantic_package_indexes_real_models_and_respects_lifecycle() {
        let models =
            PathBuf::from(std::env::var("COPYPASTE_SEMANTIC_MODELS").expect("staged models"));
        let library =
            PathBuf::from(std::env::var("COPYPASTE_SEMANTIC_LIBRARY").expect("native module"));
        let runtime =
            PathBuf::from(std::env::var("COPYPASTE_SEMANTIC_RUNTIME").expect("native runtime"));
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        let source = root.join("modules/semantic-search");
        let target = ModuleTarget::current().expect("native shipped platform");
        let extension = if cfg!(windows) {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };
        let entry = format!("bin/module.{extension}");
        let runtime_name = if cfg!(windows) {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };
        let files = vec![
            (entry.clone(), library),
            (format!("bin/{runtime_name}"), runtime),
            (
                "assets/search-models.json".into(),
                source.join("assets/search-models.json"),
            ),
        ];
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(source.join("module.json")).unwrap()).unwrap();
        json["target"] = serde_json::to_value(target).unwrap();
        json["entrypoint"] = entry.into();
        json["files"] = serde_json::json!(files.iter().map(|(name, path)| {
            let bytes = fs::read(path).unwrap();
            serde_json::json!({"path":name,"size_bytes":bytes.len(),"sha256":hex::encode(Sha256::digest(&bytes))})
        }).collect::<Vec<_>>());
        let manifest: ModuleManifest = serde_json::from_value(json.clone()).unwrap();
        let provider = manifest.search_provider.as_ref().unwrap();
        let key = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let bytes = serde_json::to_vec(&json).unwrap();
        let signature = minisign::sign(
            Some(&key.pk),
            &key.sk,
            Cursor::new(&bytes),
            Some("file:manifest.json"),
            None,
        )
        .unwrap()
        .to_string();
        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("semantic.cpmodule");
        let mut archive = zip::ZipWriter::new(fs::File::create(&package).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in [
            ("manifest.json", bytes.as_slice()),
            ("manifest.json.sig", signature.as_bytes()),
        ] {
            archive.start_file(name, options).unwrap();
            archive.write_all(bytes).unwrap();
        }
        for (name, path) in files {
            archive.start_file(name, options).unwrap();
            std::io::copy(&mut fs::File::open(path).unwrap(), &mut archive).unwrap();
        }
        archive.finish().unwrap();
        let data = directory.path().join("host");
        let manager =
            Arc::new(ModuleManager::open(&data, "1.0.11", target, &key.pk.to_base64()).unwrap());
        manager.set_inference_launcher(Arc::new(crate::DesktopInferenceLauncher::new(
            PathBuf::from(
                std::env::var_os("COPYPASTE_INFERENCE_WORKER").expect("candidate inference worker"),
            ),
        )));
        assert!(!manager.install(&package).unwrap().enabled);
        for model in &provider.models {
            let destination =
                crate::resources::directory(&data.join("data").join(&manifest.id), model);
            fs::create_dir_all(&destination).unwrap();
            for file in &model.files {
                fs::copy(
                    models.join(&model.id).join(&file.path),
                    destination.join(&file.path),
                )
                .unwrap();
            }
        }
        let keys = copypaste_core::Keyring::from_secret(&[41; 32]);
        let store = Store::open(&directory.path().join("history.db"), &keys.db_key()).unwrap();
        let texts = [
            "Apartment lease rent for October",
            "Chocolate cake recipe with walnuts",
            "Оренда квартири за жовтень",
            "Рецепт шоколадного торта з горіхами",
        ];
        let mut ids = Vec::new();
        for (index, text) in texts.iter().enumerate() {
            let id = format!("fixture-{index}");
            let (nonce, ciphertext) =
                copypaste_core::encrypt(text.as_bytes(), &keys.item_key(), &id).unwrap();
            ids.push(
                store
                    .insert(copypaste_core::NewItem {
                        id,
                        content_ciphertext: ciphertext,
                        nonce,
                        content_type: "text".into(),
                        content_hash: copypaste_core::compute_content_hash(text.as_bytes()),
                        search_text: Some((*text).into()),
                        created_at: index as i64,
                        app_bundle_id: None,
                        app_name: None,
                        payload_metadata: None,
                    })
                    .unwrap()
                    .id,
            );
        }
        manager
            .set_preferences(
                &manifest.id,
                BTreeMap::from([("languages".into(), serde_json::json!(["en"]))]),
            )
            .unwrap();
        manager.set_enabled(&manifest.id, true).unwrap();
        let service = SemanticSearch::start(
            Arc::clone(&manager),
            &SearchBinding {
                store: store.clone(),
                changed: Arc::new(|| {}),
            },
        );
        for (languages, query, relevant, excluded) in [
            (
                vec!["en"],
                "paying for housing",
                vec![ids[0].clone()],
                vec![ids[1].clone()],
            ),
            (
                vec!["uk", "en"],
                "оплата житла",
                vec![ids[0].clone(), ids[2].clone()],
                vec![ids[1].clone(), ids[3].clone()],
            ),
        ] {
            manager
                .set_preferences(
                    &manifest.id,
                    BTreeMap::from([("languages".into(), serde_json::json!(languages))]),
                )
                .unwrap();
            let scope = manager.search_configuration().unwrap().unwrap().scope;
            service.notify();
            let started = Instant::now();
            while store.semantic_work(&scope).unwrap().is_some()
                || !service.ready.load(Ordering::Acquire)
            {
                assert!(
                    started.elapsed() < Duration::from_secs(20),
                    "native indexing must complete"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            let query = HistoryQuery {
                search: Some(query.into()),
                sort: copypaste_core::HistorySort::Relevance,
                ..Default::default()
            };
            assert!(store
                .search(query.search.as_deref().unwrap(), 10)
                .unwrap()
                .is_empty());
            let _ = service.query(&query, None, 10, usize::MAX, None).unwrap();
            let started = Instant::now();
            while !service.queries.lock().unwrap().iter().any(|entry| {
                entry.scope == scope
                    && Some(entry.text.as_str()) == query.search.as_deref()
                    && entry.vector.is_some()
            }) {
                assert!(
                    started.elapsed() < Duration::from_secs(20),
                    "native query embedding must complete"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            let page = service.query(&query, None, 10, usize::MAX, None).unwrap();
            let found: Vec<_> = page.items.iter().map(|item| item.id.clone()).collect();
            for id in relevant {
                assert!(
                    found.contains(&id),
                    "semantic result missing: {id}, got {found:?}"
                );
            }
            for id in excluded {
                assert!(!found.contains(&id), "unrelated result: {id}");
            }
        }
        manager.set_enabled(&manifest.id, false).unwrap();
        let query = HistoryQuery {
            search: Some("оплата житла".into()),
            sort: copypaste_core::HistorySort::Relevance,
            ..Default::default()
        };
        assert!(service
            .query(&query, None, 10, usize::MAX, None)
            .unwrap()
            .items
            .is_empty());
        manager.remove(&manifest.id).unwrap();
        store.clear_semantic_index(&manifest.id).unwrap();
        assert!(
            manager.list().unwrap().is_empty(),
            "worker-owned code must not require an application restart"
        );
        service.shutdown();
    }
}

#[cfg(test)]
#[path = "semantic_search_tests.rs"]
mod tests;
