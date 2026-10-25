//! Product policy for synchronization conditions. Wire errors must select a
//! code here; views never classify error strings or infer a cause from Offline.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncEvidence {
    CompatibleProtocol,
    GroupAccepted,
    HistoryComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncIssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncIssueCode {
    VersionMismatch,
    ConnectionLost,
    SecureChannelInterrupted,
    TimedOut,
    HistoryBusy,
    ResourcesUnavailable,
    HistoryUnavailable,
    StorageFull,
    StoragePermissionDenied,
    HistoryLocked,
    HistoryCorrupt,
    HistoryIncompatible,
    LocalPayloadInvalid,
    EncryptionUnavailable,
    MembershipUnavailable,
    MembershipRejected,
    InvalidPeerData,
    ProtocolLimit,
    LocalProtocolError,
    ItemsTooLarge,
    ItemsUnavailable,
    ItemsRejected,
    ClockSkew,
    MorePending,
    UnknownFailure,
}

impl SyncIssueCode {
    pub const COUNT: usize = 25;
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VersionMismatch => "version_mismatch",
            Self::ConnectionLost => "connection_lost",
            Self::SecureChannelInterrupted => "secure_channel_interrupted",
            Self::TimedOut => "timed_out",
            Self::HistoryBusy => "history_busy",
            Self::ResourcesUnavailable => "resources_unavailable",
            Self::HistoryUnavailable => "history_unavailable",
            Self::StorageFull => "storage_full",
            Self::StoragePermissionDenied => "storage_permission_denied",
            Self::HistoryLocked => "history_locked",
            Self::HistoryCorrupt => "history_corrupt",
            Self::HistoryIncompatible => "history_incompatible",
            Self::LocalPayloadInvalid => "local_payload_invalid",
            Self::EncryptionUnavailable => "encryption_unavailable",
            Self::MembershipUnavailable => "membership_unavailable",
            Self::MembershipRejected => "membership_rejected",
            Self::InvalidPeerData => "invalid_peer_data",
            Self::ProtocolLimit => "protocol_limit",
            Self::LocalProtocolError => "local_protocol_error",
            Self::ItemsTooLarge => "items_too_large",
            Self::ItemsUnavailable => "items_unavailable",
            Self::ItemsRejected => "items_rejected",
            Self::ClockSkew => "clock_skew",
            Self::MorePending => "more_pending",
            Self::UnknownFailure => "unknown_failure",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::VersionMismatch => "Incompatible versions",
            Self::ConnectionLost => "Connection interrupted",
            Self::SecureChannelInterrupted => "Secure connection interrupted",
            Self::TimedOut => "Device did not respond",
            Self::HistoryBusy => "History is busy",
            Self::ResourcesUnavailable => "Resources unavailable",
            Self::HistoryUnavailable => "History unavailable",
            Self::StorageFull => "Storage full",
            Self::StoragePermissionDenied => "Storage access denied",
            Self::HistoryLocked => "History locked",
            Self::HistoryCorrupt => "History integrity problem",
            Self::HistoryIncompatible => "History version unsupported",
            Self::LocalPayloadInvalid => "History data cannot be opened",
            Self::EncryptionUnavailable => "Encryption unavailable",
            Self::MembershipUnavailable => "Device group unavailable",
            Self::MembershipRejected => "Device group rejected",
            Self::InvalidPeerData => "Invalid data received",
            Self::ProtocolLimit => "Synchronization limit exceeded",
            Self::LocalProtocolError => "Synchronization data could not be prepared",
            Self::ItemsTooLarge => "Clips exceed the size limit",
            Self::ItemsUnavailable => "Some clips are unavailable",
            Self::ItemsRejected => "Some clips failed validation",
            Self::ClockSkew => "Device time needs checking",
            Self::MorePending => "More clips pending",
            Self::UnknownFailure => "Synchronization interrupted",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::VersionMismatch => "Update CopyPaste on both devices.",
            Self::ConnectionLost => "The connection ended before synchronization completed.",
            Self::SecureChannelInterrupted => "The encrypted channel could not be completed or verified.",
            Self::TimedOut => "The device did not finish responding within the synchronization deadline.",
            Self::HistoryBusy => "History is temporarily busy. Synchronization will retry automatically.",
            Self::ResourcesUnavailable => "This device could not allocate the resources needed for synchronization.",
            Self::HistoryUnavailable => "Clipboard history could not be read or saved on this device.",
            Self::StorageFull => "Free up storage on this device to continue synchronization.",
            Self::StoragePermissionDenied => "Restore access to CopyPaste storage on this device.",
            Self::HistoryLocked => "Unlock secure storage on this device to continue synchronization.",
            Self::HistoryCorrupt => "History failed an integrity check on this device.",
            Self::HistoryIncompatible => "This history database requires a compatible CopyPaste version.",
            Self::LocalPayloadInvalid => "Stored clipboard data could not be authenticated or opened on this device.",
            Self::EncryptionUnavailable => "Secure storage could not encrypt clipboard data on this device.",
            Self::MembershipUnavailable => "The paired-device group could not be read or saved on this device.",
            Self::MembershipRejected => "The received device-group update could not be verified.",
            Self::InvalidPeerData => "Data received from this device could not be validated.",
            Self::ProtocolLimit => "Data from this device exceeds the supported synchronization limits.",
            Self::LocalProtocolError => "This device could not prepare clipboard data for synchronization.",
            Self::ItemsTooLarge => "These clips exceed the synchronization size limit.",
            Self::ItemsUnavailable => "Some requested clips were not supplied.",
            Self::ItemsRejected => "Some clips could not be validated.",
            Self::ClockSkew => "Some clip timestamps are too far in the future. Check the date and time on both devices.",
            Self::MorePending => "Waiting for the next batch.",
            Self::UnknownFailure => "The synchronization attempt did not complete. A more specific cause was not reported.",
        }
    }

    pub const fn resolved_message(self) -> &'static str {
        match self {
            Self::VersionMismatch => "Compatible versions confirmed.",
            Self::ConnectionLost | Self::SecureChannelInterrupted | Self::TimedOut => {
                "Connection check succeeded."
            }
            Self::UnknownFailure => "Synchronization completed.",
            Self::ResourcesUnavailable => "Synchronization resources are available again.",
            Self::HistoryBusy
            | Self::HistoryUnavailable
            | Self::StorageFull
            | Self::StoragePermissionDenied
            | Self::HistoryLocked
            | Self::HistoryCorrupt
            | Self::HistoryIncompatible
            | Self::LocalPayloadInvalid => "History access restored.",
            Self::EncryptionUnavailable => "Encryption is available again.",
            Self::MembershipUnavailable | Self::MembershipRejected => "Device group accepted.",
            Self::InvalidPeerData | Self::ProtocolLimit | Self::LocalProtocolError => {
                "Synchronization data validated."
            }
            Self::ItemsTooLarge => "No clips over the size limit remain pending.",
            Self::ItemsUnavailable => "No unavailable clips remain pending.",
            Self::ItemsRejected => "No rejected clips remain pending.",
            Self::ClockSkew => "No clips with invalid timestamps remain pending.",
            Self::MorePending => "The pending queue is synchronized.",
        }
    }

    pub const fn blocking(self) -> bool {
        match self {
            Self::VersionMismatch
            | Self::HistoryUnavailable
            | Self::StorageFull
            | Self::StoragePermissionDenied
            | Self::HistoryLocked
            | Self::HistoryCorrupt
            | Self::HistoryIncompatible
            | Self::LocalPayloadInvalid
            | Self::EncryptionUnavailable
            | Self::MembershipUnavailable
            | Self::MembershipRejected
            | Self::InvalidPeerData
            | Self::ProtocolLimit
            | Self::LocalProtocolError => true,
            Self::ConnectionLost
            | Self::SecureChannelInterrupted
            | Self::TimedOut
            | Self::HistoryBusy
            | Self::ResourcesUnavailable
            | Self::ItemsTooLarge
            | Self::ItemsUnavailable
            | Self::ItemsRejected
            | Self::ClockSkew
            | Self::MorePending
            | Self::UnknownFailure => false,
        }
    }

    pub const fn pending(self) -> bool {
        matches!(
            self,
            Self::ItemsTooLarge
                | Self::ItemsUnavailable
                | Self::ItemsRejected
                | Self::ClockSkew
                | Self::MorePending
        )
    }

    pub const fn severity(self) -> SyncIssueSeverity {
        if self.blocking() {
            SyncIssueSeverity::Error
        } else if matches!(self, Self::MorePending) {
            SyncIssueSeverity::Info
        } else {
            SyncIssueSeverity::Warning
        }
    }

    /// Scope is independent of severity and determines offline presentation.
    pub const fn scope(self) -> &'static str {
        match self {
            Self::VersionMismatch => "compatibility",
            Self::ConnectionLost | Self::SecureChannelInterrupted | Self::TimedOut => "connection",
            Self::HistoryBusy
            | Self::ResourcesUnavailable
            | Self::HistoryUnavailable
            | Self::StorageFull
            | Self::StoragePermissionDenied
            | Self::HistoryLocked
            | Self::HistoryCorrupt
            | Self::HistoryIncompatible
            | Self::LocalPayloadInvalid
            | Self::EncryptionUnavailable => "history",
            Self::MembershipUnavailable | Self::MembershipRejected => "membership",
            Self::InvalidPeerData | Self::ProtocolLimit | Self::LocalProtocolError => "protocol",
            Self::ItemsTooLarge
            | Self::ItemsUnavailable
            | Self::ItemsRejected
            | Self::ClockSkew
            | Self::MorePending => "items",
            Self::UnknownFailure => "unknown",
        }
    }

    /// Reachability changes, retry starts, elapsed time and disabling sync do
    /// not resolve a condition. Each domain requires positive evidence.
    pub const fn resolved_by(self, evidence: SyncEvidence) -> bool {
        match evidence {
            SyncEvidence::CompatibleProtocol => matches!(
                self,
                Self::VersionMismatch
                    | Self::ConnectionLost
                    | Self::SecureChannelInterrupted
                    | Self::TimedOut
            ),
            SyncEvidence::GroupAccepted => matches!(
                self,
                Self::VersionMismatch
                    | Self::ConnectionLost
                    | Self::SecureChannelInterrupted
                    | Self::TimedOut
                    | Self::MembershipUnavailable
                    | Self::MembershipRejected
            ),
            SyncEvidence::HistoryComplete => !self.pending(),
        }
    }
}

impl std::fmt::Display for SyncIssueCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncIssue {
    pub code: SyncIssueCode,
    pub since_ms: i64,
    #[serde(default)]
    pub count: u64,
}
