package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * CopyPaste-myh8.9 wave 4 (§I "Notifications tab"): `notify_on_copy`,
 * `sound_on_copy` persistence + a 4-combination independence truth table.
 *
 * rg-confirmed: no `CopyNotificationSoundTest` (or any other test file)
 * exists for these two keys — this is a NEW file, not an extension.
 *
 * Consumer layer: both flags gate `ServiceNotifications.postCopyNotification`/
 * `playCopySound` from three call sites in [ClipboardCapturePipeline]
 * (`if (settings.notifyOnCopy) ...` / `if (settings.soundOnCopy) ...`).
 * `postCopyNotification` calls `context.getString(R.string...)`, which throws
 * under this module's JVM unit tests (no merged Android resources — see
 * [RepairedSettingsConsumersTest]'s kdoc for the same constraint applied to
 * `notifySensitiveSkipIfEnabled`). Unlike that wave-1 consumer, the
 * notify/sound-on-copy call sites are inline `if` checks inside the full
 * `captureClip`/related pipeline functions — not extracted standalone gate
 * functions — so there is no resource-free seam to call directly. The truth
 * table below therefore verifies the actual gating VALUES (the two flags read
 * and combine correctly and independently) at the [Settings] layer; the
 * literal notification-post / sound-play side effects are deferred to
 * connected/instrumented tests.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class NotificationsTabSettingsTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    @Test
    fun `notify_on_copy and sound_on_copy both default to true`() {
        val settings = Settings(context())
        assertTrue(settings.notifyOnCopy)
        assertTrue(settings.soundOnCopy)
    }

    @Test
    fun `notify_on_copy round-trips independently of sound_on_copy`() {
        val ctx = context()
        val settings = Settings(ctx)
        settings.notifyOnCopy = false
        assertFalse(Settings(ctx).notifyOnCopy)
        assertTrue("sound_on_copy must be unaffected", Settings(ctx).soundOnCopy)
    }

    @Test
    fun `sound_on_copy round-trips independently of notify_on_copy`() {
        val ctx = context()
        val settings = Settings(ctx)
        settings.soundOnCopy = false
        assertFalse(Settings(ctx).soundOnCopy)
        assertTrue("notify_on_copy must be unaffected", Settings(ctx).notifyOnCopy)
    }

    @Test
    fun `notify_on_copy and sound_on_copy upgrade fixture — pre-redesign raw keys survive`() {
        val ctx = context()
        rawPrefs().edit()
            .putBoolean("notify_on_copy", false)
            .putBoolean("sound_on_copy", false)
            .apply()
        val settings = Settings(ctx)
        assertFalse(settings.notifyOnCopy)
        assertFalse(settings.soundOnCopy)
    }

    /**
     * The full 2x2 independence truth table: every one of the 4 combinations of
     * (notifyOnCopy, soundOnCopy) is a distinct, independently-readable state —
     * neither flag ever drives or clobbers the other, in either direction.
     */
    @Test
    fun `4-combination independence truth table — every notify-sound pair is reachable and stable`() {
        val ctx = context()
        val combinations = listOf(
            true to true,
            true to false,
            false to true,
            false to false,
        )

        combinations.forEach { (notify, sound) ->
            val settings = Settings(ctx)
            settings.notifyOnCopy = notify
            settings.soundOnCopy = sound

            val reloaded = Settings(ctx)
            assertEquals("notifyOnCopy=$notify soundOnCopy=$sound", notify, reloaded.notifyOnCopy)
            assertEquals("notifyOnCopy=$notify soundOnCopy=$sound", sound, reloaded.soundOnCopy)
        }
    }

    @Test
    fun `notify_on_copy and sound_on_copy survive the atomic saveScreenSettings batch independently`() {
        val ctx = context()
        val settings = Settings(ctx)
        val committed = settings.saveScreenSettings(
            captureEnabled = true,
            privateMode = false,
            syncEnabled = true,
            notifyOnSensitiveSkip = true,
            maskSensitiveContent = true,
            translucency = true,
            themeMode = ThemeMode.DARK,
            accent = com.copypaste.android.ui.theme.AccentColor.INDIGO,
            imageMaxHeight = 40,
            previewDelayMs = 1500L,
            maxTextSizeBytes = settings.maxTextSizeBytes,
            maxImageSizeBytes = settings.maxImageSizeBytes,
            storageQuotaBytes = settings.storageQuotaBytes,
            syncOnWifiOnly = false,
            syncBackend = SyncBackend.SUPABASE,
            p2pSyncEnabled = true,
            lanVisibility = true,
            supabaseUrl = "",
            supabaseAnonKey = "",
            supabaseEmail = "",
            relayUrl = "",
            notifyOnCopy = false,
            soundOnCopy = true,
            logcatCaptureEnabled = false,
            collectPublicIp = settings.collectPublicIp,
            pasteAsPlainText = settings.pasteAsPlainText,
            excludedAppBundleIds = settings.excludedAppBundleIds,
            showSensitiveWarnings = true,
            autoApplySyncedClip = true,
            maxFileSizeBytes = settings.maxFileSizeBytes,
            sensitiveTtlSecs = settings.sensitiveTtlSecs,
            previewLines = 1,
            maxHistoryItems = 1000,
        )
        assertTrue(committed)
        val reloaded = Settings(ctx)
        assertFalse(reloaded.notifyOnCopy)
        assertTrue(reloaded.soundOnCopy)
    }
}
