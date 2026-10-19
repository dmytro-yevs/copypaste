//! The daemon name for the shared idle-activity cadence.
pub use copypaste_sync::cadence::AdaptiveCadence as Idle;
#[cfg(test)]
pub use copypaste_sync::cadence::{MAX_POLL_INTERVAL_WITHOUT_PUSH, MIN_POLL_INTERVAL};
