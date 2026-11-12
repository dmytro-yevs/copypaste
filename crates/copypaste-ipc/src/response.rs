//! The response envelope and every successful payload wrapper.

use crate::config::ConfigData;
use crate::error::ErrorCode;
use crate::payload::{
    BackupData, CloudStatusData, CloudSyncData, DiscoveredData, ExportChunk, ExportData,
    HistoryFacets, ImagePreview, ImportData, Item, ItemPage, PairingInviteData,
    PairingProgressData, PeerInfo, PrivateModeData, StatusData, SyncResult, TransferProgress,
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
    /// History weighed against `config.storage_quota_bytes`, read after this
    /// reply's settings took effect. Absent from an older runtime or when the
    /// store could not answer; never a guess.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_usage: Option<StorageUsage>,
}

/// What the storage quota currently counts. Pinned clips are outside the
/// quota and therefore outside both fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageUsage {
    /// Retained encrypted bytes of live unpinned clips: the total the quota
    /// sweep compares with the limit. One clip larger than the quota is kept,
    /// so this can exceed it.
    pub used_bytes: u64,
    /// Capture time of the oldest live unpinned clip, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oldest_unpinned_at: Option<i64>,
}

/// What changed. Coalesced: a burst of captures may produce one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// History changed — an item was added, deleted, pinned, imported or
    /// arrived from a peer or the cloud.
    Items,
    /// The paired-device list or protected pairing ceremony changed.
    Peers,
    /// The paste queue moved to [`EventData::paste_queue_revision`].
    PasteQueue,
    /// Synced library records changed; [`EventData::library`] says which.
    Library,
    /// An AI tool approval is pending or a session's authorization changed.
    AiApproval,
    /// A long operation advanced; [`EventData::operation`] carries the counts.
    Progress,
}

/// Bitset of the library record kinds a [`EventKind::Library`] event covers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LibraryKinds(pub u32);

impl LibraryKinds {
    pub const SNIPPETS: Self = Self(1);
    pub const COLLECTIONS: Self = Self(1 << 1);
    pub const RULES: Self = Self(1 << 2);
    pub const PIPELINES: Self = Self(1 << 3);
    pub const PREFERENCES: Self = Self(1 << 4);
    pub const SET_MEMBERS: Self = Self(1 << 5);
    pub const GRANTS: Self = Self(1 << 6);

    #[must_use]
    pub const fn all() -> Self {
        Self((1 << 7) - 1)
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

/// Counts of one cancellable operation, keyed by the caller's request id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationProgress {
    pub request_id: String,
    pub done: u64,
    pub total: u64,
}

/// One kind a [`crate::Method::WatchEvents`] subscriber asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKindFilter {
    Items,
    Peers,
    PasteQueue,
    /// Only library events that intersect `kinds`.
    Library {
        #[serde(default = "LibraryKinds::all")]
        kinds: LibraryKinds,
    },
    AiApproval,
    Progress,
    /// Capture-state snapshots, including the one sent on subscription.
    Status,
    /// File transfer operations and their byte progress.
    Transfers,
}

/// At most this many filters per subscription.
pub const MAX_EVENT_FILTERS: usize = 16;

impl EventKindFilter {
    fn admits_change(self, change: &EventData) -> bool {
        match self {
            Self::Items => change.event == EventKind::Items,
            Self::Peers => change.event == EventKind::Peers,
            Self::PasteQueue => change.event == EventKind::PasteQueue,
            Self::Library { kinds } => {
                change.event == EventKind::Library
                    && change
                        .library
                        .is_none_or(|changed| changed.intersects(kinds))
            }
            Self::AiApproval => change.event == EventKind::AiApproval,
            Self::Progress => change.event == EventKind::Progress,
            Self::Status | Self::Transfers => false,
        }
    }

    /// Whether a frame passes `filters`; a frame with several parts passes
    /// when any part does.
    #[must_use]
    pub fn admits(filters: &[Self], event: &RuntimeEventData) -> bool {
        filters.iter().any(|filter| {
            event
                .change
                .as_ref()
                .is_some_and(|change| filter.admits_change(change))
                || (*filter == Self::Status && event.capture.is_some())
                || (*filter == Self::Transfers
                    && (event.file_operation.is_some() || event.progress.is_some()))
        })
    }
}

/// One push frame on a [`crate::Method::Watch`] connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_status: Option<crate::SyncStatus>,
    pub event: EventKind,
    /// Live item count at the time of the event, so a client can render a badge
    /// without a round trip.
    pub item_count: u64,
    /// True for a local capture or a received item successfully written to the
    /// system clipboard, so the client can apply `notify_on_copy`. Native hosts
    /// own sound feedback and Android notifications.
    ///
    /// A defaulted flag, not a new [`EventKind`], keeps older watchers decoding:
    /// an unknown enum variant would reject the frame. It carries no clipboard
    /// content; subscribers re-read through ordinary methods.
    #[serde(default)]
    pub captured: bool,
    /// Identifies the copied clip without including its content in events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<LibraryKinds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_queue_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<OperationProgress>,
}

