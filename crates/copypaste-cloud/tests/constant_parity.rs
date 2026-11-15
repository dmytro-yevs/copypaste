//! Constants this crate must hold in step with another crate that cannot be
//! imported from here.

/// `copypaste_cloud::sync::pull::MAX_FUTURE_SKEW_MS` against
/// `copypaste_p2p::sync::plan::MAX_FUTURE_SKEW_MS`.
///
/// Both transports feed the same last-write-wins comparator, so a device that
/// refuses a stamp one way and accepts it the other converges on different
/// content per transport (manifest 05 §3.4, R-CLK-2).
#[test]
fn the_two_transports_refuse_the_same_future_stamps() {
    assert_eq!(
        copypaste_cloud::sync::MAX_FUTURE_SKEW_MS,
        copypaste_p2p::sync::MAX_FUTURE_SKEW_MS,
    );
}
