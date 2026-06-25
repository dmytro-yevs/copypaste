//! Cross-crate frame-size constant equality tests (CopyPaste-w47w #1).
//!
//! `copypaste_sync::engine::MAX_FRAME_BYTES` and
//! `copypaste_p2p::transport::MAX_FRAME_BYTES` must be equal — both cap data
//! frames at 16 MiB so the P2P sync protocol and the P2P transport codec are
//! in lockstep.  Neither crate depends on the other, so the equality is verified
//! here in `copypaste-daemon`, which has both as dev-dependencies.
//!
//! The 64 KiB bootstrap-handshake cap (`copypaste_p2p::bootstrap::framing`)
//! is intentionally NOT tested here — it is a separate, independent limit for
//! PAKE messages, not a data-frame limit (see `MAX_HANDSHAKE_FRAME_BYTES`).

/// Compile-time assertion: the two 16 MiB frame caps are identical.
///
/// If either constant changes, this assertion fires and forces the developer
/// to update both sites together.
const _: () = assert!(
    copypaste_sync::engine::MAX_FRAME_BYTES == copypaste_p2p::transport::MAX_FRAME_BYTES,
    "MAX_FRAME_BYTES mismatch: copypaste-sync and copypaste-p2p must agree on the 16 MiB cap"
);

#[test]
fn max_frame_bytes_is_16_mib() {
    assert_eq!(
        copypaste_sync::engine::MAX_FRAME_BYTES,
        16 * 1024 * 1024,
        "copypaste_sync::engine::MAX_FRAME_BYTES must be 16 MiB"
    );
    assert_eq!(
        copypaste_p2p::transport::MAX_FRAME_BYTES,
        16 * 1024 * 1024,
        "copypaste_p2p::transport::MAX_FRAME_BYTES must be 16 MiB"
    );
}

#[test]
fn sync_and_p2p_frame_bytes_are_equal() {
    assert_eq!(
        copypaste_sync::engine::MAX_FRAME_BYTES,
        copypaste_p2p::transport::MAX_FRAME_BYTES,
        "the two 16 MiB frame caps must stay in sync"
    );
}
