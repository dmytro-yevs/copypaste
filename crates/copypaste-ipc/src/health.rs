//! What the daemon reports about its own degraded state.
//!
//! Separate from [`crate::payload`] because it is not a reply to anything a
//! client asked for: it rides on `status` so that a screen learns its values
//! are not the user's own in the same round trip that fetches them.

use serde::{Deserialize, Serialize};

/// Which persisted settings did not survive the last read.
///
/// Field *names* only. A value that would not parse is exactly the value most
/// likely to be a fragment of a user's clipboard or a path, so none of it
/// crosses this boundary (AGENTS.md rule 4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct SettingsHealth {
    /// The whole record was unreadable, so every field fell back at once.
    pub record_unreadable: bool,
    /// Fields that were present but could not be read, in `ConfigData` order.
    pub unreadable_fields: Vec<String>,
}

impl SettingsHealth {
    /// Whether anything at all fell back.
    ///
    /// A health record is only ever built when something did, so this is the
    /// guard against constructing an empty one and reporting a degraded daemon
    /// that is fine.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.record_unreadable || !self.unreadable_fields.is_empty()
    }
}
