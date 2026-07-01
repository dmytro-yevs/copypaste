//! Clock-skew security ceilings.
//!
//! These bounds prevent a hostile or buggy peer from jamming the local
//! Lamport clock or wall-time tie-break via `LamportClock::observe()`.

/// Maximum number of Lamport ticks a remote item is allowed to be ahead of the
/// local clock before its timestamp is clamped.
///
/// Why this bound? A real deployment might have thousands of devices, each
/// performing thousands of writes per day over years of uptime. 10^12 ticks is
/// larger than (10^6 devices × 10^6 writes each), so it accommodates any
/// realistic scenario while still preventing a single hostile/buggy peer from
/// jamming the local clock to u64::MAX (which would make that peer win every
/// future LWW conflict forever). The wall_time is a Unix-ms timestamp; 10^12 ms
/// is roughly 31.7 years in the future (1 year ≈ 3.156 × 10^10 ms), a similarly
/// generous but finite bound.
pub const MAX_LAMPORT_SKEW: u64 = 1_000_000_000_000; // 10^12 ticks
pub const MAX_WALL_TIME_SKEW_MS: i64 = 1_000_000_000_000_i64; // 10^12 ms ≈ 31.7 years
