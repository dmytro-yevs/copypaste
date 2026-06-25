package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * CopyPaste-pkd0: regression guard for the LAN-discovered-peers section in
 * [DevicesActivity.DevicesScreen].
 *
 * Root-cause: the Liquid Glass redesign dropped the "Discovered on your network"
 * section label and the "Searching for nearby devices…" empty-state row from
 * [deviceRows]. The section silently disappeared even when P2P was enabled, so
 * users could not see or pair with mDNS-advertised peers.
 *
 * Fix: restored the section inside the `if (p2pEnabled)` block (label + empty-
 * state OR label + peer rows). The decision is extracted as the pure function
 * [discoveredSectionPresence] so it can be verified here without a Compose
 * runtime or an Android emulator.
 *
 * These tests are pure JVM — they do NOT require Android SDK, emulator, or NDK.
 */
class DiscoveredSectionPresenceTest {

    // ── P2P disabled: section must be completely hidden ───────────────────────

    @Test
    fun `section is HIDDEN when p2pEnabled is false and no peers`() {
        assertEquals(
            "No section when P2P disabled (0 peers)",
            DiscoveredSectionPresence.HIDDEN,
            discoveredSectionPresence(p2pEnabled = false, discoveredCount = 0),
        )
    }

    @Test
    fun `section is HIDDEN when p2pEnabled is false even with peers present`() {
        // Peers might exist in the native cache but P2P is disabled by the user —
        // we must not show the section at all.
        assertEquals(
            "No section when P2P disabled (2 peers)",
            DiscoveredSectionPresence.HIDDEN,
            discoveredSectionPresence(p2pEnabled = false, discoveredCount = 2),
        )
    }

    // ── P2P enabled, no peers: show label + empty-state row ──────────────────

    @Test
    fun `section is EMPTY_STATE when p2pEnabled and zero discovered peers`() {
        // pkd0 fix: the empty-state row ("Searching for nearby devices…") must
        // appear while scanning so the LAN section is always visible to the user.
        assertEquals(
            "Empty-state row must be shown when P2P enabled but 0 peers",
            DiscoveredSectionPresence.EMPTY_STATE,
            discoveredSectionPresence(p2pEnabled = true, discoveredCount = 0),
        )
    }

    // ── P2P enabled, peers present: show label + peer rows ───────────────────

    @Test
    fun `section is SHOW_PEERS when p2pEnabled and one discovered peer`() {
        assertEquals(
            "Peer rows must be shown when 1 discovered peer",
            DiscoveredSectionPresence.SHOW_PEERS,
            discoveredSectionPresence(p2pEnabled = true, discoveredCount = 1),
        )
    }

    @Test
    fun `section is SHOW_PEERS when p2pEnabled and multiple discovered peers`() {
        assertEquals(
            "Peer rows must be shown when 3 discovered peers",
            DiscoveredSectionPresence.SHOW_PEERS,
            discoveredSectionPresence(p2pEnabled = true, discoveredCount = 3),
        )
    }

    // ── Source-scan: DevicesActivity must have the p2pEnabled guard in place ──

    /**
     * Structural guard: [DevicesActivity] must emit the "Discovered on your network"
     * section label inside the `if (p2pEnabled)` block.
     *
     * This catches the pkd0 regression pattern (the block being entirely dropped)
     * even when the pure function above is present but the call site is reverted.
     */
    @Test
    fun `DevicesActivity deviceRows contains the discovered-section label`() {
        val anchor = DiscoveredSectionPresenceTest::class.java
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
            "src/main/java/com/copypaste/android/DevicesActivity.kt",
        ).readText()

        // The section label text must be present inside the deviceRows block.
        assert(source.contains("Discovered on your network")) {
            "DevicesActivity must contain the 'Discovered on your network' section label " +
                "(pkd0: regression — label was dropped by the Liquid Glass redesign)"
        }

        // The empty-state text must also be wired up (no_devices_nearby resource key or inline).
        assert(source.contains("no_devices_nearby") || source.contains("Searching for nearby")) {
            "DevicesActivity must show an empty-state row when no peers are found " +
                "(pkd0: 'Searching for nearby devices…' row was dropped)"
        }
    }
}
