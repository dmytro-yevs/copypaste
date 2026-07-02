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
 * android-appearance D4/D6/S3 task 3.4: theme/accent defaults, the versioned
 * `migrateThemeForTwoAxis` latch (`theme_migrated_2axis` -> the D6-fixed
 * `theme_migrated_2axis_v2`), and cross-instance persistence of the two-axis
 * keys. Robolectric (not the plain-JVM stub) because [Settings] always goes
 * through a real `Context.getSharedPreferences`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SettingsThemeMigrationTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    @Test
    fun `fresh install defaults to dark theme and indigo accent`() {
        val settings = Settings(context())
        assertEquals(ThemeMode.DARK, settings.themeMode)
        assertEquals(AccentColor.INDIGO, settings.accent)
    }

    @Test
    fun `migration removes stale Liquid-Glass keys but retains theme_mode and accent`() {
        val settings = Settings(context())
        // A pre-S3 upgrade: the user already saved a real theme/accent, AND the
        // stale Liquid-Glass-era keys are still present from an older install.
        settings.themeMode = ThemeMode.LIGHT
        settings.accent = AccentColor.ROSE
        rawPrefs().edit()
            .putString("palette", "stale")
            .putString("skin", "stale")
            .putString("density", "stale")
            .putBoolean("motion_reduced", true)
            .putString("contrast", "stale")
            .apply()

        settings.migrateThemeForTwoAxis()

        assertFalse(rawPrefs().contains("palette"))
        assertFalse(rawPrefs().contains("skin"))
        assertFalse(rawPrefs().contains("density"))
        assertFalse(rawPrefs().contains("motion_reduced"))
        assertFalse(rawPrefs().contains("contrast"))
        // The D6 fix under test: theme_mode/accent must survive migration.
        assertEquals(ThemeMode.LIGHT, settings.themeMode)
        assertEquals(AccentColor.ROSE, settings.accent)
    }

    @Test
    fun `migration is idempotent — a second call does not re-clear a freshly saved theme`() {
        val settings = Settings(context())
        settings.migrateThemeForTwoAxis()
        settings.themeMode = ThemeMode.LIGHT
        settings.accent = AccentColor.TEAL

        settings.migrateThemeForTwoAxis() // second call — latch already set, must no-op

        assertEquals(ThemeMode.LIGHT, settings.themeMode)
        assertEquals(AccentColor.TEAL, settings.accent)
    }

    @Test
    fun `already-migrated install is untouched — the v2 latch skips the migration body entirely`() {
        val settings = Settings(context())
        rawPrefs().edit()
            .putBoolean("theme_migrated_2axis_v2", true)
            .putString("palette", "still-here")
            .apply()

        settings.migrateThemeForTwoAxis()

        // Latch already set — the stale-key removal never runs.
        assertTrue(rawPrefs().contains("palette"))
    }

    // NOTE: a "migration runs before first getter read" test was deleted here — it was a can't-fail duplicate of "migration is idempotent" (D6 made theme_mode/accent canonical, so getter order no longer matters).

    @Test
    fun `saveScreenSettings persists theme_mode and accent and a fresh Settings instance reads them back`() {
        val ctx = context()
        val settings = Settings(ctx)
        val committed = settings.saveScreenSettings(
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
        )

        assertTrue(committed)
        // A brand-new Settings instance against the SAME SharedPreferences file —
        // what a process restart after force-stop reads back (android-appearance
        // "Force-stop-safe save" / "committed-survives-process-death").
        val reloaded = Settings(ctx)
        assertEquals(ThemeMode.LIGHT, reloaded.themeMode)
        assertEquals(AccentColor.AMBER, reloaded.accent)
        assertFalse(reloaded.translucency)
    }

    @Test
    fun `corrupt persisted theme_mode falls back to the default instead of crashing`() {
        val ctx = context()
        rawPrefs().edit().putString("theme_mode", "not_a_real_mode").apply()

        assertEquals(ThemeMode.DEFAULT, Settings(ctx).themeMode)
    }
}