impl EventData {
    /// A content-free change signal with no optional details.
    #[must_use]
    pub fn change(event: EventKind, item_count: u64) -> Self {
        Self {
            sync_status: None,
            event,
            item_count,
            captured: false,
            captured_item_id: None,
            library: None,
            paste_queue_revision: None,
            operation: None,
        }
    }
}

/// Progress contains only the caller's opaque token and byte counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProgress {
    pub token: String,
    pub written: u64,
    pub total: u64,
}

/// Opt-in runtime stream payload. Legacy watchers never receive these frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEventData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_operation: Option<crate::FileOperationUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<EventData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<FileProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<crate::StatusData>,
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
    #[serde(default, deserialize_with = "deserialize_present_option")]
    data: Presence<Option<ResponseData>>,
    #[serde(default, deserialize_with = "deserialize_present_option")]
    error: Presence<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_present_option")]
    error_code: Presence<Option<String>>,
}

#[derive(Default)]
enum Presence<T> {
    #[default]
    Missing,
    Present(T),
}

fn deserialize_present_option<'de, D, T>(deserializer: D) -> Result<Presence<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Presence::Present)
}

impl Serialize for Response {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
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
        let (data, error, wire_error_code) = if response.ok {
            if !matches!(response.error, Presence::Missing)
                || !matches!(response.error_code, Presence::Missing)
            {
                return Err(serde::de::Error::custom(
                    "successful response requires data and no failure fields",
                ));
            }
            let Presence::Present(Some(data)) = response.data else {
                return Err(serde::de::Error::custom(
                    "successful response requires data and no failure fields",
                ));
            };
            (Some(data), None, None)
        } else {
            if !matches!(response.data, Presence::Missing) {
                return Err(serde::de::Error::custom(
                    "failed response requires error and error_code without data",
                ));
            }
            let Presence::Present(Some(error)) = response.error else {
                return Err(serde::de::Error::custom(
                    "failed response requires error and error_code without data",
                ));
            };
            let Presence::Present(Some(error_code)) = response.error_code else {
                return Err(serde::de::Error::custom(
                    "failed response requires error and error_code without data",
                ));
            };
            if error.is_empty() || error_code.is_empty() {
                return Err(serde::de::Error::custom(
                    "failed response requires error and error_code without data",
                ));
            }
            (None, Some(error), Some(error_code))
        };

        let error_code = wire_error_code.as_deref().and_then(ErrorCode::parse);
        let raw_error_code = if error_code.is_none() {
            wire_error_code
        } else {
            None
        };
        if raw_error_code
            .as_deref()
            .is_some_and(|code| !is_safe_raw_error_code(code))
        {
            return Err(serde::de::Error::custom("invalid unknown error code"));
        }
        let response = Self {
            id: response.id,
            ok: response.ok,
            data,
            error,
            error_code,
            raw_error_code,
        };
        response.validate().map_err(serde::de::Error::custom)?;
        Ok(response)
    }
}

impl Response {
    fn validate(&self) -> Result<(), &'static str> {
        if self.ok {
            if self.data.is_none()
                || self.error.is_some()
                || self.error_code.is_some()
                || self.raw_error_code.is_some()
            {
                return Err("successful response requires data and no failure fields");
            }
            return Ok(());
        }

        if self.data.is_some()
            || self.error.as_deref().is_none_or(str::is_empty)
            || (self.error_code.is_some() == self.raw_error_code.is_some())
            || self
                .raw_error_code
                .as_deref()
                .is_some_and(|code| !is_safe_raw_error_code(code))
        {
            return Err("failed response requires error and exactly one error code without data");
        }

        Ok(())
    }

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
    /// discloses the local username (the privacy contract). Callers map internal
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

