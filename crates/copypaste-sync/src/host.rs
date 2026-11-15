//! Serialized services provided by the application to installed sync code.
use crate::LocalItem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const UPLOAD_FLOOR: &str = "cloud_upload_floor_ms";
pub const UPLOAD_FLOOR_ITEM: &str = "cloud_upload_floor_item_id";
pub const UNREADABLE_UPLOADS: &str = "cloud_unreadable_uploads";

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum SyncHostRequest {
    GroupSnapshot,
    GroupApply {
        device_id: String,
        snapshot: String,
    },
    SpaceKey {
        fallback: Option<zeroize::Zeroizing<String>>,
    },
    SpaceRotate,
    SpaceEnvelopes,
    SpaceAccept {
        envelopes: Vec<crate::space::SpaceEnvelope>,
    },
    SpaceRecovery,
    SpaceRecover {
        epoch: u64,
        plaintext: zeroize::Zeroizing<String>,
    },
    SpaceAllowRecovery,
    SpaceConfirm {
        device_id: String,
        epoch: u64,
    },
    SpaceDeparted,
    SpaceNotice,
    SpaceNoticed,
    Active,
    DeviceId,
    SyncEnabled,
    Revision,
    ReadState {
        keys: Vec<String>,
    },
    WriteState {
        values: BTreeMap<String, String>,
    },
    ClearState {
        keys: Vec<String>,
    },
    Scan {
        since_ms: i64,
        after_item_id: Option<String>,
    },
    Apply {
        items: Vec<LocalItem>,
    },
    Requeue {
        incoming: LocalItem,
    },
    CommitUpload {
        started_ms: i64,
    },
    /// Kept file copies made on this device that may go to cloud storage,
    /// newest first. Answers `Vec<CloudFileUpload>`.
    CloudFileUploads {
        since_ms: i64,
        limit: u32,
    },
    /// Up to `len` plaintext bytes of a kept copy from `offset`. Answers
    /// [`FileBytes`].
    ReadCloudFile {
        item_id: String,
        file_index: u32,
        offset: u64,
        len: u32,
    },
    /// Of these items, the ones no longer live here. Answers `Vec<String>`.
    GoneFiles {
        item_ids: Vec<String>,
    },
    /// Files this device waits to receive from cloud storage. Answers
    /// `Vec<CloudFileFetch>`.
    CloudFileFetches,
    /// The next bytes of a fetched file, in order.
    CloudFileChunk {
        item_id: String,
        file_index: u32,
        chunk: FileBytes,
    },
    /// Ends a fetch with the file's SHA-256, or why it could not be fetched.
    CloudFileDone {
        item_id: String,
        file_index: u32,
        sha256: Option<String>,
        error: Option<String>,
    },
    /// How one sync round ended, for the host's runtime log.
    RoundFinished {
        failure: Option<RoundFailure>,
    },
    /// What one cloud file round did, as counts, for the host's runtime log.
    FileRoundFinished {
        uploaded: u32,
        failed: u32,
        removed: u32,
    },
}

/// Why a sync round failed, as a fixed kind; never free text, so nothing a
/// provider sees can reach the host's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundFailure {
    LocalHistory,
    Encryption,
    SignInRequired,
    RateLimited,
    Unreachable,
}

impl RoundFailure {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalHistory => "local history unavailable",
            Self::Encryption => "encryption failed",
            Self::SignInRequired => "sign-in required",
            Self::RateLimited => "rate limited",
            Self::Unreachable => "backend unreachable",
        }
    }
}

/// One file of a clip the host offers for upload; a multi-file clip offers
/// each member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudFileUpload {
    pub item_id: String,
    pub file_index: u32,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: i64,
}

/// A file the host wants from cloud storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudFileFetch {
    pub item_id: String,
    pub file_index: u32,
    pub size_bytes: u64,
}

/// File bytes on the JSON host channel, as base64.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileBytes(pub Vec<u8>);

impl Serialize for FileBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use base64::Engine as _;
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for FileBytes {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use base64::Engine as _;
        let text = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(text)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
