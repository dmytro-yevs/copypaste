//! Encrypted file objects: uploads of this device's kept copies, fetches the
//! host asked for, and deletion of objects whose item is gone or past the
//! row TTL. Bytes are sealed here; the host only ever sees plaintext of its
//! own files.
use crate::host::HostStore;
use copypaste_clock::now_ms;
use copypaste_cloud::credentials::CredentialState;
use copypaste_cloud::crypto::file::{FileKeys, FileManifest, OBJECT_PLAINTEXT_BYTES};
use copypaste_cloud::storage::{object_path, StorageError, SupabaseStorage};
use copypaste_sync::host::{CloudFileFetch, CloudFileUpload, FileBytes, SyncHostRequest};
use serde_json::{json, Value};

/// Objects follow the cloud row TTL.
const TTL_MS: i64 = 24 * 60 * 60 * 1000;
/// The bucket policy refuses an account's 513th object.
const MAX_OBJECTS: u64 = 512;
/// Bounds the plaintext a round uploads.
const MAX_ROUND_BYTES: u64 = 64 * 1024 * 1024;
/// One host read and one host delivery.
const HOST_PIECE: usize = 1024 * 1024;
const LEDGER_KEY: &str = "cloud_file_objects";

/// What one account needs to move file objects.
pub struct FileStore {
    pub storage: SupabaseStorage,
}

/// Keys of the current space epoch. They change after a rotation, so every
/// round derives them anew and never caches them.
pub struct EpochKeys {
    pub epoch: u64,
    pub keys: FileKeys,
}

/// One uploaded file. The object id is kept because the key that named it
/// may no longer be derivable after a rotation.
struct Entry {
    item: String,
    file: u32,
    objects: u64,
    at: i64,
    id: Option<String>,
    epoch: u64,
}

struct Ledger(Vec<Entry>);

impl Ledger {
    fn load(host: &HostStore) -> Self {
        let entries = host
            .state(LEDGER_KEY)
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<Vec<Value>>(&json).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                Some(Entry {
                    item: entry.get("item")?.as_str()?.to_owned(),
                    file: u32::try_from(entry.get("file")?.as_u64()?).ok()?,
                    objects: entry.get("objects")?.as_u64()?,
                    at: entry.get("at")?.as_i64()?,
                    id: entry.get("id").and_then(Value::as_str).map(str::to_owned),
                    epoch: entry.get("epoch").and_then(Value::as_u64).unwrap_or(0),
                })
            })
            .collect();
        Self(entries)
    }
    fn save(&self, host: &HostStore) {
        let json = Value::Array(
            self.0
                .iter()
                .map(|entry| {
                    json!({
                        "item": entry.item,
                        "file": entry.file,
                        "objects": entry.objects,
                        "at": entry.at,
                        "id": entry.id,
                        "epoch": entry.epoch,
                    })
                })
                .collect(),
        )
        .to_string();
        let _ = host.set_state_all(&[(LEDGER_KEY, &json)]);
    }
    fn objects(&self) -> u64 {
        self.0.iter().map(|entry| entry.objects).sum()
    }
}

fn paths(user: &str, id: &str, objects: u64) -> Vec<String> {
    (0..objects)
        .map(|index| object_path(user, id, index))
        .collect()
}

/// The stored id, or for entries written before ids were kept, the id under
/// keys of the same epoch.
fn entry_id(entry: &Entry, keys: Option<&EpochKeys>) -> Option<String> {
    entry.id.clone().or_else(|| {
        keys.filter(|keys| keys.epoch == entry.epoch)
            .map(|keys| keys.keys.object_id(&entry.item, entry.file))
    })
}

