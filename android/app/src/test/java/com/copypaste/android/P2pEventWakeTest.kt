package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-mip2: Event-driven opportunistic P2P dial — wake mechanism smoke test.
 *
 * The fix replaces the plain `delay(chunk)` in FgsSyncLoop's inner P2P-sleep loop
 * with `withTimeoutOrNull(chunk) { p2pWakeChannel.receive() }`, so the loop can
 * wake early on a signal (capture/mDNS peer discovery) OR fall through on timeout.
 *
 * These are structural (source-scan) tests — they verify the presence of the wake
 * mechanism and its trigger sites in the Kotlin source, without needing an Android
 * runtime or coroutine harness (which would require robolectric/coroutines-test).
 *
 * Pure JVM: runs via `./gradlew :app:testDebugUnitTest`.
 */
class P2pEventWakeTest {

    private val fgsSource: String by lazy {
        readModuleSource("FgsSyncLoop.kt")
    }

    private val serviceSource: String by lazy {
        readModuleSource("ClipboardService.kt")
    }

    private fun readModuleSource(fileName: String): String {
        val anchor = P2pEventWakeTest::class.java
            .protectionDomain?.codeSource?.location?.toURI()
            ?.let { java.io.File(it) }
        var dir: java.io.File? = anchor
        var moduleRoot: java.io.File? = null
        while (dir != null) {
            if (java.io.File(dir, "src/main").exists()) {
                moduleRoot = dir
                break
            }
            dir = dir.parentFile
        }
        requireNotNull(moduleRoot) { "Could not locate module root from $anchor" }
        return java.io.File(moduleRoot, "src/main/java/com/copypaste/android/$fileName").readText()
    }

    // -------------------------------------------------------------------------
    // FgsSyncLoop wake mechanism
    // -------------------------------------------------------------------------

    /**
     * FgsSyncLoop must declare a CONFLATED channel (or MutableSharedFlow with
     * extraBufferCapacity=0/replay=1) used as the P2P wake signal.
     *
     * We look for Channel.CONFLATED or Channel(Channel.CONFLATED) — either form
     * is acceptable; what matters is that a Channel-based wake exists.
     */
    @Test
    fun `FgsSyncLoop declares a CONFLATED wake channel`() {
        assertTrue(
            "FgsSyncLoop must declare a Channel with Channel.CONFLATED for P2P wake",
            fgsSource.contains("Channel.CONFLATED") || fgsSource.contains("CONFLATED"),
        )
    }

    /**
     * FgsSyncLoop must expose a public signalP2pWake() (or similarly named)
     * method that the service can call after capture or mDNS discovery.
     */
    @Test
    fun `FgsSyncLoop exposes signalP2pWake method`() {
        assertTrue(
            "FgsSyncLoop must have a signalP2pWake() method",
            fgsSource.contains("fun signalP2pWake"),
        )
    }

    /**
     * The inner P2P sleep must use withTimeoutOrNull (not a bare delay) so it
     * can be interrupted by the wake signal.
     */
    @Test
    fun `FgsSyncLoop inner P2P sleep uses withTimeoutOrNull instead of bare delay`() {
        // Extract the inner-loop body — starts after the p2pChunk computation
        // and before "FgsSyncLoop stopped".
        val innerLoop = fgsSource
            .substringAfter("var remaining = nextDelay")
            .substringBefore("FgsSyncLoop stopped")

        assertTrue(
            "Inner P2P wait must use withTimeoutOrNull to allow wake interruption",
            innerLoop.contains("withTimeoutOrNull"),
        )
        // Bare delay(chunk) must NOT appear in the inner wait — it was the old behaviour.
        // We allow delay() elsewhere in the loop body (e.g. backoff paths) but the chunk
        // sleep itself must go through withTimeoutOrNull.
        assertFalse(
            "Inner P2P wait must NOT use bare delay(chunk) — that prevented wake interruption",
            innerLoop.contains("delay(chunk)"),
        )
    }

    /**
     * The debounce constant must be defined as a named const near the other
     * P2P interval constants. Hardcoded magic numbers are forbidden.
     */
    @Test
    fun `FgsSyncLoop defines P2P_WAKE_DEBOUNCE_MS as a named const`() {
        assertTrue(
            "FgsSyncLoop must define P2P_WAKE_DEBOUNCE_MS as a named constant",
            fgsSource.contains("P2P_WAKE_DEBOUNCE_MS"),
        )
    }

    // -------------------------------------------------------------------------
    // ClipboardService trigger sites
    // -------------------------------------------------------------------------

    /**
     * ClipboardService must call signalP2pWake() after a successful clipboard
     * capture (storedId.isNotEmpty() path). This covers the "copy on Android"
     * fast-path.
     */
    @Test
    fun `ClipboardService signals P2P wake after successful capture`() {
        assertTrue(
            "ClipboardService must call fgsSyncLoop.signalP2pWake() (or equivalent) on capture",
            serviceSource.contains("signalP2pWake"),
        )
    }

    /**
     * ClipboardService must start a mDNS-peer watch coroutine that fires
     * signalP2pWake when a new peer is discovered, so a freshly-discovered
     * macOS peer triggers an immediate dial.
     */
    @Test
    fun `ClipboardService watches for mDNS peer discovery and signals wake`() {
        assertTrue(
            "ClipboardService must watch for mDNS peer discovery and signal P2P wake",
            serviceSource.contains("signalP2pWake") &&
                (serviceSource.contains("listDiscovered") || serviceSource.contains("mDNS")),
        )
    }
}
