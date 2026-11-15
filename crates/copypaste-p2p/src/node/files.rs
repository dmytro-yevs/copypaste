//! Explicit file sessions reuse pairing authentication and revocation leases.
use super::{channel::NoiseChannel, ActiveSyncSource, Node};
use crate::{
    peers::Peer,
    protocol::{SyncMessage, FILE_CHUNK_BYTES, FILE_RANGE_UNSUPPORTED, PROTOCOL_VERSION},
    sync::{SyncChannel, SyncError, SyncSource},
};
use sha2::{Digest, Sha256};

/// One member of a file offer, starting at a chunk boundary.
#[derive(Debug, Clone, Copy)]
pub struct FileRange<'a> {
    pub id: &'a str,
    /// The member of a multi-file clip; 0 for a single file.
    pub file: u32,
    pub sha256: &'a str,
    pub size: u64,
    /// The first chunk to receive; non-zero resumes an earlier transfer.
    pub start_index: u64,
}

impl Node {
    /// Only the explicit Download action calls this method. Caller owns output,
    /// size/hash verification and cancellation by dropping the future.
    pub async fn download_file(
        &self,
        peer: &Peer,
        id: &str,
        hash: &str,
        size: u64,
        receive: impl FnMut(&[u8]) -> Result<(), SyncError>,
    ) -> Result<String, SyncError> {
        let range = FileRange {
            id,
            file: 0,
            sha256: hash,
            size,
            start_index: 0,
        };
        self.download_file_range(peer, range, receive).await
    }

    /// Receive one member from `range.start_index`. A source that cannot
    /// resume fails with a source error carrying [`FILE_RANGE_UNSUPPORTED`];
    /// the caller then restarts from chunk 0.
    pub async fn download_file_range(
        &self,
        peer: &Peer,
        range: FileRange<'_>,
        mut receive: impl FnMut(&[u8]) -> Result<(), SyncError>,
    ) -> Result<String, SyncError> {
        let FileRange {
            id,
            file,
            sha256: hash,
            size,
            start_index,
        } = range;
        let _permit = self
            .file_transfers
            .try_acquire()
            .map_err(|_| SyncError::Source("Another file transfer is in progress.".into()))?;
        let peer = self
            .current_peer(peer)
            .ok_or_else(|| SyncError::Source("The source device is unavailable.".into()))?;
        let mut connected = None;
        for candidate in self.dial_candidates_with_sources(&peer, true) {
            if let Ok(session) = self.connect_peer(&candidate.endpoint, &peer).await {
                connected = Some(session);
                break;
            }
        }
        let (session, peer) = connected.ok_or_else(|| {
            SyncError::Source(
                "The source device is unavailable. Bring it online and try Download again.".into(),
            )
        })?;
        let active = self
            .begin_active_sync(&peer.pairing_id)
            .ok_or_else(|| SyncError::Source("The source device is unavailable.".into()))?;
        let cancel = active.cancelled();
        let operation = async {
            let mut channel = NoiseChannel::new(session);
            // FILE_CHUNK_BYTES bounds one storage read and one plaintext buffer;
            // it no longer causes a request/response round trip per chunk.
            channel
                .send(SyncMessage::FileRequest {
                    protocol_version: PROTOCOL_VERSION,
                    item_id: id.into(),
                    sha256: hash.into(),
                    index: start_index,
                    size,
                    file,
                })
                .await?;
            let chunk_count = size.div_ceil(FILE_CHUNK_BYTES as u64).max(1);
            for index in start_index..chunk_count {
                match channel.recv().await? {
                    SyncMessage::FileChunk {
                        index: received,
                        mut bytes,
                    } if received == index => {
                        let expected =
                            (size - index * FILE_CHUNK_BYTES as u64).min(FILE_CHUNK_BYTES as u64);
                        if bytes.len() as u64 != expected {
                            return Err(SyncError::Source(
                                "The file transfer is incomplete.".into(),
                            ));
                        }
                        let result = receive(&bytes);
                        zeroize::Zeroize::zeroize(&mut bytes);
                        result?;
                    }
                    SyncMessage::FileUnavailable { reason } => {
                        return Err(SyncError::Source(reason));
                    }
                    _ => return Err(SyncError::Source("The source file is unavailable.".into())),
                }
            }
            channel.send(SyncMessage::Done).await?;
            let checksum = if hash.is_empty() {
                match channel.recv().await? {
                    SyncMessage::FileChecksum { sha256 } => sha256,
                    SyncMessage::FileUnavailable { reason } => {
                        return Err(SyncError::Source(reason));
                    }
                    _ => {
                        return Err(SyncError::Source(
                            "The source file could not be verified.".into(),
                        ));
                    }
                }
            } else {
                hash.into()
            };
            channel.close().await;
            Ok(checksum)
        };
        tokio::select! { biased; _ = cancel.cancelled() => Err(SyncError::Source("The source device was unpaired or revoked.".into())), result = operation => result }
    }

