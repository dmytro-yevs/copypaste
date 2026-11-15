//! Bounded upload-page results across the provider boundary.
use crate::{
    unreadable::{UnreadableUploads, UploadFloor},
    LocalItem,
};
use serde::{Deserialize, Serialize};
/// What the last upload scan was actually shown.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Offer {
    /// The scan hit the upload scan limit, so there is more behind it.
    pub truncated: bool,
    /// The last offered row's full keyset — the furthest the floor may move
    /// when the scan was truncated.
    pub last: UploadFloor,
    /// The cursor this scan started from. It lets completion detect a peer
    /// write that lowered the floor while the round was in flight.
    pub started: UploadFloor,
    /// Detects a write that landed at the same keyset boundary, where comparing
    /// only the floor pair cannot prove the scan saw it.
    pub started_epoch: u64,
}

/// The result of one upload scan.
#[derive(Serialize, Deserialize)]
pub struct Scan {
    pub items: Vec<LocalItem>,
    pub offer: Offer,
    pub unreadable: UnreadableUploads,
}
