#![forbid(unsafe_code)]

use std::time::Duration;

use backon::ExponentialBuilder;

pub const STREAM_RECONNECT_MIN: Duration = Duration::from_secs(5);
pub const STREAM_RECONNECT_MAX: Duration = Duration::from_secs(300);

#[must_use]
pub fn stream_reconnect_backoff() -> ExponentialBuilder {
    ExponentialBuilder::new()
        .with_min_delay(STREAM_RECONNECT_MIN)
        .with_max_delay(STREAM_RECONNECT_MAX)
        .without_max_times()
}

#[cfg(test)]
mod tests {
    use backon::BackoffBuilder as _;

    use super::*;

    #[test]
    fn stream_reconnect_schedule_is_exact_bounded_and_continuous() {
        let delays: Vec<_> = stream_reconnect_backoff().build().take(9).collect();
        assert_eq!(
            delays,
            [5, 10, 20, 40, 80, 160, 300, 300, 300].map(Duration::from_secs)
        );
    }
}