impl FileStore {
    /// One file pass after a successful row round: fetches first (someone is
    /// waiting), then deletions, then uploads.
    pub async fn round(&self, host: &HostStore, user: &str, token: &str, keys: &EpochKeys) {
        if let Ok(fetches) = host
            .0
            .request::<_, Vec<CloudFileFetch>>(&SyncHostRequest::CloudFileFetches)
        {
            for fetch in fetches {
                let (sha256, error) = match self.fetch(host, user, token, keys, &fetch).await {
                    Ok(sha256) => (Some(sha256), None),
                    Err(error) => (None, Some(error)),
                };
                let _ = host.0.request::<_, ()>(&SyncHostRequest::CloudFileDone {
                    item_id: fetch.item_id,
                    file_index: fetch.file_index,
                    sha256,
                    error,
                });
            }
        }
        let mut ledger = Ledger::load(host);
        let before = ledger.0.len();
        let removed = self.sweep(host, user, token, keys, &mut ledger).await;
        let swept = ledger.0.len() != before;
        let (uploaded, failed) = self.upload(host, user, token, keys, &mut ledger).await;
        if swept || uploaded > 0 {
            ledger.save(host);
        }
        if uploaded > 0 || failed > 0 || removed > 0 {
            let _ = host
                .0
                .request::<_, ()>(&SyncHostRequest::FileRoundFinished {
                    uploaded,
                    failed,
                    removed,
                });
        }
    }

    /// Deletes expired and gone objects, and objects of an older epoch, which
    /// members on the current key cannot read; those are uploaded again.
    async fn sweep(
        &self,
        host: &HostStore,
        user: &str,
        token: &str,
        keys: &EpochKeys,
        ledger: &mut Ledger,
    ) -> u32 {
        if ledger.0.is_empty() {
            return 0;
        }
        let mut removed = 0;
        let gone: Vec<String> = host
            .0
            .request(&SyncHostRequest::GoneFiles {
                item_ids: ledger.0.iter().map(|entry| entry.item.clone()).collect(),
            })
            .unwrap_or_default();
        let expired = now_ms().saturating_sub(TTL_MS);
        let mut kept = Vec::with_capacity(ledger.0.len());
        for entry in std::mem::take(&mut ledger.0) {
            let stale =
                entry.at < expired || gone.contains(&entry.item) || entry.epoch != keys.epoch;
            if stale {
                let Some(id) = entry_id(&entry, Some(keys)) else {
                    // An entry from before ids were kept, named by a key that
                    // is gone: it can no longer be addressed.
                    continue;
                };
                if self
                    .storage
                    .delete(token, &paths(user, &id, entry.objects))
                    .await
                    .is_ok()
                {
                    removed += 1;
                    continue;
                }
            }
            kept.push(entry);
        }
        ledger.0 = kept;
        removed
    }

    async fn upload(
        &self,
        host: &HostStore,
        user: &str,
        token: &str,
        keys: &EpochKeys,
        ledger: &mut Ledger,
    ) -> (u32, u32) {
        let Ok(candidates) =
            host.0
                .request::<_, Vec<CloudFileUpload>>(&SyncHostRequest::CloudFileUploads {
                    since_ms: now_ms().saturating_sub(TTL_MS),
                    limit: 16,
                })
        else {
            return (0, 0);
        };
        let mut budget = MAX_ROUND_BYTES;
        let (mut uploaded, mut failed) = (0, 0);
        for file in candidates {
            let manifest = FileManifest {
                size_bytes: file.size_bytes,
                sha256: file.sha256.clone(),
            };
            let objects = manifest.chunk_count() + 1;
            if ledger
                .0
                .iter()
                .any(|entry| entry.item == file.item_id && entry.file == file.file_index)
                || ledger.objects() + objects > MAX_OBJECTS
                || file.size_bytes > budget
            {
                continue;
            }
            budget -= file.size_bytes;
            let id = keys.keys.object_id(&file.item_id, file.file_index);
            match self
                .upload_one(
                    host,
                    user,
                    token,
                    &keys.keys,
                    (&file.item_id, file.file_index),
                    &id,
                    &manifest,
                )
                .await
            {
                Ok(()) => {
                    ledger.0.push(Entry {
                        item: file.item_id,
                        file: file.file_index,
                        objects,
                        at: now_ms(),
                        id: Some(id),
                        epoch: keys.epoch,
                    });
                    uploaded += 1;
                }
                Err(StorageError::Unauthorized) => {
                    failed += 1;
                    break;
                }
                Err(_) => {
                    failed += 1;
                    // A partial upload never counts: the manifest goes last.
                    let _ = self.storage.delete(token, &paths(user, &id, objects)).await;
                }
            }
        }
        (uploaded, failed)
    }

