package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Tests for two QR countdown drain-bar fixes:
 *
 * CopyPaste-wfba — PairActivity was missing the drain-bar countdown that
 *   DevicesActivity's OwnQrSection already shows. These tests verify the
 *   drain-bar is present in PairActivity and wired to the correct TTL constant.
 *
 * CopyPaste-h59h — On visibility-restore after the QR has expired (>105 s hidden),
 *   the drain bar briefly shows 0 s (one composition frame before
 *   LaunchedEffect(qr) resets remainingSeconds). The fix guards the countdown
 *   block with `!loading` so no stale 0 s frame is visible to the user.
 *
 * All tests run on the JVM without an emulator (source-inspection + pure-logic).
 */
class PairDrainBarTest {

    private fun pairSource(): String {
        val f = File("src/main/java/com/copypaste/android/PairActivity.kt")
        assertTrue("PairActivity.kt not found at ${f.absolutePath}", f.exists())
        return f.readText()
    }

    private fun devicesSource(): String {
        val f = File("src/main/java/com/copypaste/android/DevicesActivity.kt")
        assertTrue("DevicesActivity.kt not found at ${f.absolutePath}", f.exists())
        return f.readText()
    }

    // ── CopyPaste-wfba: drain-bar present in PairActivity ────────────────────

    @Test
    fun `PairActivity countdown block contains a drain-bar Box`() {
        val src = pairSource()
        // The drain-bar is a thin 2.dp height Box with a fillMaxWidth track that
        // contains a fillMaxWidth(fraction) fill Box — same structural pattern as
        // DevicesActivity OwnQrSection §10.
        val hasDrainBar = src.contains(".height(2.dp)") &&
            src.contains("qrCountdownProgress(")
        assertTrue(
            "PairActivity must render a 2dp drain-bar using qrCountdownProgress(…) " +
                "(CopyPaste-wfba: drain-bar was missing)",
            hasDrainBar,
        )
    }

    @Test
    fun `PairActivity drain-bar references PAIR_TOKEN_TTL_SECONDS as totalSeconds`() {
        val src = pairSource()
        // The fillMaxWidth fraction call must pass PAIR_TOKEN_TTL_SECONDS as the
        // second argument to qrCountdownProgress so the bar drains over the correct
        // 120-second window — not a hardcoded literal.
        val correct = src.contains("qrCountdownProgress(remainingSeconds, PAIR_TOKEN_TTL_SECONDS)")
        assertTrue(
            "Drain-bar in PairActivity must use qrCountdownProgress(remainingSeconds, " +
                "PAIR_TOKEN_TTL_SECONDS) — hardcoding a literal would diverge from the Rust TTL",
            correct,
        )
    }

    @Test
    fun `PairActivity drain-bar switches to warning color when urgent`() {
        val src = pairSource()
        // The drain-bar fill color must flip to c.warning when urgent (≤20 s).
        // The urgency predicate is either inline `remainingSeconds <= …` or via
        // `isQrWarning(remainingSeconds)` — either form is acceptable as long as
        // the warning branch references c.warning.
        val hasWarningColor = src.contains("c.warning") && src.contains(".height(2.dp)")
        assertTrue(
            "PairActivity drain-bar fill must use c.warning colour in the urgent zone",
            hasWarningColor,
        )
    }

    // ── CopyPaste-h59h: 0s flash fix — loading guard on countdown block ───────

    @Test
    fun `DevicesActivity OwnQrSection countdown guard includes loading check`() {
        val src = devicesSource()
        // The guard condition for the §10 countdown text + drain-bar block must
        // include `!loading` so that during QR regeneration (loading = true) the
        // stale remainingSeconds == 0 value is never rendered to the user.
        // We verify this by checking that `!loading` appears inside the OwnQrSection
        // after the progress-bar comment anchor.
        val progressAnchor = "§10 Countdown"
        val anchorIdx = src.indexOf(progressAnchor)
        assertTrue("§10 Countdown anchor not found in OwnQrSection", anchorIdx >= 0)

        val afterAnchor = src.substring(anchorIdx)
        // The guard should contain `!loading` close to the qr != null check.
        val hasLoadingGuard = afterAnchor.contains("!loading") &&
            afterAnchor.indexOf("!loading") < afterAnchor.indexOf("errorMsg?.let")
        assertTrue(
            "OwnQrSection §10 countdown block must guard on !loading to prevent the " +
                "1-frame 0s flash when a new QR is being loaded (CopyPaste-h59h)",
            hasLoadingGuard,
        )
    }

    @Test
    fun `PairActivity countdown block guards on positive remainingSeconds`() {
        val src = pairSource()
        // After the wfba drain-bar is added, the same loading guard must be present
        // in PairActivity's countdown block so it is consistent with DevicesActivity.
        // Specifically: the `if (qr != null)` countdown block must also check !loading.
        val expiresIdx = src.indexOf("pair_token_expires_in_seconds")
        assertTrue("pair_token_expires_in_seconds string not found in PairActivity", expiresIdx >= 0)

        // Walk backwards from the expires string to find the enclosing `if` guard.
        // The guard should appear within 600 characters before the expires reference.
        val guardWindow = src.substring(maxOf(0, expiresIdx - 600), expiresIdx)
        val hasLoadingGuard = guardWindow.contains("!loading")
        assertTrue(
            "PairActivity countdown block must check !loading (prevents 0s flash, " +
                "consistent with DevicesActivity CopyPaste-h59h fix)",
            hasLoadingGuard,
        )
    }

    // ── Pure-logic tests for shared drain-bar helpers ─────────────────────────

    @Test
    fun `qrCountdownProgress returns 1f at full TTL`() {
        assertEquals(1f, qrCountdownProgress(120, 120), 0.001f)
    }

    @Test
    fun `qrCountdownProgress returns 0f at zero remaining`() {
        assertEquals(0f, qrCountdownProgress(0, 120), 0.001f)
    }

    @Test
    fun `qrCountdownProgress returns 0_5f at half TTL`() {
        assertEquals(0.5f, qrCountdownProgress(60, 120), 0.001f)
    }

    @Test
    fun `qrCountdownProgress clamps negative remaining to 0f`() {
        assertEquals(0f, qrCountdownProgress(-5, 120), 0.001f)
    }

    @Test
    fun `isQrWarning returns true at urgency threshold`() {
        // DEVICES_QR_URGENT_THRESHOLD_SECONDS = 20
        assertTrue(isQrWarning(20))
        assertTrue(isQrWarning(1))
        assertTrue(isQrWarning(0))
    }

    @Test
    fun `isQrWarning returns false above urgency threshold`() {
        assertTrue(!isQrWarning(21))
        assertTrue(!isQrWarning(120))
    }
}