    pub(super) async fn serve_file<S: SyncSource>(
        &self,
        pairing_id: &str,
        source: &S,
        channel: &mut NoiseChannel,
        request: SyncMessage,
    ) -> Result<(), SyncError> {
        let mut permit = match self.file_transfers.try_acquire() {
            Ok(permit) => Some(permit),
            Err(_) => {
                let reason = "Another file transfer is in progress. Wait for it to finish.";
                let _ = channel
                    .send(SyncMessage::FileUnavailable {
                        reason: reason.into(),
                    })
                    .await;
                return Err(SyncError::Source(reason.into()));
            }
        };
        let Some(active) = self.begin_active_sync(pairing_id) else {
            let reason = "The device is no longer paired.";
            let _ = channel
                .send(SyncMessage::FileUnavailable {
                    reason: reason.into(),
                })
                .await;
            return Err(SyncError::Source(reason.into()));
        };
        let source = ActiveSyncSource {
            source,
            state: std::sync::Arc::clone(&active.state),
        };
        let cancel = active.cancelled();
        let (first_id, first_file) = match &request {
            SyncMessage::FileRequest { item_id, file, .. } => (item_id.clone(), *file),
            _ => return Err(SyncError::Source("Invalid file request.".into())),
        };
        let peer = self
            .peers()
            .list()
            .into_iter()
            .find(|p| p.pairing_id == pairing_id);
        let peer_name = peer
            .as_ref()
            .map_or_else(|| "Paired device".into(), |p| p.name.clone());
        let peer_device_id = peer.as_ref().and_then(|p| p.device_id.clone());
        let mut observer = source.observe_member_transfer(&first_id, first_file, &peer_name);
        let operation = async {
            // The source checks this device's own policy; the peer's needs the
            // device id its sessions recorded.
            if let Some(device_id) = peer_device_id.as_deref() {
                if !source
                    .withheld_from(device_id, std::slice::from_ref(&first_id))?
                    .is_empty()
                {
                    return Err(SyncError::Source(
                        "File sync with this device is turned off.".into(),
                    ));
                }
            }
            let SyncMessage::FileRequest {
                item_id,
                sha256,
                index: start_index,
                size,
                file,
                ..
            } = request
            else {
                return Err(SyncError::Source("Invalid file request.".into()));
            };
            let mut deferred = source.deferred_member(&item_id, file)?;
            if deferred.is_some() && !sha256.is_empty() {
                return Err(SyncError::Source(
                    "The requested file version does not match the source.".into(),
                ));
            }
            if source.member_size(&item_id, file, &sha256)? != size {
                return Err(SyncError::Source(
                    "The requested file size does not match the source.".into(),
                ));
            }
            let mut digest = Sha256::new();
            let mut written = 0u64;
            let chunk_count = size.div_ceil(FILE_CHUNK_BYTES as u64).max(1);
            if start_index >= chunk_count {
                return Err(SyncError::Source(FILE_RANGE_UNSUPPORTED.into()));
            }
            // A deferred source is hashed from its first byte, so a resumed
            // stream still ends with the checksum of the whole file.
            if let Some(input) = deferred.as_mut() {
                for _ in 0..start_index {
                    let mut bytes = input.read_chunk()?;
                    digest.update(&bytes);
                    zeroize::Zeroize::zeroize(&mut bytes);
                }
            }
            written += start_index * FILE_CHUNK_BYTES as u64;
            // Backpressure from `channel.send` bounds the stream to one
            // FILE_CHUNK_BYTES buffer without a request per chunk.
            for index in start_index..chunk_count {
                let chunk = if let Some(input) = deferred.as_mut() {
                    input.read_chunk()
                } else {
                    source.member_chunk(&item_id, file, &sha256, index)
                };
                match chunk {
                    Ok(mut bytes) => {
                        let expected =
                            (size - index * FILE_CHUNK_BYTES as u64).min(FILE_CHUNK_BYTES as u64);
                        if bytes.len() as u64 != expected {
                            return Err(SyncError::Source("The source file is incomplete.".into()));
                        }
                        written += bytes.len() as u64;
                        if deferred.is_some() {
                            digest.update(&bytes);
                        }
                        let message = SyncMessage::FileChunk {
                            index,
                            bytes: std::mem::take(&mut bytes),
                        };
                        channel.send(message).await?;
                        if let Some(observer) = observer.as_mut() {
                            observer.progress(written);
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
            if !matches!(channel.recv().await?, SyncMessage::Done) {
                return Err(SyncError::Source(
                    "The file transfer was not confirmed.".into(),
                ));
            }
            let checksum = if let Some(mut input) = deferred.take() {
                input.verify()?;
                Some(format!("{:x}", digest.finalize()))
            } else {
                None
            };
            // All file I/O is done. Release admission before the terminal
            // reply so a new transfer does not wait for history bookkeeping.
            drop(permit.take());
            if let Some(sha256) = checksum {
                channel.send(SyncMessage::FileChecksum { sha256 }).await?;
            }
            Ok(())
        };
        let result = tokio::select! { biased; _ = cancel.cancelled() => Err(SyncError::Source("File transfer was cancelled.".into())), result = operation => result };
        drop(permit.take());
        if let Some(observer) = observer.as_mut() {
            observer.finish(result.as_ref().err().map(ToString::to_string));
        }
        if !cancel.is_cancelled() {
            if let Err(SyncError::Source(reason)) = &result {
                // Source errors are already safe to display; preserve their
                // actionable cause across the authenticated file session.
                let _ = channel
                    .send(SyncMessage::FileUnavailable {
                        reason: reason.clone(),
                    })
                    .await;
            }
        }
        result
    }
}
