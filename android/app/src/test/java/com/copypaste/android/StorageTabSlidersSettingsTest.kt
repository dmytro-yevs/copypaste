package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * CopyPaste-myh8.9 wave 4 (§I "Storage tab", sliders/knobs half):
 * `max_text_size_bytes`, `max_image_size_bytes`, `max_file_size_bytes`
 * (persistence+clamp only — consumer already covered by
 * [RepairedSettingsConsumersTest]'s maxFileSizeBytes capture/import rows),
 * `storage_quota_bytes`, `sensitive_ttl_secs`, `max_history_items`,
 * `excluded_app_bundle_ids` dedup/trim.
 *
 * `max_history_items`'s consumer ([ClipboardRepository.applyHistoryCap]) is
 * only invoked from [SettingsActivity]'s `persistAll()`, a function declared
 * inside a `@Composable` — this module has no `createComposeRule` infra
 * (Wave-4 constraint), so there is no call-order seam reachable from a plain
 * JVM unit test. Persistence-only here; the call-order guarantee ("cap is
 * applied only after a successful commit") is deferred to
 * connected/integration coverage. NOTE this explicitly per the wave brief.
 *
 * Store-level clamp/default/round-trip characterization for these SAME knobs
 * already exists in [ConfigKnobsStoreTest] (against a bare [ConfigKnobsStore]
 * + [FakeSharedPreferences]) — this file instead goes through the
 * [Settings] facade with a real Robolectric [Context], which is the layer
 * S9.4 actually calls out ("persistence · consumer" per control), and adds
 * upgrade-fixture coverage that [ConfigKnobsStoreTest] does not.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class StorageTabSlidersSettingsTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    // ── max_text_size_bytes / max_image_size_bytes ───────────────────────────

    @Test
    fun `max_text_size_bytes defaults to the native defaultConfig value and round-trips`() {
        val ctx = context()
        assertEquals(defaultConfig().maxTextSizeBytes.toLong(), Settings(ctx).maxTextSizeBytes)
        Settings(ctx).maxTextSizeBytes = 2_000_000L
        assertEquals(2_000_000L, Settings(ctx).maxTextSizeBytes)
    }

    @Test
    fun `max_image_size_bytes defaults to the native defaultConfig value and round-trips`() {
        val ctx = context()
        assertEquals(defaultConfig().maxImageSizeBytes.toLong(), Settings(ctx).maxImageSizeBytes)
        Settings(ctx).maxImageSizeBytes = 8_000_000L
        assertEquals(8_000_000L, Settings(ctx).maxImageSizeBytes)
    }

    @Test
    fun `max_text_size_bytes and max_image_size_bytes upgrade fixture — pre-redesign raw keys survive`() {
        val ctx = context()
        rawPrefs().edit()
            .putLong("max_text_size_bytes", 111L)
            .putLong("max_image_size_bytes", 222L)
            .apply()
        val settings = Settings(ctx)
        assertEquals(111L, settings.maxTextSizeBytes)
        assertEquals(222L, settings.maxImageSizeBytes)
    }

    // ── max_file_size_bytes (persistence+clamp only) ─────────────────────────

    @Test
    fun `max_file_size_bytes defaults to the native defaultConfig value and round-trips`() {
        val ctx = context()
        assertEquals(defaultConfig().maxFileSizeBytes.toLong(), Settings(ctx).maxFileSizeBytes)
        Settings(ctx).maxFileSizeBytes = 50_000_000L
        assertEquals(50_000_000L, Settings(ctx).maxFileSizeBytes)
    }

    @Test
    fun `max_file_size_bytes clamps a negative write to the zero floor`() {
        val ctx = context()
        Settings(ctx).maxFileSizeBytes = -5L
        assertTrue(Settings(ctx).maxFileSizeBytes >= 0L)
    }

    @Test
    fun `max_file_size_bytes upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putLong("max_file_size_bytes", 333L).apply()
        assertEquals(333L, Settings(ctx).maxFileSizeBytes)
    }

    // ── storage_quota_bytes ───────────────────────────────────────────────────

    @Test
    fun `storage_quota_bytes defaults to the native defaultConfig value and round-trips`() {
        val ctx = context()
        assertEquals(defaultConfig().storageQuotaBytes.toLong(), Settings(ctx).storageQuotaBytes)
        Settings(ctx).storageQuotaBytes = 500_000_000L
        assertEquals(500_000_000L, Settings(ctx).storageQuotaBytes)
    }

    @Test
    fun `storage_quota_bytes upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putLong("storage_quota_bytes", 444L).apply()
        assertEquals(444L, Settings(ctx).storageQuotaBytes)
    }

    // ── sensitive_ttl_secs ────────────────────────────────────────────────────

    @Test
    fun `sensitive_ttl_secs defaults to the native defaultConfig value and round-trips`() {
        val ctx = context()
        assertEquals(defaultConfig().sensitiveTtlSecs.toLong(), Settings(ctx).sensitiveTtlSecs)
        Settings(ctx).sensitiveTtlSecs = 3600L
        assertEquals(3600L, Settings(ctx).sensitiveTtlSecs)
    }

    @Test
    fun `sensitive_ttl_secs 0 auto-wipe-disabled sentinel survives through Settings`() {
        val ctx = context()
        Settings(ctx).sensitiveTtlSecs = 0L
        assertEquals(0L, Settings(ctx).sensitiveTtlSecs)
    }

    @Test
    fun `sensitive_ttl_secs upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putLong("sensitive_ttl_secs", 555L).apply()
        assertEquals(555L, Settings(ctx).sensitiveTtlSecs)
    }

    // ── max_history_items — persistence only (see class kdoc) ───────────────

    @Test
    fun `max_history_items defaults to 1000 and round-trips`() {
        val ctx = context()
        assertEquals(1000, Settings(ctx).maxHistoryItems)
        Settings(ctx).maxHistoryItems = 250
        assertEquals(250, Settings(ctx).maxHistoryItems)
    }

    @Test
    fun `max_history_items upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putInt("max_history_items", 42).apply()
        assertEquals(42, Settings(ctx).maxHistoryItems)
    }

    @Test
    fun `max_history_items survives the atomic saveScreenSettings batch`() {
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
            notifyOnCopy = true,
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
            maxHistoryItems = 77,
        )
        assertTrue(committed)
        assertEquals(77, Settings(ctx).maxHistoryItems)
    }

    // ── excluded_app_bundle_ids dedup/trim (clampSizeKnobs behavior) ─────────

    @Test
    fun `excluded_app_bundle_ids trims blanks and de-dups on write through Settings`() {
        val ctx = context()
        Settings(ctx).excludedAppBundleIds =
            listOf(" com.example.app ", "com.example.app", "", "  ", "com.other.app")

        assertEquals(
            listOf("com.example.app", "com.other.app"),
            Settings(ctx).excludedAppBundleIds,
        )
    }

    @Test
    fun `excluded_app_bundle_ids defaults to empty and upgrade fixture survives`() {
        val ctx = context()
        assertEquals(defaultConfig().excludedAppBundleIds, Settings(ctx).excludedAppBundleIds)

        rawPrefs().edit()
            .putString(ConfigKnobsStore.KEY_EXCLUDED_APP_BUNDLE_IDS, "com.legacy.one${ConfigKnobsStore.EXCLUDED_APP_DELIM}com.legacy.two")
            .apply()
        assertEquals(listOf("com.legacy.one", "com.legacy.two"), Settings(ctx).excludedAppBundleIds)
    }
}
