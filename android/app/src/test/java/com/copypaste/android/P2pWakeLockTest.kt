package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-y4xa: P2P fragile to Doze/OEM-kill — no WakeLock, lifecycle tied to FGS.
 *
 * Root-cause: dialPairedPeer() in FgsSyncLoop performs a blocking mTLS dial and data
 * exchange under the CPU lock held only by the FGS notification. On OEM devices (Xiaomi,
 * Oppo, Samsung) the CPU can enter Doze mid-dial if the screen turns off, orphaning the
 * TLS handshake and causing the next restart to find an already-failed connection.
 *
 * Fix: FgsSyncLoop acquires a PARTIAL_WAKE_LOCK around each dialPairedPeer() call so the
 * CPU stays on for the duration of the active P2P sync. The lock is released in a
 * finally block to prevent leaks even on exception paths.
 *
 * Structural (source-scan) test — FgsSyncLoop requires an Android runtime to execute.
 */
class P2pWakeLockTest {

    private val fgsSource: String by lazy {
        val anchor = P2pWakeLockTest::class.java
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
            "src/main/java/com/copypaste/android/FgsSyncLoop.kt",
        ).readText()
    }

    /**
     * FgsSyncLoop must reference PowerManager.WakeLock (or PARTIAL_WAKE_LOCK) around
     * the P2P dial so the CPU cannot sleep mid-handshake.
     */
    @Test
    fun `FgsSyncLoop acquires WakeLock around P2P dial`() {
        assertTrue(
            "FgsSyncLoop must hold a WakeLock (PARTIAL_WAKE_LOCK) around dialPairedPeer",
            fgsSource.contains("WakeLock") || fgsSource.contains("PARTIAL_WAKE_LOCK"),
        )
    }

    /**
     * The WakeLock must be released in a finally block to prevent leaks on exceptions.
     */
    @Test
    fun `WakeLock is released in finally block`() {
        val dialBody = fgsSource
            .substringAfter("private suspend fun dialPairedPeer(")
            .substringBefore("private fun resolveAddrByIp(")

        assertTrue(
            "WakeLock release must be in a finally block in dialPairedPeer",
            dialBody.contains("finally") && (dialBody.contains("release()") || dialBody.contains("wakeLock")),
        )
    }
}
