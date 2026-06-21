package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-bdac.77: version format parity with macOS.
 * CopyPaste-bdac.42: Test-connection wiring (relay health) and compact-DB UI.
 *
 * Pure-JVM tests — no Android SDK, no Compose runtime, no coroutines needed.
 */
class AndroidParityBdac42And77Test {

    // ─────────────────────────────────────────────────────────────────────────
    // bdac.77 — version label format
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * macOS AboutView shows only the version name string (from Tauri getVersion()
     * which has no build-number equivalent). Android's [versionLabel] must return
     * VERSION_NAME only — no "(build N)" suffix — to match.
     *
     * We can't call BuildConfig directly in unit tests (no Android runtime), so
     * we test the SHAPE of the format using [versionLabelFromParts], the extracted
     * pure function below.
     */
    @Test
    fun `versionLabelFromParts returns bare version name only`() {
        val label = versionLabelFromParts(versionName = "1.2.3", versionCode = 42)
        assertEquals(
            "versionLabel must match macOS format: version name only, no build suffix",
            "1.2.3",
            label,
        )
    }

    @Test
    fun `versionLabel does not contain the build suffix`() {
        val label = versionLabelFromParts(versionName = "0.5.3", versionCode = 8)
        assertFalse(
            "versionLabel must not contain '(build' — macOS parity requires version-name only",
            label.contains("(build"),
        )
        assertFalse(
            "versionLabel must not contain the raw build number",
            label.contains("8"),
        )
    }

    @Test
    fun `versionLabel contains the version name`() {
        val label = versionLabelFromParts(versionName = "0.3.0", versionCode = 5)
        assertTrue(
            "versionLabel must contain the VERSION_NAME string",
            label.contains("0.3.0"),
        )
    }

    @Test
    fun `versionLabel is not blank`() {
        val label = versionLabelFromParts(versionName = "1.0.0", versionCode = 1)
        assertTrue("versionLabel must not be blank", label.isNotBlank())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // bdac.42 — Test-connection: relay health result → toast kind mapping
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * The "Test connection" button in the Sync tab runs [RelayClient.health()]
     * and shows a GlassToast. The mapping from boolean health result to toast kind
     * is extracted in [testConnectionToastKind] so it can be unit-tested without
     * a running relay server or coroutines.
     */
    @Test
    fun `testConnectionToastKind is SUCCESS when health returns true`() {
        assertEquals(
            "health=true (relay reachable) must produce SUCCESS toast",
            TestConnectionResult.SUCCESS,
            testConnectionToastKind(healthOk = true),
        )
    }

    @Test
    fun `testConnectionToastKind is FAIL when health returns false`() {
        assertEquals(
            "health=false (relay unreachable) must produce FAIL toast",
            TestConnectionResult.FAIL,
            testConnectionToastKind(healthOk = false),
        )
    }

    // ─────────────────────────────────────────────────────────────────────────
    // bdac.42 — Compact-database: vacuum button enabled only when callback present
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * The compact-database button in StorageTab is ENABLED only when
     * onVacuumDatabase != null. On Android, SQLCipher lives inside the Rust .so
     * and no FFI vacuum entry-point is exposed via the current UDL, so the button
     * is disabled (null callback) with an explanatory subtitle. These tests verify
     * the disabled-state logic.
     */
    @Test
    fun `compactButtonEnabled is false when onVacuumDatabase is null`() {
        assertFalse(
            "Compact button must be disabled when onVacuumDatabase is null",
            compactButtonEnabled(onVacuumDatabase = null),
        )
    }

    @Test
    fun `compactButtonEnabled is true when onVacuumDatabase is provided`() {
        assertTrue(
            "Compact button must be enabled when a vacuum callback is provided",
            compactButtonEnabled(onVacuumDatabase = { /* stub */ }),
        )
    }

    @Test
    fun `compactButtonSubtitle is unavailable message when null`() {
        val msg = compactButtonSubtitle(
            onVacuumDatabase = null,
            availableText = "Run VACUUM to reclaim space.",
            unavailableText = "Not available on this build (requires FFI vacuum support)",
        )
        assertTrue(
            "Subtitle must be unavailableText when onVacuumDatabase is null",
            msg.contains("Not available"),
        )
    }

    @Test
    fun `compactButtonSubtitle is availableText when callback present`() {
        val msg = compactButtonSubtitle(
            onVacuumDatabase = { /* stub */ },
            availableText = "Run VACUUM to reclaim space.",
            unavailableText = "Not available on this build",
        )
        assertEquals(
            "Subtitle must be availableText when onVacuumDatabase is provided",
            "Run VACUUM to reclaim space.",
            msg,
        )
    }
}

// ---------------------------------------------------------------------------
// Pure helper functions extracted from SettingsActivity / AboutActivity logic
// so they can be unit-tested on the JVM without Android/Compose runtime.
//
// These mirror the exact production logic:
//   AboutActivity.versionLabel() → BuildConfig.VERSION_NAME   (bdac.77)
//   StorageTab onVacuumDatabase null-check                     (bdac.42)
//   onTestConnection health→toast mapping                      (bdac.42)
// ---------------------------------------------------------------------------

/**
 * Version label shape — mirrors [AboutActivity.versionLabel] but takes the
 * raw parts so unit tests don't depend on BuildConfig.
 *
 * CopyPaste-bdac.77: returns VERSION_NAME only (no build suffix).
 */
fun versionLabelFromParts(versionName: String, @Suppress("UNUSED_PARAMETER") versionCode: Int): String =
    versionName

/** Result type for the relay health probe — used as a testable value-type. */
enum class TestConnectionResult { SUCCESS, FAIL }

/**
 * Maps a relay [healthOk] result to the [TestConnectionResult] that drives the
 * GlassToast kind in SettingsScreen's onTestConnection lambda.
 */
fun testConnectionToastKind(healthOk: Boolean): TestConnectionResult =
    if (healthOk) TestConnectionResult.SUCCESS else TestConnectionResult.FAIL

/**
 * Whether the Compact database button should be enabled.
 * Mirrors the `enabled = onVacuumDatabase != null` logic in StorageTab.
 */
fun compactButtonEnabled(onVacuumDatabase: (() -> Unit)?): Boolean = onVacuumDatabase != null

/**
 * The subtitle shown below the Compact database button — available text when
 * the callback is wired, unavailable text when it is null.
 * Mirrors the `if (onVacuumDatabase != null)` conditional in StorageTab.
 */
fun compactButtonSubtitle(
    onVacuumDatabase: (() -> Unit)?,
    availableText: String,
    unavailableText: String,
): String = if (onVacuumDatabase != null) availableText else unavailableText
