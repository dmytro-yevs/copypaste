package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.copypaste.android.ui.theme.AccentColor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * CopyPaste-npqx: reviewer follow-up from S3 (CopyPaste-myh8.3) — the
 * `commit() == false` branch of [Settings.saveScreenSettings] had no
 * automated test because real Android SharedPreferences.commit() cannot be
 * forced to fail from a JVM test. [FakeSharedPreferences.forceCommitFailure]
 * plus the [Settings] `prefsOverride` seam make it reproducible.
 *
 * Documented behavior of that branch (Settings.kt saveScreenSettings kdoc,
 * android-settings spec D5/M6): `saveScreenSettings` returns the raw
 * `commit()` result with no rollback/post-commit hook of its own — the
 * caller (SettingsScreen) is the one that must keep the dirty flag set /
 * surface an error instead of publishing committed-appearance state. This
 * test asserts the [Settings] side of that contract: a failed commit
 * propagates `false` to the caller, even though (mirroring real
 * SharedPreferencesImpl) the in-memory values were already applied.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SettingsSaveScreenSettingsCommitFailureTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    @Test
    fun `saveScreenSettings returns false when the underlying commit fails`() {
        val fakePrefs = FakeSharedPreferences(forceCommitFailure = true)
        val settings = Settings(context(), prefsOverride = fakePrefs)

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
            collectPublicIp = true,
            pasteAsPlainText = true,
            excludedAppBundleIds = emptyList(),
            showSensitiveWarnings = false,
            autoApplySyncedClip = false,
            maxFileSizeBytes = 12_345_678L,
            sensitiveTtlSecs = 42L,
            previewLines = 4,
            maxHistoryItems = 250,
        )

        // The documented contract: a failed disk commit propagates false, no
        // exception, no silent success.
        assertFalse(committed)
        // Mirrors real SharedPreferencesImpl: the in-memory map is updated
        // synchronously regardless of the disk-write outcome, so a fresh
        // Settings instance sharing the same fake prefs sees the new value —
        // it is the CALLER's job (SettingsScreen) to keep its own dirty flag
        // set when saveScreenSettings returns false, not Settings itself.
        assertEquals(4, Settings(context(), prefsOverride = fakePrefs).previewLines)
    }
}
