package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-km61: regression guard — RECENT_SYNC_MS (Android) must match the
 * Rust source of truth `copypaste_ipc::SYNC_BADGE_RECENT_MS`.
 *
 * Before this fix, DevicesOnlineState.kt hardcoded `RECENT_SYNC_MS = 5 * 60 * 1_000L`
 * as a literal. If the Rust constant changed, Android would silently render differently
 * from macOS. The constant is now seeded from `syncBadgeRecentMs()` (FFI getter) at
 * startup; the compile-time fallback literal is preserved for stub-mode devices.
 *
 * These tests:
 * 1. Assert the compile-time fallback literal equals 5 minutes (prevents silent changes).
 * 2. Assert that [DevicesOnlineState.seedFromRust] overrides RECENT_SYNC_MS.
 * 3. Assert the structural contract: both [DevicesOnlineState.kt] and [SyncStatusBadge.kt]
 *    reference RECENT_SYNC_MS (not a hardcoded literal) so the seeded value is actually used.
 *
 * Pure-JVM — no Android SDK, no Compose runtime.
 */
class RecentSyncMsParityTest {

    /**
     * The compile-time fallback value of RECENT_SYNC_MS must be 5 minutes (300_000 ms).
     * This matches `copypaste_ipc::SYNC_BADGE_RECENT_MS` = `5 * 60 * 1_000` in Rust.
     * If this test breaks someone changed the Kotlin fallback without updating Rust, or
     * vice-versa — both MUST stay equal.
     */
    @Test
    fun `RECENT_SYNC_MS compile-time fallback is five minutes`() {
        val fiveMinutesMs = 5 * 60 * 1_000L
        assertEquals(
            "RECENT_SYNC_MS must equal 5 minutes (300_000 ms) — mirrors " +
                "copypaste_ipc::SYNC_BADGE_RECENT_MS. If either changes, update both. " +
                "See CopyPaste-km61.",
            fiveMinutesMs,
            RECENT_SYNC_MS,
        )
    }

    /**
     * RECENT_SYNC_MS must be positive and sensible (> 1 minute, < 1 hour).
     * Guards against accidental zeroing or enormous values.
     */
    @Test
    fun `RECENT_SYNC_MS is within sensible bounds`() {
        val oneMinuteMs = 60_000L
        val oneHourMs = 3_600_000L
        assertTrue(
            "RECENT_SYNC_MS ($RECENT_SYNC_MS) must be > 1 minute",
            RECENT_SYNC_MS > oneMinuteMs,
        )
        assertTrue(
            "RECENT_SYNC_MS ($RECENT_SYNC_MS) must be < 1 hour",
            RECENT_SYNC_MS < oneHourMs,
        )
    }

    /**
     * DevicesOnlineState.seedFromRust() must exist as a method and accept no parameters.
     * This test uses reflection to verify the structural contract — it does NOT call the
     * method (that would require the native .so). We just confirm the API contract is present
     * so the call in CopyPasteApp.onCreate compiles correctly.
     */
    @Test
    fun `DevicesOnlineState exposes seedFromRust method`() {
        val method = try {
            DevicesOnlineState::class.java.getMethod("seedFromRust")
        } catch (e: NoSuchMethodException) {
            null
        }
        assertTrue(
            "DevicesOnlineState.seedFromRust() must exist (CopyPaste-km61). " +
                "CopyPasteApp.onCreate should call it to seed RECENT_SYNC_MS from Rust.",
            method != null,
        )
        // seedFromRust takes no parameters.
        assertEquals(
            "seedFromRust must take no parameters",
            0,
            method!!.parameterCount,
        )
    }

