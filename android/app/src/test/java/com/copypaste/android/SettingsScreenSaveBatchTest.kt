package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.copypaste.android.ui.theme.AccentColor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * CopyPaste-myh8.9 wave 0: folds the 9 stray settings fields (collectPublicIp,
 * pasteAsPlainText, excludedAppBundleIds, showSensitiveWarnings,
 * autoApplySyncedClip, maxFileSizeBytes, sensitiveTtlSecs, previewLines,
 * maxHistoryItems) into the atomic [Settings.saveScreenSettings] `commit()`
 * batch, so a force-stop right after Save can no longer drop them (the same
 * root-cause class as the theme_mode/accent fold — see
 * SettingsThemeMigrationTest).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SettingsScreenSaveBatchTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    private fun callBatch(
        settings: Settings,
        collectPublicIp: Boolean = true,
        pasteAsPlainText: Boolean = true,
        excludedAppBundleIds: List<String> = listOf("com.example.one", "com.example.two"),
        showSensitiveWarnings: Boolean = false,
        autoApplySyncedClip: Boolean = false,
        maxFileSizeBytes: Long = 12_345_678L,
        sensitiveTtlSecs: Long = 42L,
        previewLines: Int = 4,
        maxHistoryItems: Int = 250,
    ): Boolean = settings.saveScreenSettings(
        captureEnabled = true,
        privateMode = false,
        syncEnabled = true,
        notifyOnSensitiveSkip = true,
        maskSensitiveContent = true,
        translucency = false,
        themeMode = ThemeMode.LIGHT,
        accent = AccentColor.AMBER,
        imageMaxHeight = 40,
        previewDelayMs = 1500L,
        maxTextSizeBytes = 1_000_000L,
        maxImageSizeBytes = 5_000_000L,
        storageQuotaBytes = 1_000_000_000L,
        syncOnWifiOnly = false,
        syncBackend = SyncBackend.SUPABASE,
        p2pSyncEnabled = true,
        lanVisibility = true,
        supabaseUrl = "",
        supabaseAnonKey = "",
        supabaseEmail = "",
        relayUrl = "",
        notifyOnCopy = true,
        soundOnCopy = true,
        logcatCaptureEnabled = false,
        collectPublicIp = collectPublicIp,
        pasteAsPlainText = pasteAsPlainText,
        excludedAppBundleIds = excludedAppBundleIds,
        showSensitiveWarnings = showSensitiveWarnings,
        autoApplySyncedClip = autoApplySyncedClip,
        maxFileSizeBytes = maxFileSizeBytes,
        sensitiveTtlSecs = sensitiveTtlSecs,
        previewLines = previewLines,
        maxHistoryItems = maxHistoryItems,
    )

    @Test
    fun `saveScreenSettings persists all 9 folded fields and a fresh Settings instance reads them back`() {
        val ctx = context()
        val settings = Settings(ctx)

        val committed = callBatch(settings)

        assertTrue(committed)
        val reloaded = Settings(ctx)
        assertTrue(reloaded.collectPublicIp)
        assertTrue(reloaded.pasteAsPlainText)
        assertEquals(listOf("com.example.one", "com.example.two"), reloaded.excludedAppBundleIds)
        assertEquals(false, reloaded.showSensitiveWarnings)
        assertEquals(false, reloaded.autoApplySyncedClip)
        assertEquals(12_345_678L, reloaded.maxFileSizeBytes)
        assertEquals(42L, reloaded.sensitiveTtlSecs)
        assertEquals(4, reloaded.previewLines)
        assertEquals(250, reloaded.maxHistoryItems)
    }

    @Test
    fun `legacy pre-seeded values are not silently dropped by the new batch write`() {
        val ctx = context()
        // Pre-S9 upgrade fixture: a prior install already wrote these keys via the
        // old per-setter apply() paths before this batch existed.
        rawPrefs().edit()
            .putBoolean("collect_public_ip", false)
            .putBoolean("paste_as_plain_text", false)
            .putString("excluded_app_bundle_ids", "com.legacy.app")
            .putBoolean("show_sensitive_warnings_reveal_guard", true)
            .putBoolean("auto_apply_synced_clip", true)
            .putLong("max_file_size_bytes", 999L)
            .putLong("sensitive_ttl_secs", 7L)
            .putInt("preview_lines", 2)
            .putInt("max_history_items", 500)
            .commit()

        val settings = Settings(ctx)
        val committed = callBatch(
            settings,
            collectPublicIp = true,
            pasteAsPlainText = true,
            excludedAppBundleIds = listOf("com.upgraded.app"),
            showSensitiveWarnings = false,
            autoApplySyncedClip = false,
            maxFileSizeBytes = 2_000_000L,
            sensitiveTtlSecs = 99L,
            previewLines = 6,
            maxHistoryItems = 42,
        )

        assertTrue(committed)
        assertTrue(settings.collectPublicIp)
        assertTrue(settings.pasteAsPlainText)
        assertEquals(listOf("com.upgraded.app"), settings.excludedAppBundleIds)
        assertEquals(false, settings.showSensitiveWarnings)
        assertEquals(false, settings.autoApplySyncedClip)
        assertEquals(2_000_000L, settings.maxFileSizeBytes)
        assertEquals(99L, settings.sensitiveTtlSecs)
        assertEquals(6, settings.previewLines)
        assertEquals(42, settings.maxHistoryItems)
    }

    @Test
    fun `all 9 keys are readable synchronously right after commit returns — no flush wait needed`() {
        val ctx = context()
        val settings = Settings(ctx)

        callBatch(settings)

        val raw = rawPrefs()
        assertTrue(raw.contains("collect_public_ip"))
        assertTrue(raw.contains("paste_as_plain_text"))
        assertTrue(raw.contains("excluded_app_bundle_ids"))
        assertTrue(raw.contains("show_sensitive_warnings_reveal_guard"))
        assertTrue(raw.contains("auto_apply_synced_clip"))
        assertTrue(raw.contains("max_file_size_bytes"))
        assertTrue(raw.contains("sensitive_ttl_secs"))
        assertTrue(raw.contains("preview_lines"))
        assertTrue(raw.contains("max_history_items"))
    }
}
