//! Explicit file sessions reuse pairing authentication and revocation leases.
use super::{channel::NoiseChannel, ActiveSyncSource, Node};
use crate::{
    peers::Peer,
    protocol::{SyncMessage, FILE_CHUNK_BYTES, PROTOCOL_VERSION},
    sync::{SyncChannel, SyncError, SyncSource},
    transport::Session,
};
use sha2::{Digest, Sha256};

impl Node {
    /// Only the explicit Download action calls this method. Caller owns output,
    /// size/hash verification and cancellation by dropping the future.
    pub async fn download_file(
        &self,
        peer: &Peer,
        id: &str,
        hash: &str,
        size: u64,
        mut receive: impl FnMut(&[u8]) -> Result<(), SyncError>,
    ) -> Result<String, SyncError> {
        let _permit = self
            .file_transfers
            .try_acquire()
            .map_err(|_| SyncError::Source("Another file transfer is in progress.".into()))?;
        let active = self
            .begin_active_sync(&peer.pairing_id)
            .ok_or_else(|| SyncError::Source("The source device is unavailable.".into()))?;
        let cancel = active.cancelled();
        let operation = async {
            let mut connected = None;
            for candidate in self.dial_candidates_with_sources(peer, true) {
                if let Ok(session) = Session::connect(candidate.addr, &peer.psk).await {
                    connected = Some(session);
                    break;
                }
            }
            let session = connected.ok_or_else(|| {
                SyncError::Source(
                    "The source device is unavailable. Bring it online and try Download again."
                        .into(),
                )
            })?;
            let mut channel = NoiseChannel::new(session);
            for index in 0..size.div_ceil(FILE_CHUNK_BYTES as u64).max(1) {
                channel
                    .send(SyncMessage::FileRequest {
                        protocol_version: PROTOCOL_VERSION,
                        item_id: id.into(),
                        sha256: hash.into(),
                        index,
                    })
                    .await?;
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
                        return Err(SyncError::Source(reason))
                    }
                    _ => return Err(SyncError::Source("The source file is unavailable.".into())),
                }
            }
            channel.send(SyncMessage::Done).await?;
            let checksum = if hash.is_empty() {
                match channel.recv().await? {
                    SyncMessage::FileChecksum { sha256 } => sha256,
                    SyncMessage::FileUnavailable { reason } => {
                        return Err(SyncError::Source(reason))
                    }
                    _ => {
                        return Err(SyncError::Source(
                            "The source file could not be verified.".into(),
                        ))
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
        mut request: SyncMessage,
    ) -> Result<(), SyncError> {
        let _permit = self
            .file_transfers
            .try_acquire()
            .map_err(|_| SyncError::Source("Another file transfer is in progress.".into()))?;
        let active = self
            .begin_active_sync(pairing_id)
            .ok_or_else(|| SyncError::Source("The device is no longer paired.".into()))?;
        let source = ActiveSyncSource {
            source,
            state: std::sync::Arc::clone(&active.state),
        };
        let cancel = active.cancelled();
        let first_id = match &request {
            SyncMessage::FileRequest { item_id, .. } => item_id.clone(),
            _ => return Err(SyncError::Source("Invalid file request.".into())),
        };
        let peer_name = self
            .peers()
            .list()
            .into_iter()
            .find(|p| p.pairing_id == pairing_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Paired device".into());
        let mut observer = source.observe_file_transfer(&first_id, &peer_name);
        let operation = async {
            let mut deferred = source.deferred_file(&first_id)?;
            if deferred.is_some()
                && matches!(&request, SyncMessage::FileRequest { sha256, .. } if !sha256.is_empty())
            {
                return Err(SyncError::Source(
                    "The requested file version does not match the source.".into(),
                ));
            }
            let mut digest = Sha256::new();
            let mut written = 0u64;
            let mut identity = None;
            let mut next_index = 0;
            loop {
                let SyncMessage::FileRequest {
                    item_id,
                    sha256,
                    index,
                    ..
                } = request
                else {
                    return Err(SyncError::Source("Invalid file request.".into()));
                };
                let current = (item_id, sha256);
                if index != next_index || identity.as_ref().is_some_and(|value| value != &current) {
                    return Err(SyncError::Source("Invalid file range.".into()));
                }
                identity = Some(current);
                let (id, hash) = identity.as_ref().unwrap();
                let chunk = if let Some(input) = deferred.as_mut() {
                    input.read_chunk()
                } else {
                    source.file_chunk(id, hash, index)
                };
                match chunk {
                    Ok(mut bytes) => {
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
                next_index += 1;
                request = channel.recv().await?;
                if matches!(request, SyncMessage::Done) {
                    if let Some(input) = deferred.as_mut() {
                        input.verify()?;
                        channel
                            .send(SyncMessage::FileChecksum {
                                sha256: format!("{:x}", digest.finalize()),
                            })
                            .await?;
                    }
                    return Ok(());
                }
            }
        };
        let result = tokio::select! { biased; _ = cancel.cancelled() => Err(SyncError::Source("File transfer was cancelled.".into())), result = operation => result };
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
