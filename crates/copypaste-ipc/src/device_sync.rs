//! Which content syncs with which device.
//!
//! Device-local like every setting: each device decides what it sends to and
//! accepts from each peer. [`DeviceSyncSettings::allows`] is the one check every
//! transport and file transfer asks, so the answer cannot differ between P2P,
//! cloud, and file bytes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::content_type::{self, ContentClass};

/// The most paired devices a policy may name.
pub const MAX_DEVICE_POLICIES: usize = 256;
/// Device ids are UUID-shaped; the cap only bounds the stored record.
pub const MAX_DEVICE_ID_LEN: usize = 128;

/// A content kind a policy selects. Rich text and HTML are text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncKind {
    Text,
    Images,
    Files,
}

impl SyncKind {
    /// The kind a content type syncs as, or `None` for internal records such as
    /// pin organization, which follow only the device switches.
    #[must_use]
    pub fn of(content_type: &str) -> Option<Self> {
        if content_type::is_internal(content_type) {
            return None;
        }
        Some(match content_type::classify(content_type) {
            ContentClass::Text => Self::Text,
            ContentClass::Image => Self::Images,
            // A future binary type gets the file switch, like the file limit.
            ContentClass::File | ContentClass::Other => Self::Files,
        })
    }
}

/// Content kinds one device exchanges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncContent {
    pub text: bool,
    pub images: bool,
    pub files: bool,
}

impl Default for SyncContent {
    fn default() -> Self {
        Self::ALL
    }
}

impl SyncContent {
    pub const ALL: Self = Self {
        text: true,
        images: true,
        files: true,
    };
    pub const NONE: Self = Self {
        text: false,
        images: false,
        files: false,
    };

    #[must_use]
    pub fn allows(self, kind: SyncKind) -> bool {
        match kind {
            SyncKind::Text => self.text,
            SyncKind::Images => self.images,
            SyncKind::Files => self.files,
        }
    }

    fn widens(self, before: Self) -> bool {
        (self.text && !before.text)
            || (self.images && !before.images)
            || (self.files && !before.files)
    }
}

/// What this device exchanges with one paired device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PeerSyncPolicy {
    pub enabled: bool,
    /// Stops automatic exchange with this device; a manual send still works.
    pub paused: bool,
    pub content: SyncContent,
}

impl Default for PeerSyncPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            paused: false,
            content: SyncContent::ALL,
        }
    }
}

impl PeerSyncPolicy {
    fn allows(self, kind: Option<SyncKind>) -> bool {
        self.enabled && kind.is_none_or(|kind| self.content.allows(kind))
    }

    fn widens(self, before: Self) -> bool {
        self.enabled && (!before.enabled || self.content.widens(before.content))
    }
}

/// Per-device sync policy. **Live.**
///
/// A device without an entry syncs everything, so pairing needs no extra step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSyncSettings {
    /// Stops all automatic sync, peers and cloud; a manual send still works.
    pub paused: bool,
    /// Kinds this device exchanges with every peer and the cloud account.
    pub this_device: SyncContent,
    /// Paired devices by device id.
    pub devices: BTreeMap<String, PeerSyncPolicy>,
}

/// Where a policy change let more content through, so the transports can
/// re-offer what they passed over while it was off.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncWidening {
    pub this_device: bool,
    pub devices: Vec<String>,
}

impl SyncWidening {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.this_device && self.devices.is_empty()
    }
}

impl DeviceSyncSettings {
    /// The value an unreadable record falls back to: nothing syncs.
    #[must_use]
    pub fn closed() -> Self {
        Self {
            paused: false,
            this_device: SyncContent::NONE,
            devices: BTreeMap::new(),
        }
    }

    /// The policy for one paired device.
    #[must_use]
    pub fn device(&self, device_id: &str) -> PeerSyncPolicy {
        self.devices.get(device_id).copied().unwrap_or_default()
    }

    /// Whether content of `content_type` may move between this device and every
    /// device in `devices`: the link peer, the origin of a relayed clip, or none
    /// for the cloud account. It never removes anything already stored.
    #[must_use]
    pub fn allows(&self, content_type: &str, devices: &[&str]) -> bool {
        let kind = SyncKind::of(content_type);
        kind.is_none_or(|kind| self.this_device.allows(kind))
            && devices.iter().all(|id| self.device(id).allows(kind))
    }

    /// Whether automatic sync is paused everywhere or, for `device_id`, with
    /// that device. Manual sends ignore it.
    #[must_use]
    pub fn auto_sync_paused(&self, device_id: Option<&str>) -> bool {
        self.paused || device_id.is_some_and(|id| self.device(id).paused)
    }

