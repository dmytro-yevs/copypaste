package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.copypaste.android.ui.theme.AccentColor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * CopyPaste-myh8.9 wave 4 (§I "Display tab"): 3-layer preservation coverage.
 *
 * `notify_on_sensitive_skip`'s legacy-key migration mirrors
 * [SettingsThemeMigrationTest]'s style; its consumer (suppression-branch
 * toast) is already covered end-to-end by
 * [RepairedSettingsConsumersTest.notifyOnSensitiveSkip] tests — not
 * duplicated here.
 *
 * `allow_screenshots` is Immediate (readable without Save) — persistence
 * layer only; its FLAG_SECURE consumer application
 * ([com.copypaste.android.ui.theme.CopyPasteTheme] / SettingsComposables'
 * `applyScreenshotPolicy`) runs inside a `LaunchedEffect`/Compose view chrome
 * with no `createComposeRule` infra available — deferred to goldens/connected.
 *
 * `mask_sensitive_content`/`translucency`/`image_max_height`/
 * `preview_delay_ms`/`preview_lines` consumers are all Compose recomposition
 * reads (HistoryRow / chrome surfaces / auto-collapse timer) — persistence +
 * clamp layer only here, UI layer deferred per the wave brief.
 *
 * `theme_mode`/`accent` persistence + migration idempotency are already
 * covered by [SettingsThemeMigrationTest] — this file adds only the
 * Display-tab-specific clamp/round-trip rows not covered there.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class DisplayTabSettingsTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    // ── notify_on_sensitive_skip (+legacy migration) ─────────────────────────

    @Test
    fun `notify_on_sensitive_skip defaults to true on a fresh install`() {
        assertTrue(Settings(context()).notifyOnSensitiveSkip)
    }

    @Test
    fun `notify_on_sensitive_skip round-trips through a fresh Settings instance`() {
        val ctx = context()
        Settings(ctx).notifyOnSensitiveSkip = false
        assertFalse(Settings(ctx).notifyOnSensitiveSkip)
    }

    @Test
    fun `notify_on_sensitive_skip migrates from the legacy show_sensitive_warnings key on first read`() {
        val ctx = context()
        // Pre-bdac.32 upgrade fixture: only the OLD key was ever written.
        rawPrefs().edit().putBoolean("show_sensitive_warnings", false).apply()

        assertFalse(
            "first read must fall back to the legacy key's value",
            Settings(ctx).notifyOnSensitiveSkip,
        )
    }

    @Test
    fun `notify_on_sensitive_skip write scrubs the legacy key so it cannot resurrect a stale value`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("show_sensitive_warnings", false).apply()
        val settings = Settings(ctx)
        assertFalse(settings.notifyOnSensitiveSkip) // migrate-read

        settings.notifyOnSensitiveSkip = true // explicit write

        assertFalse(rawPrefs().contains("show_sensitive_warnings"))
        assertTrue(Settings(ctx).notifyOnSensitiveSkip)
    }

    // ── show_sensitive_warnings_reveal_guard ─────────────────────────────────

    @Test
    fun `reveal guard defaults to true`() {
        assertTrue(Settings(context()).showSensitiveWarnings)
    }

    @Test
    fun `reveal guard round-trips and upgrade fixture survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("show_sensitive_warnings_reveal_guard", false).apply()
        assertFalse(Settings(ctx).showSensitiveWarnings)

        Settings(ctx).showSensitiveWarnings = true
        assertTrue(Settings(ctx).showSensitiveWarnings)
    }

    // ── mask_sensitive_content ───────────────────────────────────────────────

    @Test
    fun `mask_sensitive_content defaults to true and round-trips`() {
        val ctx = context()
        assertTrue(Settings(ctx).maskSensitiveContent)
        Settings(ctx).maskSensitiveContent = false
        assertFalse(Settings(ctx).maskSensitiveContent)
    }

    @Test
    fun `mask_sensitive_content upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("mask_sensitive_content", false).apply()
        assertFalse(Settings(ctx).maskSensitiveContent)
    }

    // ── allow_screenshots (Immediate) ────────────────────────────────────────

    @Test
    fun `allow_screenshots defaults to false (SECURE by default)`() {
        assertFalse(Settings(context()).allowScreenshots)
    }

    @Test
    fun `allow_screenshots is readable immediately after a bare setter write — no Save batch required`() {
        val ctx = context()
        val settings = Settings(ctx)

        settings.allowScreenshots = true // NOT via saveScreenSettings — Immediate mode

        assertTrue("Immediate mode: a fresh instance must see the write without a Save", Settings(ctx).allowScreenshots)
    }

    @Test
    fun `allow_screenshots upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("allow_screenshots", true).apply()
        assertTrue(Settings(ctx).allowScreenshots)
    }

    // ── translucency ──────────────────────────────────────────────────────────

    @Test
    fun `translucency defaults to true and round-trips`() {
        val ctx = context()
        assertTrue(Settings(ctx).translucency)
        Settings(ctx).translucency = false
        assertFalse(Settings(ctx).translucency)
    }

    // ── image_max_height (1-200 clamp) ───────────────────────────────────────

    @Test
    fun `image_max_height defaults to 40dp`() {
        assertEquals(40, Settings(context()).imageMaxHeight)
    }

    @Test
    fun `image_max_height round-trips a valid value and clamps out-of-range writes`() {
        val ctx = context()
        val settings = Settings(ctx)

        settings.imageMaxHeight = 120
        assertEquals(120, settings.imageMaxHeight)

        settings.imageMaxHeight = 500 // above 200 ceiling
        assertEquals(200, settings.imageMaxHeight)

        settings.imageMaxHeight = -10 // below 1 floor
        assertEquals(1, settings.imageMaxHeight)
    }

    // ── preview_delay_ms (200-100000 clamp) ──────────────────────────────────

    @Test
    fun `preview_delay_ms defaults to 1500ms and clamps to its 200-100000 range`() {
        val ctx = context()
        val settings = Settings(ctx)
        assertEquals(1500L, settings.previewDelay)

        settings.previewDelay = 50L // below floor
        assertEquals(200L, settings.previewDelay)

        settings.previewDelay = 999_999L // above ceiling
        assertEquals(100_000L, settings.previewDelay)
    }

    // ── preview_lines (1-6 clamp) ─────────────────────────────────────────────

    @Test
    fun `preview_lines defaults to 1 and clamps to its 1-6 range`() {
        val ctx = context()
        val settings = Settings(ctx)
        assertEquals(1, settings.previewLines)

        settings.previewLines = 4
        assertEquals(4, settings.previewLines)

        settings.previewLines = 20 // above ceiling
        assertEquals(6, settings.previewLines)

        settings.previewLines = 0 // below floor
        assertEquals(1, settings.previewLines)
    }

    // ── theme_mode / accent — Display-tab-specific clamp/save-batch round-trip ──

    @Test
    fun `theme_mode and accent survive the atomic saveScreenSettings batch alongside the other Display-tab knobs`() {
        val ctx = context()
        val settings = Settings(ctx)

        val committed = settings.saveScreenSettings(
            captureEnabled = true,
            privateMode = false,
            syncEnabled = true,
            notifyOnSensitiveSkip = true,
            maskSensitiveContent = false,
            translucency = false,
            themeMode = ThemeMode.LIGHT,
            accent = AccentColor.TEAL,
            imageMaxHeight = 80,
            previewDelayMs = 3000L,
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
            previewLines = 3,
            maxHistoryItems = 1000,
        )

        assertTrue(committed)
        val reloaded = Settings(ctx)
        assertEquals(ThemeMode.LIGHT, reloaded.themeMode)
        assertEquals(AccentColor.TEAL, reloaded.accent)
        assertEquals(80, reloaded.imageMaxHeight)
        assertEquals(3000L, reloaded.previewDelay)
        assertEquals(3, reloaded.previewLines)
        assertFalse(reloaded.maskSensitiveContent)
        assertFalse(reloaded.translucency)
    }
}
