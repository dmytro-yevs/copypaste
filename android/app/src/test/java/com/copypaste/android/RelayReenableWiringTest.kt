package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-crh3.102: Android SyncBackend.RELAY outbound upload re-enablement.
 *
 * The relay cloud-upload path was disabled: MainActivity constructed the
 * [SyncManager] with `token = ""` and the relay push block was absent. This task
 * verifies the wiring that re-enables it:
 *   1. the server-issued relay token is persisted into [Settings.relayToken] at
 *      registration time (ensureRelayToken in both SyncManager and the SSE client),
 *   2. MainActivity passes `token = settings.relayToken` (not the dead `""`),
 *   3. ClipboardService.notifySyncManager contains a relay push block that fires
 *      via [SyncManager.pushToRelay] when the fan-out set includes RELAY.
 *
 * Structural (source-scan) tests — the runtime relay round-trip needs a live relay
 * and is verified on-device; here we lock the source wiring in place.
 */
class RelayReenableWiringTest {

    private fun moduleFile(relative: String): String {
        val anchor = RelayReenableWiringTest::class.java
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
        return java.io.File(moduleRoot, relative).readText()
    }

    private val mainActivitySource by lazy {
        moduleFile("src/main/java/com/copypaste/android/MainActivity.kt")
    }
    private val syncManagerSource by lazy {
        moduleFile("src/main/java/com/copypaste/android/SyncManager.kt")
    }
    private val relaySubSource by lazy {
        moduleFile("src/main/java/com/copypaste/android/RelaySubscriptionClient.kt")
    }
    private val clipboardServiceSource by lazy {
        moduleFile("src/main/java/com/copypaste/android/ClipboardService.kt")
    }

    /**
     * MainActivity must construct the SyncManager with the persisted relay token,
     * not the dead empty-string placeholder that disabled the relay path.
     */
    @Test
    fun `MainActivity constructs SyncManager with persisted relayToken`() {
        assertTrue(
            "MainActivity must pass token = settings.relayToken to SyncManager (crh3.102)",
            mainActivitySource.contains("token = settings.relayToken"),
        )
    }

    /**
     * The server-issued relay Device.token must be persisted into Settings at
     * registration time — both in the producer path (SyncManager.ensureRelayToken)
     * and the SSE subscribe path (RelaySubscriptionClient.ensureRelayToken).
     */
    @Test
    fun `relay token is persisted from registration in SyncManager`() {
        assertTrue(
            "SyncManager must persist the registered relay token to Settings.relayToken (crh3.102)",
            syncManagerSource.contains("relayToken = device.token"),
        )
    }

    @Test
    fun `relay token is persisted from registration in RelaySubscriptionClient`() {
        assertTrue(
            "RelaySubscriptionClient must persist the registered relay token to Settings.relayToken (crh3.102)",
            relaySubSource.contains("relayToken = device.token"),
        )
    }

    /**
     * notifySyncManager must contain a relay push block driven by the additive
     * fan-out set (SyncTransport.RELAY) and SyncManager.pushToRelay.
     */
    @Test
    fun `notifySyncManager has a relay push block gated on the fan-out set`() {
        val body = clipboardServiceSource
            .substringAfter("private suspend fun notifySyncManager(")
            .substringBefore("fun postCopyNotification(")
        assertTrue(
            "notifySyncManager must gate on SyncTransport.RELAY in the transport fan-out set (crh3.102)",
            body.contains("SyncTransport.RELAY in transports"),
        )
        assertTrue(
            "notifySyncManager must push via SyncManager.pushToRelay (crh3.102)",
            body.contains("pushToRelay("),
        )
    }
}