    /// Whether a pause switch changed, so schedulers must be woken.
    #[must_use]
    pub fn pause_changed(&self, before: &Self) -> bool {
        self.paused != before.paused
            || self
                .devices
                .keys()
                .chain(before.devices.keys())
                .any(|id| self.device(id).paused != before.device(id).paused)
    }

    /// Whether anything may move with `device_id`.
    #[must_use]
    pub fn device_enabled(&self, device_id: &str) -> bool {
        self.device(device_id).enabled
    }

    /// Where `self` lets through content `before` did not.
    #[must_use]
    pub fn widening_from(&self, before: &Self) -> SyncWidening {
        let ids: std::collections::BTreeSet<&String> =
            self.devices.keys().chain(before.devices.keys()).collect();
        SyncWidening {
            this_device: self.this_device.widens(before.this_device),
            devices: ids
                .into_iter()
                .filter(|id| self.device(id).widens(before.device(id)))
                .cloned()
                .collect(),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), crate::config::ConfigError> {
        if self.devices.len() > MAX_DEVICE_POLICIES {
            return Err(crate::config::ConfigError::TooMany {
                field: "device_sync",
                max: MAX_DEVICE_POLICIES,
            });
        }
        if self
            .devices
            .keys()
            .any(|id| id.trim().is_empty() || id.len() > MAX_DEVICE_ID_LEN)
        {
            return Err(crate::config::ConfigError::BadEntry {
                field: "device_sync",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_peer(this_device: SyncContent, peer: PeerSyncPolicy) -> DeviceSyncSettings {
        DeviceSyncSettings {
            this_device,
            devices: BTreeMap::from([("peer".to_string(), peer)]),
            ..DeviceSyncSettings::default()
        }
    }

    #[test]
    fn this_device_and_every_named_device_must_allow_the_kind() {
        let no_peer_images = with_peer(
            SyncContent::ALL,
            PeerSyncPolicy {
                content: SyncContent {
                    images: false,
                    ..SyncContent::ALL
                },
                ..PeerSyncPolicy::default()
            },
        );
        assert!(no_peer_images.allows(content_type::IMAGE_PNG, &["other"]));
        assert!(!no_peer_images.allows(content_type::IMAGE_PNG, &["other", "peer"]));
        assert!(no_peer_images.allows(content_type::HTML, &["peer"]));

        let no_local_files = with_peer(
            SyncContent {
                files: false,
                ..SyncContent::ALL
            },
            PeerSyncPolicy::default(),
        );
        assert!(!no_local_files.allows(content_type::FILE, &[]));
        assert!(!no_local_files.allows("application/x-future", &["peer"]));
        assert!(no_local_files.allows(content_type::TEXT, &["peer"]));
    }

    #[test]
    fn internal_records_follow_only_the_device_switch() {
        let disabled = with_peer(
            SyncContent::NONE,
            PeerSyncPolicy {
                enabled: false,
                ..PeerSyncPolicy::default()
            },
        );
        assert!(disabled.allows(content_type::ORGANIZATION, &[]));
        assert!(!disabled.allows(content_type::ORGANIZATION, &["peer"]));
        assert!(!disabled.allows(content_type::TEXT, &[]));
    }

    #[test]
    fn widening_names_only_what_was_turned_on() {
        let closed = with_peer(
            SyncContent::NONE,
            PeerSyncPolicy {
                enabled: false,
                ..PeerSyncPolicy::default()
            },
        );
        let open = DeviceSyncSettings::default();
        assert_eq!(
            open.widening_from(&closed),
            SyncWidening {
                this_device: true,
                devices: vec!["peer".into()],
            }
        );
        assert!(closed.widening_from(&open).is_empty());
    }

    #[test]
    fn a_pause_covers_its_scope_and_never_blocks_content() {
        let paused_peer = with_peer(
            SyncContent::ALL,
            PeerSyncPolicy {
                paused: true,
                ..PeerSyncPolicy::default()
            },
        );
        assert!(paused_peer.auto_sync_paused(Some("peer")));
        assert!(!paused_peer.auto_sync_paused(Some("other")));
        assert!(!paused_peer.auto_sync_paused(None));
        assert!(paused_peer.allows(content_type::TEXT, &["peer"]));
        assert!(paused_peer.pause_changed(&DeviceSyncSettings::default()));
        let everywhere = DeviceSyncSettings {
            paused: true,
            ..DeviceSyncSettings::default()
        };
        assert!(everywhere.auto_sync_paused(Some("other")));
        assert!(everywhere.auto_sync_paused(None));
    }
}