    async fn upload_one(
        &self,
        host: &HostStore,
        user: &str,
        token: &str,
        keys: &FileKeys,
        (item, file): (&str, u32),
        id: &str,
        manifest: &FileManifest,
    ) -> Result<(), StorageError> {
        for index in 1..=manifest.chunk_count() {
            let offset = (index - 1) * OBJECT_PLAINTEXT_BYTES as u64;
            let len = (manifest.size_bytes - offset).min(OBJECT_PLAINTEXT_BYTES as u64) as usize;
            let mut plaintext = zeroize::Zeroizing::new(Vec::with_capacity(len));
            while plaintext.len() < len {
                let piece: FileBytes = host
                    .0
                    .request(&SyncHostRequest::ReadCloudFile {
                        item_id: item.into(),
                        file_index: file,
                        offset: offset + plaintext.len() as u64,
                        len: (len - plaintext.len()).min(HOST_PIECE) as u32,
                    })
                    .map_err(|_| StorageError::NotFound)?;
                let piece = zeroize::Zeroizing::new(piece.0);
                if piece.is_empty() {
                    return Err(StorageError::NotFound);
                }
                plaintext.extend_from_slice(&piece);
            }
            let sealed = keys
                .seal_chunk(id, manifest, index, &plaintext)
                .map_err(|_| StorageError::NotFound)?;
            // The upload can take minutes; only the sealed object waits for it.
            drop(plaintext);
            self.storage
                .upload(token, &object_path(user, id, index), sealed)
                .await?;
        }
        let sealed = keys
            .seal_manifest(id, manifest)
            .map_err(|_| StorageError::NotFound)?;
        self.storage
            .upload(token, &object_path(user, id, 0), sealed)
            .await
    }

    async fn fetch(
        &self,
        host: &HostStore,
        user: &str,
        token: &str,
        keys: &EpochKeys,
        fetch: &CloudFileFetch,
    ) -> Result<String, String> {
        let keys = &keys.keys;
        let id = keys.object_id(&fetch.item_id, fetch.file_index);
        let missing = |_| "The file is not in cloud storage yet.".to_string();
        let unverified = |_| "The cloud copy could not be verified.".to_string();
        let manifest_object = self
            .storage
            .download(token, &object_path(user, &id, 0))
            .await
            .map_err(missing)?;
        let manifest = keys
            .open_manifest(&id, &manifest_object)
            .map_err(unverified)?;
        if manifest.size_bytes != fetch.size_bytes {
            return Err("The cloud copy does not match this file.".into());
        }
        for index in 1..=manifest.chunk_count() {
            let object = self
                .storage
                .download(token, &object_path(user, &id, index))
                .await
                .map_err(missing)?;
            let plaintext = keys
                .open_chunk(&id, &manifest, index, object)
                .map_err(unverified)?;
            for piece in plaintext.chunks(HOST_PIECE) {
                host.0
                    .request::<_, ()>(&SyncHostRequest::CloudFileChunk {
                        item_id: fetch.item_id.clone(),
                        file_index: fetch.file_index,
                        chunk: FileBytes(piece.to_vec()),
                    })
                    .map_err(|_| "The download stopped.".to_string())?;
            }
        }
        Ok(manifest.sha256)
    }

    /// Removes every uploaded object, for sign-out.
    pub async fn delete_all(
        &self,
        host: &HostStore,
        user: &str,
        token: &str,
        keys: Option<&EpochKeys>,
    ) {
        let ledger = Ledger::load(host);
        for entry in &ledger.0 {
            if let Some(id) = entry_id(entry, keys) {
                let _ = self
                    .storage
                    .delete(token, &paths(user, &id, entry.objects))
                    .await;
            }
        }
        Ledger(Vec::new()).save(host);
    }
}

/// Whether the host waits for a file, so the provider should run a round.
pub fn fetch_pending(host: &HostStore) -> bool {
    host.0
        .request::<_, Vec<CloudFileFetch>>(&SyncHostRequest::CloudFileFetches)
        .is_ok_and(|fetches| !fetches.is_empty())
}
