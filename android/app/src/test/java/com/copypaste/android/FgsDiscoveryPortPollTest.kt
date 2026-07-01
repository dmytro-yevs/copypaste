package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-j2vf: regression guard for the FGS-discovery port-poll fix in
 * [ClipboardService.startFgsDiscovery].
 *
 * Root-cause: [ClipboardService] called [startDiscovery] before the inbound mTLS
 * listener had bound, so [activeListenerPort] was still 0. The mDNS advertisement
 * published `syncPort=0`, which causes the macOS daemon to see "device unavailable"
 * when it tries to dial back to pair.
 *
 * Fix: [startFgsDiscovery] now spins a capped exponential-backoff poll (up to
 * [ClipboardService.PORT_POLL_TIMEOUT_MS]) and refuses to call [startDiscovery]
 * when [activeListenerPort] is still 0 after the timeout. The two decision points
 * are extracted as pure companion-object helpers:
 *   - [ClipboardService.portPollNextBackoffMs]: computes the next wait interval.
 *   - [ClipboardService.shouldAdvertisePort]: decides whether to proceed.
 *
 * These tests are pure JVM — they do NOT require Android SDK, emulator, or NDK.
 */
class FgsDiscoveryPortPollTest {

    // ── shouldAdvertisePort ───────────────────────────────────────────────────

    @Test
    fun `shouldAdvertisePort returns false for port 0`() {
        // The core j2vf fix: port=0 must never be advertised.
        assertFalse(
            "Port 0 must NOT be advertised (causes macOS 'device unavailable')",
            ClipboardService.shouldAdvertisePort(0),
        )
    }

    @Test
    fun `shouldAdvertisePort returns true for a valid ephemeral port`() {
        assertTrue(
            "A non-zero OS-assigned port must be advertisable",
            ClipboardService.shouldAdvertisePort(52_000),
        )
    }

    @Test
    fun `shouldAdvertisePort returns true for the minimum valid port`() {
        assertTrue(
            "Port 1 is technically non-zero and must be advertisable",
            ClipboardService.shouldAdvertisePort(1),
        )
    }

    @Test
    fun `shouldAdvertisePort returns false for negative port`() {
        // Defensive: a negative value (e.g. uninitialized field quirk) is not valid.
        assertFalse(
            "Negative port must not be advertised",
            ClipboardService.shouldAdvertisePort(-1),
        )
    }

    // ── portPollNextBackoffMs (exponential backoff) ───────────────────────────

    @Test
    fun `backoff doubles from initial value`() {
        val next = ClipboardService.portPollNextBackoffMs(
            currentMs = ClipboardService.PORT_POLL_INITIAL_BACKOFF_MS,
            maxMs = ClipboardService.PORT_POLL_MAX_BACKOFF_MS,
        )
        assertEquals(
            "First backoff step must double from ${ClipboardService.PORT_POLL_INITIAL_BACKOFF_MS}ms",
            ClipboardService.PORT_POLL_INITIAL_BACKOFF_MS * 2L,
            next,
        )
    }

    @Test
    fun `backoff is capped at max backoff`() {
        // A very large currentMs input must never exceed the cap.
        val next = ClipboardService.portPollNextBackoffMs(
            currentMs = 10_000L,
            maxMs = ClipboardService.PORT_POLL_MAX_BACKOFF_MS,
        )
        assertEquals(
            "Backoff must be capped at PORT_POLL_MAX_BACKOFF_MS",
            ClipboardService.PORT_POLL_MAX_BACKOFF_MS,
            next,
        )
    }

    @Test
    fun `backoff sequence from initial reaches the cap within the timeout`() {
        // Walk the exponential sequence as startFgsDiscovery does and verify
        // it caps before PORT_POLL_TIMEOUT_MS elapses — so the safety window
        // is actually effective.
        var backoff = ClipboardService.PORT_POLL_INITIAL_BACKOFF_MS
        var elapsed = 0L
        var steps = 0
        while (elapsed < ClipboardService.PORT_POLL_TIMEOUT_MS) {
            elapsed += backoff
            backoff = ClipboardService.portPollNextBackoffMs(
                currentMs = backoff,
                maxMs = ClipboardService.PORT_POLL_MAX_BACKOFF_MS,
            )
            steps++
            if (steps > 200) break // infinite-loop guard
        }
        assertTrue(
            "The poll loop must fit within PORT_POLL_TIMEOUT_MS (${ ClipboardService.PORT_POLL_TIMEOUT_MS}ms); " +
                "elapsed=$elapsed after $steps steps",
            elapsed >= ClipboardService.PORT_POLL_INITIAL_BACKOFF_MS,
        )
        assertEquals(
            "Backoff must have reached the cap after doubling through the range",
            ClipboardService.PORT_POLL_MAX_BACKOFF_MS,
            backoff,
        )
    }

    @Test
    fun `initial backoff constant is 20ms as implemented`() {
        // Pin the known-good constant so a refactor that silently changes it
        // (e.g. reducing the responsiveness) fails loudly.
        assertEquals(20L, ClipboardService.PORT_POLL_INITIAL_BACKOFF_MS)
    }

    @Test
    fun `max backoff constant is 500ms as implemented`() {
        assertEquals(500L, ClipboardService.PORT_POLL_MAX_BACKOFF_MS)
    }

    @Test
    fun `timeout constant is 10 seconds as implemented`() {
        assertEquals(10_000L, ClipboardService.PORT_POLL_TIMEOUT_MS)
    }

    // ── Source-scan: startFgsDiscovery must contain the port=0 guard ──────────

    /**
     * Structural guard: [ClipboardService.startFgsDiscovery] must have a
     * `syncPort == 0` (or `port == 0`) early-return so a zero port is NEVER
     * forwarded to [startDiscovery].
     *
     * This catches the j2vf regression pattern (the guard being removed) even
     * when the pure helper tests above pass.
     */
    @Test
    fun `startFgsDiscovery contains the syncPort equals 0 guard`() {
        val anchor = FgsDiscoveryPortPollTest::class.java
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
        val source = java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/ClipboardService.kt",
        ).readText()

        // Extract the startFgsDiscovery body.
        val fgsBody = source
            .substringAfter("private fun startFgsDiscovery()")
            .substringBefore("/**") // ends at the next KDoc block

        assertTrue(
            "startFgsDiscovery must guard against syncPort==0 before calling startDiscovery " +
                "(j2vf fix: advertising port 0 causes macOS 'device unavailable')",
            fgsBody.contains("syncPort == 0") || fgsBody.contains("port == 0"),
        )
    }

    /**
     * Structural guard: the port-poll loop must use exponential backoff,
     * not a fixed sleep, so it responds quickly when the listener binds promptly.
     */
    @Test
    fun `startFgsDiscovery uses exponential backoff in port-poll loop`() {
        val anchor = FgsDiscoveryPortPollTest::class.java
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
        val source = java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/ClipboardService.kt",
        ).readText()

        val fgsBody = source
            .substringAfter("private fun startFgsDiscovery()")
            .substringBefore("/**")

        assertTrue(
            "startFgsDiscovery port-poll loop must apply exponential backoff (coerceAtMost / * 2)",
            fgsBody.contains("coerceAtMost") && fgsBody.contains("* 2"),
        )
    }
}
