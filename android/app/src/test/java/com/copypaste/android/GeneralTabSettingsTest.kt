package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows
import org.robolectric.annotation.Config

/**
 * CopyPaste-myh8.9 wave 4 (§I "General tab"): 3-layer preservation coverage for
 * `private_mode`, `sync_enabled`, `collect_public_ip` (ConfigKnobs),
 * `paste_as_plain_text` (ConfigKnobs), `logcat_capture_enabled`.
 *
 * Persistence layer: default, round-trip via [Settings], and a pre-redesign
 * upgrade fixture (raw key written directly, then read through a fresh
 * [Settings] instance — matches [SettingsThemeMigrationTest]'s pattern).
 *
 * Consumer layer: wired where reachable without native/.so or Compose infra
 * ([collectPublicIp] gates [StunUtils.queryPublicIp] before any I/O;
 * [logcatCaptureEnabled] gates [LogcatCaptureService.syncState]/[status]).
 * `private_mode`/`sync_enabled` consumers live inside
 * [ClipboardCapturePipeline.captureClip] (requires a live [SyncManager] +
 * native encryption library, same reason [RepairedSettingsConsumersTest]
 * avoids calling it directly) and inside long-running sync-loop bodies
 * (FgsSyncLoop/RelaySubscriptionClient/SupabasePollWorker) with no extracted
 * pure gate function — deferred to connected/integration coverage, NOTE only.
 * `paste_as_plain_text`'s consumer read (`HistoryList.kt`) is Compose-only —
 * deferred to goldens/connected per the wave brief.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class GeneralTabSettingsTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    // ── private_mode ─────────────────────────────────────────────────────────

    @Test
    fun `private_mode defaults to false`() {
        assertFalse(Settings(context()).privateMode)
    }

    @Test
    fun `private_mode round-trips true through a fresh Settings instance`() {
        val ctx = context()
        Settings(ctx).privateMode = true
        assertTrue(Settings(ctx).privateMode)
    }

    @Test
    fun `private_mode upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("private_mode", true).apply()
        assertTrue(Settings(ctx).privateMode)
    }

    // ── sync_enabled ─────────────────────────────────────────────────────────

    @Test
    fun `sync_enabled defaults to true`() {
        assertTrue(Settings(context()).syncEnabled)
    }

    @Test
    fun `sync_enabled round-trips false through a fresh Settings instance`() {
        val ctx = context()
        Settings(ctx).syncEnabled = false
        assertFalse(Settings(ctx).syncEnabled)
    }

    @Test
    fun `sync_enabled upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("sync_enabled", false).apply()
        assertFalse(Settings(ctx).syncEnabled)
    }

    // ── collect_public_ip (ConfigKnobs) ─────────────────────────────────────

    @Test
    fun `collect_public_ip defaults to the native defaultConfig value`() {
        assertEquals(defaultConfig().collectPublicIp, Settings(context()).collectPublicIp)
    }

    @Test
    fun `collect_public_ip round-trips through a fresh Settings instance`() {
        val ctx = context()
        Settings(ctx).collectPublicIp = true
        assertTrue(Settings(ctx).collectPublicIp)
    }

    @Test
    fun `collect_public_ip upgrade fixture — pre-redesign raw ConfigKnobs key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("collect_public_ip", true).apply()
        assertTrue(Settings(ctx).collectPublicIp)
    }

    @Test
    fun `collect_public_ip consumer — StunUtils queryPublicIp short-circuits to null without any I-O when disabled`() = runBlocking {
        val ctx = context()
        Settings(ctx).collectPublicIp = false
        val settings = Settings(ctx)

        // The gate itself: StunUtils.queryPublicIp(collectEnabled) — the ACTUAL
        // consumer read is `StunUtils.queryPublicIp(settings.collectPublicIp)`
        // (DevicesController / P2pListenerController / PairBootstrapSync). We
        // exercise the gate function directly with the live setting value; the
        // disabled path returns null before any socket/STUN I/O, so this is
        // safe and fast under JVM unit tests (the enabled path performs a real
        // 5s-budget network call and is deferred to connected tests).
        val result = StunUtils.queryPublicIp(settings.collectPublicIp)

        assertNull("collectPublicIp=false must short-circuit to null with no STUN I/O", result)
    }

    // ── paste_as_plain_text (ConfigKnobs) ───────────────────────────────────

    @Test
    fun `paste_as_plain_text defaults to the native defaultConfig value`() {
        assertEquals(defaultConfig().pasteAsPlainText, Settings(context()).pasteAsPlainText)
    }

    @Test
    fun `paste_as_plain_text round-trips through a fresh Settings instance`() {
        val ctx = context()
        Settings(ctx).pasteAsPlainText = true
        assertTrue(Settings(ctx).pasteAsPlainText)
    }

    // NOTE: paste_as_plain_text's consumer (HistoryList.kt `forcePlainText = settings.pasteAsPlainText`)
    // is read inside a @Composable — this module has no createComposeRule infra (Wave-4 constraint).
    // Deferred to goldens/connected coverage.

    // ── logcat_capture_enabled ───────────────────────────────────────────────

    @Test
    fun `logcat_capture_enabled defaults to false`() {
        assertFalse(Settings(context()).logcatCaptureEnabled)
    }

    @Test
    fun `logcat_capture_enabled round-trips true through a fresh Settings instance`() {
        val ctx = context()
        Settings(ctx).logcatCaptureEnabled = true
        assertTrue(Settings(ctx).logcatCaptureEnabled)
    }

    @Test
    fun `logcat_capture_enabled upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("logcat_capture_enabled", true).apply()
        assertTrue(Settings(ctx).logcatCaptureEnabled)
    }

    @Test
    fun `logcat_capture_enabled consumer — status() reports DISABLED when the toggle is off (permission granted)`() {
        val ctx = context()
        // status() checks READ_LOGS FIRST, then logcatCaptureEnabled — grant the
        // permission via the Robolectric shadow so the toggle itself is under test.
        Shadows.shadowOf(ctx as android.app.Application)
            .grantPermissions(android.Manifest.permission.READ_LOGS)
        val settings = Settings(ctx)
        settings.logcatCaptureEnabled = false

        assertEquals(LogcatCaptureStatus.DISABLED, LogcatCaptureService.status(ctx, settings))
    }

    @Test
    fun `logcat_capture_enabled consumer — status() reports NOT_GRANTED regardless of the toggle when permission is missing`() {
        val ctx = context()
        val settings = Settings(ctx)
        settings.logcatCaptureEnabled = true

        assertEquals(LogcatCaptureStatus.NOT_GRANTED, LogcatCaptureService.status(ctx, settings))
    }
}
