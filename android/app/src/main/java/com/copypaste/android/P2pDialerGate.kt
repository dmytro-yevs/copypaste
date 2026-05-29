package com.copypaste.android

/**
 * Pure decision logic for the background Android→macOS P2P dialer.
 *
 * Kept free of Android/Compose/FFI types so it can be unit-tested on the plain
 * JVM (`src/test`) without an emulator. [FgsSyncLoop] evaluates [shouldDial] on
 * each tick and dials the persisted peer only when all three credentials are
 * present.
 */
object P2pDialerGate {

    /**
     * The background dialer should attempt a sync only when ALL of the persisted
     * pairing credentials are present and usable:
     *  - a non-blank peer sync address (host:port),
     *  - a non-blank peer fingerprint to pin, and
     *  - a non-empty 32-byte PAKE session key.
     *
     * Any blank/empty value means the device was never paired (or the secret
     * could not be unwrapped), so dialing would fail before any network I/O.
     */
    fun shouldDial(
        peerSyncAddr: String,
        peerFingerprint: String,
        sessionKey: ByteArray,
    ): Boolean =
        peerSyncAddr.isNotBlank() &&
            peerFingerprint.isNotBlank() &&
            sessionKey.isNotEmpty()

    /**
     * Next delay before the following dial tick.
     *
     * - After a failure (or any tick where the gate was closed) we slow down to
     *   [errorBackoffMs] to avoid hammering an unreachable peer / churning the
     *   battery while unpaired.
     * - After a tick where dialing was attempted and did not error we use the
     *   normal [normalIntervalMs] cadence.
     */
    fun nextDelayMs(
        attemptedAndSucceeded: Boolean,
        normalIntervalMs: Long,
        errorBackoffMs: Long,
    ): Long = if (attemptedAndSucceeded) normalIntervalMs else errorBackoffMs
}
