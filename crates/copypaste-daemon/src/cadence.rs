//! The daemon name for the shared idle-activity cadence.

pub use copypaste_cloud::sync::AdaptiveCadence as Idle;
#[cfg(test)]
pub use copypaste_cloud::sync::MIN_POLL_INTERVAL;
