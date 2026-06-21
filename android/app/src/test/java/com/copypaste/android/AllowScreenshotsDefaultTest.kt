package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * CopyPaste-1g00: allowScreenshots pref default must be false (FLAG_SECURE ON by default).
 *
 * Root cause: audit-verify found Settings.kt had getBoolean("allow_screenshots", true)
 * — screenshots allowed (FLAG_SECURE OFF) on a fresh install. The KDoc stated default
 * should be false (protection ON). Fix: changed default to false.
 *
 * Security invariant: a missing pref (first install, cleared-prefs) must NEVER silently
 * expose clipboard contents via screenshots/recents. Only an explicit user opt-in sets
 * the pref to true.
 */
class AllowScreenshotsDefaultTest {

    private val settingsSrc: String by lazy {
        val candidates = listOf(
            "android/app/src/main/java/com/copypaste/android/Settings.kt",
            "../android/app/src/main/java/com/copypaste/android/Settings.kt",
            "../../android/app/src/main/java/com/copypaste/android/Settings.kt",
        )
        candidates
            .map { File(it) }
            .firstOrNull { it.exists() }
            ?.readText()
            ?: error("Could not locate Settings.kt from test working directory")
    }

    @Test
    fun allowScreenshots_defaultMustBeFalse() {
        // The getter line should read: getBoolean("allow_screenshots", false)
        // Match it exactly to prevent a regression back to `true`.
        assertTrue(
            "Settings.allowScreenshots getter must default to false " +
                "(FLAG_SECURE ON = protection on fresh install)",
            settingsSrc.contains("""getBoolean("allow_screenshots", false)"""),
        )
    }

    @Test
    fun allowScreenshots_defaultMustNotBeTrue() {
        // Negative guard: the old insecure default must not exist anywhere in the getter.
        // (This would only fire if someone reverted the fix.)
        assertFalse(
            "Settings.allowScreenshots getter must NOT default to true — " +
                "that would leave clipboard exposed on fresh install",
            settingsSrc.contains("""getBoolean("allow_screenshots", true)"""),
        )
    }
}
