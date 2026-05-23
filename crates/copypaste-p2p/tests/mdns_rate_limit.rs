//! Per-IP token-bucket rate-limit tests for the mDNS event gate.
//!
//! Mitigates THREAT-MODEL OI-3 (mDNS responder flood). Tests use
//! `tokio::time::pause()` + `advance()` for deterministic, instant runs.

use std::net::IpAddr;
use std::time::Duration;

use copypaste_p2p::rate_limit::{
    MdnsRateLimiter, BURST_CAPACITY, CLEANUP_IDLE, REFILL_RATE_PER_SEC,
};

fn ip(s: &str) -> IpAddr {
    s.parse().expect("valid IP literal in test")
}

#[tokio::test(start_paused = true)]
async fn bursts_of_10_pass_through() {
    let rl = MdnsRateLimiter::new();
    let addr = ip("192.168.1.42");

    for i in 0..BURST_CAPACITY {
        assert!(
            rl.try_admit(addr),
            "token #{i} should be admitted within burst capacity"
        );
    }
    assert_eq!(rl.total_drops(), 0, "no drops within burst window");
}

#[tokio::test(start_paused = true)]
async fn eleventh_query_dropped() {
    let rl = MdnsRateLimiter::new();
    let addr = ip("192.168.1.43");

    // Drain the bucket.
    for _ in 0..BURST_CAPACITY {
        assert!(rl.try_admit(addr));
    }
    // The next call (#11) must be denied — bucket exhausted, no time advanced.
    assert!(!rl.try_admit(addr), "11th query must be dropped");
    assert_eq!(rl.total_drops(), 1, "drop counter incremented");
}

#[tokio::test(start_paused = true)]
async fn refill_after_500ms_allows_1_more() {
    let rl = MdnsRateLimiter::new();
    let addr = ip("192.168.1.44");

    // Empty the bucket.
    for _ in 0..BURST_CAPACITY {
        assert!(rl.try_admit(addr));
    }
    assert!(!rl.try_admit(addr));

    // Advance virtual clock by 500ms — at 2 tokens/sec that is exactly 1 token.
    // We advance a hair past 500ms to absorb float-rounding (refill uses f64).
    tokio::time::advance(Duration::from_millis(501)).await;

    assert!(
        rl.try_admit(addr),
        "after 500ms one token should be refilled at {} tok/s",
        REFILL_RATE_PER_SEC
    );
    // And only one — the bucket is empty again.
    assert!(!rl.try_admit(addr), "no second token after only 500ms");
}

#[tokio::test(start_paused = true)]
async fn different_ips_have_independent_buckets() {
    let rl = MdnsRateLimiter::new();
    let alice = ip("10.0.0.1");
    let bob = ip("10.0.0.2");

    // Alice exhausts her bucket.
    for _ in 0..BURST_CAPACITY {
        assert!(rl.try_admit(alice));
    }
    assert!(!rl.try_admit(alice), "alice should be throttled");

    // Bob is untouched — full burst still available.
    for i in 0..BURST_CAPACITY {
        assert!(
            rl.try_admit(bob),
            "bob token #{i} should not be affected by alice"
        );
    }

    assert_eq!(rl.tracked_ip_count(), 2, "both IPs tracked separately");
}

#[tokio::test(start_paused = true)]
async fn cleanup_removes_old_entries() {
    let rl = MdnsRateLimiter::new();
    let stale = ip("10.0.0.10");
    let fresh = ip("10.0.0.11");

    // Touch both IPs so both buckets exist.
    assert!(rl.try_admit(stale));
    assert!(rl.try_admit(fresh));
    assert_eq!(rl.tracked_ip_count(), 2);

    // Advance past the idle threshold for `stale`…
    tokio::time::advance(CLEANUP_IDLE - Duration::from_secs(1)).await;
    // …then re-touch `fresh` so its `last_used` is recent.
    assert!(rl.try_admit(fresh));
    // …then advance just past `CLEANUP_IDLE` so `stale` is now idle but
    // `fresh` is not.
    tokio::time::advance(Duration::from_secs(2)).await;

    rl.cleanup_now();
    assert_eq!(
        rl.tracked_ip_count(),
        1,
        "stale bucket should be reaped, fresh bucket should remain"
    );

    // The remaining bucket should be `fresh` — verify by exhausting it from
    // a known-mostly-full state. After two admits we should still have plenty
    // of headroom (BURST_CAPACITY - 2 left), so the next admit must succeed.
    assert!(rl.try_admit(fresh), "fresh bucket should still admit");
}

#[tokio::test(start_paused = true)]
async fn sustained_2_per_second_passes_long_term() {
    // OI-3 sanity check: a legitimate peer responding at the long-term refill
    // rate (2/sec) should never be throttled past the initial burst.
    let rl = MdnsRateLimiter::new();
    let addr = ip("10.0.0.99");

    // Burn the burst.
    for _ in 0..BURST_CAPACITY {
        assert!(rl.try_admit(addr));
    }

    // Now request 1 token every 500ms — exactly the refill rate.
    // 20 iterations = 10s of simulated traffic.
    for i in 0..20 {
        tokio::time::advance(Duration::from_millis(501)).await;
        assert!(
            rl.try_admit(addr),
            "sustained 2/sec traffic should be admitted forever (iter {i})"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn full_refill_caps_at_burst_capacity() {
    // After a long idle, the bucket must not exceed BURST_CAPACITY tokens —
    // otherwise an attacker could "save up" infinite bursts by idling.
    let rl = MdnsRateLimiter::new();
    let addr = ip("10.0.0.100");

    // Drain.
    for _ in 0..BURST_CAPACITY {
        assert!(rl.try_admit(addr));
    }
    assert!(!rl.try_admit(addr));

    // Idle for 1 hour — refill would compute to 7200 tokens uncapped.
    tokio::time::advance(Duration::from_secs(3600)).await;

    // Should admit exactly BURST_CAPACITY, then drop.
    for i in 0..BURST_CAPACITY {
        assert!(rl.try_admit(addr), "post-idle burst #{i} should pass");
    }
    assert!(
        !rl.try_admit(addr),
        "cap must clamp post-idle burst at BURST_CAPACITY = {BURST_CAPACITY}"
    );
}
