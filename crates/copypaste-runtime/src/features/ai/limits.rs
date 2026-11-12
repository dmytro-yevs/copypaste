//! Per-grant rate limits, per-session read volume and response bounds.

use std::collections::HashMap;

/// Per-grant token bucket: calls allowed in one burst window.
pub(super) const BURST_CALLS: u32 = 30;
pub(super) const BURST_WINDOW_SECS: u64 = 10;
pub(super) const HOURLY_CALLS: u32 = 600;

/// Content one session may read before the desktop must confirm more; each
/// approval grants this much again.
pub(super) const SESSION_READ_ALLOWANCE_BYTES: u64 = 2 * 1024 * 1024;
/// Text one response may carry in total.
pub(super) const MAX_RESPONSE_BYTES: usize = 64 * 1024;
/// Characters of one full-text chunk.
pub(super) const CHUNK_CHARS: usize = 16_000;
/// Characters of one list or search preview.
pub(super) const PREVIEW_CHARS: usize = 200;
pub(super) const DEFAULT_LIMIT: u32 = 10;
pub(super) const MAX_LIMIT: u32 = 50;
/// The largest text `copy_to_clipboard` accepts.
pub(super) const MAX_COPY_BYTES: usize = 1024 * 1024;
/// Longest edge of an image handed to a model.
pub(super) const IMAGE_EDGE: u32 = 1568;
/// Rate buckets kept; the least recently used goes first.
const MAX_BUCKETS: usize = 64;

/// Both buckets of one grant, as fractional tokens.
struct Bucket {
    burst: f64,
    hourly: f64,
    at_ms: i64,
}

#[derive(Default)]
pub(super) struct Buckets(HashMap<String, Bucket>);

impl Buckets {
    /// Takes one call from `grant_id`'s buckets, or answers the whole
    /// seconds until one is available.
    pub(super) fn take(&mut self, grant_id: &str, now_ms: i64) -> Result<(), u32> {
        if !self.0.contains_key(grant_id) && self.0.len() >= MAX_BUCKETS {
            if let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, bucket)| bucket.at_ms)
                .map(|(id, _)| id.clone())
            {
                self.0.remove(&oldest);
            }
        }
        let bucket = self.0.entry(grant_id.to_owned()).or_insert(Bucket {
            burst: f64::from(BURST_CALLS),
            hourly: f64::from(HOURLY_CALLS),
            at_ms: now_ms,
        });
        let burst_rate = f64::from(BURST_CALLS) / (BURST_WINDOW_SECS as f64 * 1000.0);
        let hourly_rate = f64::from(HOURLY_CALLS) / 3_600_000.0;
        let elapsed = now_ms.saturating_sub(bucket.at_ms).max(0) as f64;
        bucket.at_ms = now_ms;
        bucket.burst = (bucket.burst + elapsed * burst_rate).min(f64::from(BURST_CALLS));
        bucket.hourly = (bucket.hourly + elapsed * hourly_rate).min(f64::from(HOURLY_CALLS));
        if bucket.burst >= 1.0 && bucket.hourly >= 1.0 {
            bucket.burst -= 1.0;
            bucket.hourly -= 1.0;
            return Ok(());
        }
        let wait_ms = ((1.0 - bucket.burst).max(0.0) / burst_rate)
            .max((1.0 - bucket.hourly).max(0.0) / hourly_rate);
        Err(((wait_ms / 1000.0).ceil() as u32).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thirty_first_call_in_ten_seconds_waits_and_the_bucket_refills() {
        let mut buckets = Buckets::default();
        for call in 0..BURST_CALLS {
            assert!(buckets.take("g", 1_000 + i64::from(call)).is_ok());
        }
        let retry = buckets.take("g", 1_100).unwrap_err();
        assert!((1..=1).contains(&retry), "{retry}");
        assert!(buckets.take("other", 1_100).is_ok(), "per grant");
        assert!(buckets.take("g", 1_100 + 400).is_ok());
    }
}
