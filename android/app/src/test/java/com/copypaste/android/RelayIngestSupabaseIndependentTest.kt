package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-crh3.112: Android relay SSE/poll ingestion silently no-ops when
 * Supabase is NOT configured — relay-only users receive nothing.
 *
 * Root cause: [SyncManager.ingestRelaySseItem] obtained the decryption key via
 * [SyncManager.resolveSyncContext], which short-circuits to null whenever
 * `!settings.isSupabaseConfigured` (it builds a SupabaseClient + bearer that the
 * relay decrypt path never uses). So a relay-only install (relay configured,
 * Supabase absent) dropped EVERY received item before decryption.
 *
 * Fix: the relay ingest path must resolve the cross-device sync key directly via
 * [SyncManager.resolveCloudSyncKey] — the SAME Supabase-independent source that
 * [SyncManager.relayRegistration] and [SyncManager.pushToRelay] already use — so
 * relay receive works independent of Supabase configuration.
 *
 * Structural (source-scan) test — the FFI decrypt path is not JVM-runnable, so we
 * assert on the source of [ingestRelaySseItem] the way [RelayIngestLwwTest] does.
 */
class RelayIngestSupabaseIndependentTest {

    private val syncManagerSource: String by lazy {
        val anchor = RelayIngestSupabaseIndependentTest::class.java
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
        java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/SyncManager.kt",
        ).readText()
    }

    /** Body of ingestRelaySseItem, up to the next function. */
    private val ingestBody: String by lazy {
        syncManagerSource
            .substringAfter("suspend fun ingestRelaySseItem(")
            .substringBefore("fun relayRegistration(")
    }

    /**
     * The relay ingest path must NOT obtain its decryption key via
     * resolveSyncContext(), which gates on isSupabaseConfigured. Doing so makes a
     * relay-only install drop all received items.
     */
    @Test
    fun `ingestRelaySseItem does not gate on resolveSyncContext`() {
        assertFalse(
            "ingestRelaySseItem must not use resolveSyncContext() — it requires " +
                "Supabase to be configured, breaking relay-only receive (crh3.112)",
            ingestBody.contains("resolveSyncContext"),
        )
    }

    /**
     * The relay ingest path must resolve the sync key via the Supabase-independent
     * resolveCloudSyncKey — the same source relayRegistration/pushToRelay use.
     */
    @Test
    fun `ingestRelaySseItem resolves the sync key via resolveCloudSyncKey`() {
        assertTrue(
            "ingestRelaySseItem must resolve the cross-device key via " +
                "resolveCloudSyncKey (Supabase-independent) so relay-only receive works (crh3.112)",
            ingestBody.contains("resolveCloudSyncKey"),
        )
    }
}
