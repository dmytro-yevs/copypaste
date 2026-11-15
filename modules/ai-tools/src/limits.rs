//! Per-grant rate limits, per-connection read volume and response bounds.
use std::collections::HashMap;

pub(crate) const BURST_CALLS: u32 = 30;
pub(crate) const BURST_WINDOW_SECS: u64 = 10;
pub(crate) const HOURLY_CALLS: u32 = 600;
/// Content one connection may read before the user must allow more; each
/// approval adds this much again.
pub(crate) const SESSION_READ_ALLOWANCE_BYTES: u64 = 2 * 1024 * 1024;
pub(crate) const MAX_RESPONSE_BYTES: usize = 64 * 1024;
pub(crate) const CHUNK_CHARS: usize = 16_000;
pub(crate) const PREVIEW_CHARS: usize = 200;
pub(crate) const DEFAULT_LIMIT: u32 = 10;
pub(crate) const MAX_LIMIT: u32 = 50;
pub(crate) const MAX_COPY_BYTES: usize = 1024 * 1024;
/// An image larger than this is described, not sent, so a reply stays
/// within one endpoint frame.
pub(crate) const MAX_IMAGE_BASE64_BYTES: usize = 640 * 1024;
pub(crate) const MAX_LABEL_CHARS: usize = 64;
/// How long an approval waits for the user; no answer denies.
pub(crate) const PROMPT_TIMEOUT_SECS: u32 = 120;
pub(crate) const DENIAL_WINDOW_MS: i64 = 10 * 60 * 1000;
pub(crate) const DENIALS_TO_BLOCK: usize = 3;
pub(crate) const MAX_DENIED_CLIENTS: usize = 64;
pub(crate) const MAX_CLIENT_CHARS: usize = 128;
pub(crate) const MAX_GRANTS: usize = 32;
const MAX_BUCKETS: usize = 64;

struct Bucket {
    burst: f64,
    hourly: f64,
    at_ms: i64,
}

#[derive(Default)]
pub(crate) struct Buckets(HashMap<String, Bucket>);

impl Buckets {
    /// Takes one call from `grant_id`'s buckets, or answers the whole
    /// seconds until one is available.
    pub(crate) fn take(&mut self, grant_id: &str, now_ms: i64) -> Result<(), u32> {
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

    pub(crate) fn forget(&mut self, grant_id: &str) {
        self.0.remove(grant_id);
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
        assert_eq!(retry, 1);
        assert!(buckets.take("other", 1_100).is_ok(), "per grant");
        assert!(buckets.take("g", 1_100 + 400).is_ok());
    }
}