fn is_safe_raw_error_code(code: &str) -> bool {
    (1..=64).contains(&code.len())
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// The payload of a successful reply.
///
/// Every variant is externally tagged on the wire. The wrapper is load-bearing:
/// an empty array contains no element shape from which a decoder could infer
/// whether it is a peer list or a sync report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseData {
    /// SDK JSON contracts carried without exposing a module's native ABI.
    Modules {
        json: String,
    },
    FileOperations(Vec<crate::FileOperation>),
    DownloadedFile(Option<String>),
    Status(StatusData),
    Export(ExportData),
    ExportChunk(ExportChunk),
    Import(ImportData),
    Backup(BackupData),
    /// An interim frame of a streamed operation.
    Progress(TransferProgress),
    Discovered(DiscoveredData),
    Config(ConfigApplied),
    Event(EventData),
    RuntimeEvent(Box<RuntimeEventData>),
    Page(ItemPage),
    HistoryFacets(HistoryFacets),
    /// `None` when the query cannot be counted exactly from indexes.
    HistoryCounts(Option<crate::HistoryGroupCounts>),
    HistoryTags(crate::HistoryTags),
    SpoilerState {
        revision: u64,
        revealed_ids: Vec<String>,
    },
    Item(Item),
    ImagePreview(ImagePreview),
    SourceAppIcon(ImagePreview),
    Count(u64),
    PairingInvite(PairingInviteData),
    PairingProgress(PairingProgressData),
    Peers(Vec<PeerInfo>),
    Sync(Vec<SyncResult>),
    CloudStatus(CloudStatusData),
    CloudSync(CloudSyncData),
    PrivateMode(PrivateModeData),
    SnippetPage(crate::snippets::SnippetPage),
    Snippet(crate::snippets::SnippetDetail),
    SnippetSave(crate::snippets::SnippetSaveOutcome),
    SnippetCollections(Vec<crate::snippets::SnippetCollection>),
    SnippetCollection(crate::snippets::SnippetCollection),
    TemplateLints(Vec<crate::snippets::TemplateLint>),
    SnippetHits(Vec<crate::snippets::SnippetHit>),
    Rendered(crate::snippets::Rendered),
    Delivered(crate::clips::Delivered),
    SnippetImportPlan(crate::snippets::ImportPlan),
    SnippetImportReport(crate::snippets::ImportReport),
    SnippetPreferences(crate::snippets::SnippetPreferences),
    ClipAnnotations(Vec<crate::clips::ClipAnnotations>),
    RuleTrace(crate::rules::RuleTrace),
    MaybeItem(Option<Item>),
    PasteQueue(crate::paste_queue::PasteQueueState),
    QueueDelivery(crate::paste_queue::QueueDelivery),
    TransformCatalogue(Vec<crate::transforms::TransformInfo>),
    TransformPreview(crate::transforms::TransformPreview),
    TransformSuggestions(Vec<String>),
    Pipelines(Vec<crate::transforms::Pipeline>),
    Pipeline(crate::transforms::Pipeline),
    TransformPreferences(crate::transforms::TransformPreferences),
    PipelineDefault(Option<String>),
    RuleList(crate::rules::RuleList),
    Rule(crate::rules::RuleRow),
    RuleErrors(Vec<crate::rules::RuleError>),
    RulePreview(crate::rules::RulePreview),
    RuleApplyReport(crate::rules::RuleApplyReport),
    OperationProgress(Option<OperationProgress>),
    SkipLog(Vec<crate::rules::SkipEvent>),
    AiPolicy(crate::ai::AiPolicy),
    AiGrants(Vec<crate::ai::AiGrant>),
    AiGrant(crate::ai::AiGrant),
    AiGrantCreated(crate::ai::AiGrantCreated),
    AiApprovals(Vec<crate::ai::AiApprovalRequest>),
    AiAccessLog(Vec<crate::ai::AiAccessEntry>),
    AiStatus(crate::ai::AiStatus),
    AiSession(crate::ai::AiSession),
    AiToolResult(crate::ai::AiToolResult),
    Empty {},
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_event_accepts_legacy_payload_without_capture() {
        let event: RuntimeEventData =
            serde_json::from_str(r#"{"change":null,"progress":null}"#).unwrap();
        assert!(event.capture.is_none());
    }

    #[test]
    fn event_filters_admit_only_the_requested_kinds() {
        let library = |kinds| RuntimeEventData {
            file_operation: None,
            change: Some(EventData {
                library: Some(kinds),
                ..EventData::change(EventKind::Library, 0)
            }),
            progress: None,
            capture: None,
        };
        let items = RuntimeEventData {
            change: Some(EventData::change(EventKind::Items, 1)),
            ..library(LibraryKinds::GRANTS)
        };
        let filters = [
            EventKindFilter::AiApproval,
            EventKindFilter::Library {
                kinds: LibraryKinds::GRANTS,
            },
        ];
        assert!(EventKindFilter::admits(
            &filters,
            &library(LibraryKinds::GRANTS)
        ));
        assert!(!EventKindFilter::admits(
            &filters,
            &library(LibraryKinds::SNIPPETS)
        ));
        assert!(!EventKindFilter::admits(&filters, &items));
        let decoded: Vec<EventKindFilter> =
            serde_json::from_str(r#"[{"kind":"library"},{"kind":"status"}]"#).unwrap();
        assert_eq!(
            decoded[0],
            EventKindFilter::Library {
                kinds: LibraryKinds::all()
            }
        );
    }

    #[test]
    fn storage_usage_is_additive_and_omitted_when_absent() {
        let old_wire = serde_json::json!({
            "config": ConfigData::default(),
            "restart_required": [],
        });
        let old: ConfigApplied = serde_json::from_value(old_wire.clone()).unwrap();
        assert_eq!(old.storage_usage, None);
        assert_eq!(serde_json::to_value(&old).unwrap(), old_wire);

        let current = ConfigApplied {
            storage_usage: Some(StorageUsage {
                used_bytes: 2_048,
                oldest_unpinned_at: None,
            }),
            ..old
        };
        let wire = serde_json::to_value(&current).unwrap();
        assert_eq!(
            wire["storage_usage"],
            serde_json::json!({"used_bytes": 2_048})
        );
        let decoded: ConfigApplied = serde_json::from_value(wire).unwrap();
        assert_eq!(decoded.storage_usage, current.storage_usage);
    }

    #[test]
    fn success_requires_one_payload_and_no_failure_fields() {
        for json in [
            r#"{"id":1,"ok":true}"#,
            r#"{"id":1,"ok":true,"data":null}"#,
            r#"{"id":1,"ok":true,"data":{"empty":{}},"error":"no"}"#,
            r#"{"id":1,"ok":true,"data":{"empty":{}},"error_code":"internal"}"#,
        ] {
            assert!(serde_json::from_str::<Response>(json).is_err(), "{json}");
        }
    }

    #[test]
    fn failure_requires_a_message_and_one_error_code_without_data() {
        for json in [
            r#"{"id":1,"ok":false}"#,
            r#"{"id":1,"ok":false,"error":"no"}"#,
            r#"{"id":1,"ok":false,"error_code":"internal"}"#,
            r#"{"id":1,"ok":false,"error":"no","error_code":"internal","data":{"empty":{}}}"#,
        ] {
            assert!(serde_json::from_str::<Response>(json).is_err(), "{json}");
        }
    }

    #[test]
    fn explicit_null_fields_are_not_treated_as_missing() {
        for json in [
            r#"{"id":1,"ok":true,"data":{"empty":{}},"error":null}"#,
            r#"{"id":1,"ok":true,"data":{"empty":{}},"error_code":null}"#,
            r#"{"id":1,"ok":true,"data":{"empty":{}},"error":null,"error_code":null}"#,
            r#"{"id":1,"ok":false,"data":null,"error":"no","error_code":"internal"}"#,
            r#"{"id":1,"ok":false,"error":null,"error_code":"internal"}"#,
            r#"{"id":1,"ok":false,"error":"no","error_code":null}"#,
        ] {
            assert!(serde_json::from_str::<Response>(json).is_err(), "{json}");
        }
    }

    #[test]
    fn safe_future_error_code_is_preserved_without_a_retry_policy() {
        let response: Response = serde_json::from_str(
            r#"{"id":1,"ok":false,"error":"new refusal","error_code":"future_state_2"}"#,
        )
        .unwrap();

        assert_eq!(response.error_code, None);
        assert_eq!(response.raw_error_code.as_deref(), Some("future_state_2"));
    }

    #[test]
    fn unsafe_raw_error_codes_are_rejected_at_the_ipc_boundary() {
        let too_long = "a".repeat(65);
        for code in ["", "UPPER", "path/name", "future.state", too_long.as_str()] {
            let json =
                format!(r#"{{"id":1,"ok":false,"error":"new refusal","error_code":"{code}"}}"#);
            assert!(serde_json::from_str::<Response>(&json).is_err(), "{code:?}");

            let response = Response {
                id: 1,
                ok: false,
                data: None,
                error: Some("new refusal".into()),
                error_code: None,
                raw_error_code: Some(code.into()),
            };
            assert!(serde_json::to_string(&response).is_err(), "{code:?}");
        }

        let known = Response::err(1, ErrorCode::Internal, "failed");
        assert!(serde_json::to_string(&known).is_ok());
    }

    #[test]
    fn invalid_direct_envelopes_do_not_serialize() {
        let cases = [
            Response {
                id: 1,
                ok: true,
                data: None,
                error: None,
                error_code: None,
                raw_error_code: None,
            },
            Response {
                id: 1,
                ok: false,
                data: Some(ResponseData::Empty {}),
                error: Some("no".into()),
                error_code: Some(ErrorCode::Internal),
                raw_error_code: None,
            },
            Response {
                id: 1,
                ok: false,
                data: None,
                error: Some("no".into()),
                error_code: Some(ErrorCode::Internal),
                raw_error_code: Some("future_state_2".into()),
            },
        ];

        for response in cases {
            assert!(serde_json::to_string(&response).is_err());
        }
    }
}