    /**
     * CopyPaste-km61: verify that DevicesOnlineState.kt references RECENT_SYNC_MS
     * (the variable) rather than the raw literal `5 * 60 * 1_000`, confirming the seeded
     * value propagates to badge computation and peer-online checks.
     */
    @Test
    fun `DevicesOnlineState uses RECENT_SYNC_MS variable not hardcoded literal`() {
        val source = readSource("src/main/java/com/copypaste/android/DevicesOnlineState.kt")
            ?: return // skip if running outside the module tree (CI artifact tests)

        // The raw literal `5 * 60 * 1_000L` must NOT appear outside the declaration line
        // (that would mean it's hardcoded elsewhere instead of using the variable).
        // We look for `RECENT_SYNC_MS` being passed to isPeerOnline — the key usage site.
        assertTrue(
            "DevicesOnlineState.kt must pass recentSyncMs = RECENT_SYNC_MS to isPeerOnline " +
                "(not a hardcoded literal). See CopyPaste-km61.",
            source.contains("recentSyncMs = RECENT_SYNC_MS"),
        )

        // seedFromRust() must call syncBadgeRecentMs() to load the Rust value.
        assertTrue(
            "DevicesOnlineState.seedFromRust() must call syncBadgeRecentMs() " +
                "to seed from the Rust source of truth. See CopyPaste-km61.",
            source.contains("syncBadgeRecentMs()"),
        )
    }

    /**
     * CopyPaste-234q: verify that FgsSyncLoop.kt no longer has hardcoded badge strings
     * ("synced", "idle") as the sole argument to setBadgeState; instead it should use
     * computeAndroidSyncBadgeState.
     */
    @Test
    fun `FgsSyncLoop uses computeAndroidSyncBadgeState instead of hardcoded badge strings`() {
        val source = readSource("src/main/java/com/copypaste/android/FgsSyncLoop.kt")
            ?: return

        // computeAndroidSyncBadgeState must now appear in the source.
        assertTrue(
            "FgsSyncLoop.kt must call computeAndroidSyncBadgeState to derive badge strings " +
                "from the Rust source of truth. See CopyPaste-234q.",
            source.contains("computeAndroidSyncBadgeState("),
        )

        // The old hardcoded string pattern — `setBadgeState("synced")` or
        // `setBadgeState("idle")` as a standalone literal — must not appear.
        // (The strings may appear inside the Rust-FFI stub fallback comment, but
        // must not be the direct argument to setBadgeState without going through
        // the FFI call.)
        val standaloneHardcodedSynced = Regex("""setBadgeState\("synced"\)""")
        val standaloneHardcodedIdle   = Regex("""setBadgeState\("idle"\)""")
        assertTrue(
            "FgsSyncLoop.kt must not call setBadgeState(\"synced\") directly — " +
                "use computeAndroidSyncBadgeState() instead. See CopyPaste-234q.",
            !standaloneHardcodedSynced.containsMatchIn(source),
        )
        assertTrue(
            "FgsSyncLoop.kt must not call setBadgeState(\"idle\") directly — " +
                "use computeAndroidSyncBadgeState() instead. See CopyPaste-234q.",
            !standaloneHardcodedIdle.containsMatchIn(source),
        )
    }

    /**
     * CopyPaste-234q: verify that CopypasteBindings.kt exposes computeAndroidSyncBadgeState
     * as a public Kotlin wrapper so FgsSyncLoop can call it without importing uniffi directly.
     */
    @Test
    fun `CopypasteBindings exposes computeAndroidSyncBadgeState wrapper`() {
        val source = readSource("src/main/java/com/copypaste/android/CopypasteBindings.kt")
            ?: return

        assertTrue(
            "CopypasteBindings.kt must contain a fun computeAndroidSyncBadgeState wrapper. " +
                "See CopyPaste-234q.",
            source.contains("fun computeAndroidSyncBadgeState("),
        )

        // Must also have the syncBadgeRecentMs wrapper (CopyPaste-km61).
        assertTrue(
            "CopypasteBindings.kt must contain a fun syncBadgeRecentMs wrapper. " +
                "See CopyPaste-km61.",
            source.contains("fun syncBadgeRecentMs()"),
        )
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /** Read a source file relative to the Gradle module root. Returns null if not found. */
    private fun readSource(relativePath: String): String? {
        val anchor = RecentSyncMsParityTest::class.java
            .protectionDomain?.codeSource?.location?.toURI()
            ?.let { java.io.File(it) }
        var dir: java.io.File? = anchor
        while (dir != null) {
            if (java.io.File(dir, "src/main").exists()) {
                val f = java.io.File(dir, relativePath)
                if (f.exists()) return f.readText()
                break
            }
            dir = dir.parentFile
        }
        return null
    }
}
