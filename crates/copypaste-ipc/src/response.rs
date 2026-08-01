//! The response envelope and every successful payload wrapper.

use crate::config::ConfigData;
use crate::error::ErrorCode;
use crate::payload::{
    BackupData, CloudStatusData, CloudSyncData, DiscoveredData, ExportData, ImportData, Item,
    ItemPage, PairingData, PeerInfo, PrivateModeData, StatusData, SyncResult,
};
use serde::{Deserialize, Serialize};

/// The settings as they now stand, plus what will not take effect yet.
///
/// `restart_required` is empty for every live field and is what lets a Settings
/// screen say so at the moment of the change rather than leaving the user to
/// discover it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigApplied {
    pub config: ConfigData,
    pub restart_required: Vec<String>,
}

/// What changed. Coalesced: a burst of captures may produce one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// History changed — an item was added, deleted, pinned, imported or
    /// arrived from a peer or the cloud.
    Items,
    /// The paired-device list changed.
    Peers,
}

/// One push frame on a [`crate::Method::Watch`] connection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EventData {
    pub event: EventKind,
    /// Live item count at the time of the event, so a client can render a badge
    /// without a round trip.
    pub item_count: u64,
    /// The change was a *capture* — something the user copied, as opposed to a
    /// delete, a pin, an import, or a row arriving from a peer.
    ///
    /// This is what a client needs to post the notification and play the sound
    /// that `ConfigData::notify_on_copy` and `ConfigData::sound_on_copy` gate.
    /// A flag rather than a new [`EventKind`] variant so that a client built
    /// against an older build still decodes the frame: an unknown enum variant
    /// fails deserialisation, and a watcher that stops decoding stops updating.
    ///
    /// It carries no content and no id, for the reason [`crate::Method::Watch`] gives:
    /// a subscriber that wants the item re-reads it through the ordinary
    /// methods, which keeps one set of rules about what a client may see.
    ///
    /// The daemon does **not** consult `notify_on_copy` before setting it. The
    /// flag says what happened; the setting says what to do about it, and the
    /// client that owns the notification is the one that should read it (the
    /// daemon has no bundle and therefore cannot post one — see
    /// `daemon/src/notify.rs`).
    #[serde(default)]
    pub captured: bool,

    /// Detected secrets the auto-wipe sweep deleted, in the change this event
    /// reports. Zero on every other change.
    ///
    /// A deletion the user did not ask for is the one history change they have
    /// to be told about, and until this field there was no way to tell them:
    /// `ConfigData::sensitive_ttl_secs` defaults to `0` — the feature off —
    /// *because* the count could not leave the daemon, and the Settings screen
    /// says so in as many words.
    ///
    /// A count and not a list of ids, for the reason [`crate::Method::Watch`] gives:
    /// the rows are gone, and an event carries no content.
    ///
    /// Same `#[serde(default)]` reasoning as [`EventData::captured`] — a
    /// watcher built against an older daemon must keep decoding.
    #[serde(default)]
    pub swept: u32,
}

/// One reply. `ok` distinguishes success from failure without inspecting the
/// payload.
#[derive(Debug, Clone)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    pub data: Option<ResponseData>,
    pub error: Option<String>,
    /// The machine-readable code when this build recognises it.
    pub error_code: Option<ErrorCode>,
    /// An unrecognised code exactly as it appeared on the wire.
    ///
    /// Kept separate from [`Response::error_code`] so matching known codes stays
    /// exhaustive while a future daemon can add one without breaking decoding.
    pub raw_error_code: Option<String>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ErrorCodeRef<'a> {
    Known(ErrorCode),
    Unknown(&'a str),
}

#[derive(Serialize)]
struct ResponseRef<'a> {
    id: u64,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<&'a ResponseData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<ErrorCodeRef<'a>>,
}

#[derive(Deserialize)]
struct ResponseOwned {
    id: u64,
    ok: bool,
    data: Option<ResponseData>,
    error: Option<String>,
    error_code: Option<String>,
}

impl Serialize for Response {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ResponseRef {
            id: self.id,
            ok: self.ok,
            data: self.data.as_ref(),
            error: self.error.as_deref(),
            error_code: self
                .raw_error_code
                .as_deref()
                .map(ErrorCodeRef::Unknown)
                .or_else(|| self.error_code.map(ErrorCodeRef::Known)),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Response {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let response = ResponseOwned::deserialize(deserializer)?;
        let error_code = response.error_code.as_deref().and_then(ErrorCode::parse);
        let raw_error_code = if error_code.is_none() {
            response.error_code
        } else {
            None
        };
        Ok(Self {
            id: response.id,
            ok: response.ok,
            data: response.data,
            error: response.error,
            error_code,
            raw_error_code,
        })
    }
}

impl Response {
    pub fn ok(id: u64, data: ResponseData) -> Self {
        Self {
            id,
            ok: true,
            data: Some(data),
            error: None,
            error_code: None,
            raw_error_code: None,
        }
    }

    /// Build a failure reply.
    ///
    /// `message` must never contain a filesystem path: the daemon socket path
    /// discloses the local username (CLAUDE.md rule 4). Callers map internal
    /// errors to a plain sentence before they get here.
    pub fn err(id: u64, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            data: None,
            error: Some(message.into()),
            error_code: Some(code),
            raw_error_code: None,
        }
    }
}

/// The payload of a successful reply.
///
/// Every variant is externally tagged on the wire. The wrapper is load-bearing:
/// an empty array contains no element shape from which a decoder could infer
/// whether it is a peer list or a sync report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseData {
    Status(StatusData),
    Export(ExportData),
    Import(ImportData),
    Backup(BackupData),
    Discovered(DiscoveredData),
    Config(ConfigApplied),
    Event(EventData),
    Page(ItemPage),
    Item(Item),
    Count(u64),
    Pairing(PairingData),
    Peers(Vec<PeerInfo>),
    Sync(Vec<SyncResult>),
    CloudStatus(CloudStatusData),
    CloudSync(CloudSyncData),
    PrivateMode(PrivateModeData),
    Empty {},
}
