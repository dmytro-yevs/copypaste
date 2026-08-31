//! The response envelope and every successful payload wrapper.

use crate::config::ConfigData;
use crate::error::ErrorCode;
use crate::payload::{
    BackupData, CloudStatusData, CloudSyncData, DiscoveredData, ExportData, ImagePreview,
    ImportData, Item, ItemPage, PairingInviteData, PairingProgressData, PeerInfo, PrivateModeData,
    StatusData, SyncResult,
};
use serde::{Deserialize, Serialize};

/// The settings as they now stand, plus what will not take effect yet.
///
/// `restart_required` is empty for every live field and is what lets a Settings
/// screen say so at the moment of the change rather than leaving the user to
/// discover it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct ConfigApplied {
    pub config: ConfigData,
    pub restart_required: Vec<String>,
}

/// What changed. Coalesced: a burst of captures may produce one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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
    /// True only for a clipboard capture, so the client can apply
    /// `notify_on_copy`/`sound_on_copy`; the daemon reports what happened and
    /// does not apply those client-owned settings.
    ///
    /// A defaulted flag, not a new [`EventKind`], keeps older watchers decoding:
    /// an unknown enum variant would reject the frame. It carries no content or
    /// id; subscribers re-read through ordinary methods.
    #[serde(default)]
    pub captured: bool,

    /// Secrets deleted by auto-wipe in this change; zero otherwise. This count
    /// makes an unrequested deletion visible without exposing ids or content.
    ///
    /// Defaulted so watchers built before the field keep decoding.
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
    /// discloses the local username (AGENTS.md rule 4). Callers map internal
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
    Status(StatusData),
    Export(ExportData),
    Import(ImportData),
    Backup(BackupData),
    Discovered(DiscoveredData),
    Config(ConfigApplied),
    Event(EventData),
    Page(ItemPage),
    Item(Item),
    ImagePreview(ImagePreview),
    Count(u64),
    PairingInvite(PairingInviteData),
    PairingProgress(PairingProgressData),
    Peers(Vec<PeerInfo>),
    Sync(Vec<SyncResult>),
    CloudStatus(CloudStatusData),
    CloudSync(CloudSyncData),
    PrivateMode(PrivateModeData),
    Empty {},
}

#[cfg(test)]
mod tests {
    use super::*;

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
