//! Shared keyset-pagination cursor (CopyPaste-w47w #3).
//!
//! Both the Supabase cloud poll path (`cloud::poll::PollCursor`) and the relay
//! receive path (`relay::watermark::Watermark`) advance a forward-pagination
//! cursor of the shape `(wall_time, id)`.  The two sites use different concrete
//! types for the fields (`i64`/`String` vs `u64`/`i64`) because the underlying
//! transports number their rows differently, so a single concrete struct cannot
//! serve both.  Instead, `KeysetCursor<W, I>` is a generic two-field cursor with
//! the `W`all and `I`d type parameters left to the caller.
//!
//! ## Type aliases used by callers
//!
//! | Alias | W | I | Used by |
//! |---|---|---|---|
//! | `CloudCursor` | `i64` | `String` | `cloud::poll` (Supabase row ids are strings) |
//! | `RelayCursor` | `u64` | `i64` | `relay::watermark` (relay row ids are integers) |
//!
//! The aliases keep the call-site names short while the shared struct captures
//! the structural invariant in one place.

use serde::{Deserialize, Serialize};

/// Generic `(wall_time, id)` keyset-pagination cursor.
///
/// Advancing this cursor on each ingested row keeps the poll/receive loops
/// paginating strictly forward, preventing silent re-downloads and stalls when
/// multiple rows share the same `wall_time` millisecond.
///
/// # Type parameters
/// * `W` — wall-time type (`i64` for Supabase rows; `u64` for relay rows).
/// * `I` — row-id type (`String` for Supabase; `i64` for the relay server).
///
/// `Copy` is NOT derived on the struct because `CloudCursor` uses `String` for
/// `I`, which is not `Copy`. A blanket `impl Copy` is provided for
/// `KeysetCursor<W, I>` whenever both `W: Copy` and `I: Copy`, so
/// `RelayCursor` is `Copy` while `CloudCursor` is not.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KeysetCursor<W, I> {
    /// Unix-millisecond wall time of the last ingested row.
    pub(crate) wall: W,
    /// Row id of the last ingested row (secondary keyset component).
    pub(crate) id: I,
}

// Blanket Copy impl: only when both type parameters are Copy.
// This makes `RelayCursor` (KeysetCursor<u64, i64>) implicitly Copy,
// preserving the Copy behaviour that `Watermark` previously had.
// `CloudCursor` (KeysetCursor<i64, String>) is NOT Copy because String is not.
impl<W: Copy, I: Copy> Copy for KeysetCursor<W, I> {}

/// Keyset cursor for the Supabase cloud poll path.
///
/// `wall` is `i64` because Supabase `wall_time` is a signed bigint; `id` is
/// `String` because Supabase row ids may be UUIDs or numeric strings from JSON.
/// Not serialised — the cloud path persists only `wall` (as a plain `i64`)
/// into the `settings` table; the `id` component is reconstructed on restart
/// from the first ingest.
pub(crate) type CloudCursor = KeysetCursor<i64, String>;

/// Keyset cursor for the relay receive path.
///
/// `wall` is `u64` because the relay server's `wall_time` field is unsigned;
/// `id` is `i64` because the relay assigns sequential integer row ids.
/// Serialised to `relay_watermark.json` so a daemon restart resumes forward
/// from the last-persisted position.
pub(crate) type RelayCursor = KeysetCursor<u64, i64>;

#[cfg(test)]
mod tests {
    use super::*;

    /// `KeysetCursor::default()` zeroes all fields for both alias types.
    #[test]
    fn default_is_zero() {
        let c = CloudCursor::default();
        assert_eq!(c.wall, 0);
        assert!(c.id.is_empty());

        let r = RelayCursor::default();
        assert_eq!(r.wall, 0);
        assert_eq!(r.id, 0);
    }

    /// `PartialEq` works correctly for `RelayCursor`.
    #[test]
    fn relay_cursor_equality() {
        let a = RelayCursor { wall: 1000, id: 42 };
        let b = RelayCursor { wall: 1000, id: 42 };
        let c = RelayCursor { wall: 1001, id: 42 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    /// `PartialEq` works correctly for `CloudCursor`.
    #[test]
    fn cloud_cursor_equality() {
        let a = CloudCursor {
            wall: 500,
            id: "abc".to_owned(),
        };
        let b = CloudCursor {
            wall: 500,
            id: "abc".to_owned(),
        };
        let c = CloudCursor {
            wall: 500,
            id: "xyz".to_owned(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    /// `RelayCursor` round-trips through JSON (it is persisted to disk).
    #[test]
    fn relay_cursor_serde_roundtrip() {
        let original = RelayCursor {
            wall: 1_700_000_000_000,
            id: 9999,
        };
        let json = serde_json::to_string(&original).expect("serialise");
        let parsed: RelayCursor = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(original, parsed);
    }
}
