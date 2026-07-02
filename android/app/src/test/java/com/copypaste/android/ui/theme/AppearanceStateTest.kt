package com.copypaste.android.ui.theme

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.copypaste.android.AppearanceDraft
import com.copypaste.android.Settings
import com.copypaste.android.ThemeMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * android-appearance D5: [resolveIsDark]'s pure `ThemeMode -> Boolean`
 * resolution (incl. "System reacts to OS change"), [AppearanceStore.publish]
 * as the sole app-wide write path, and [committedAppearanceFrom]'s
 * Settings->CommittedAppearance mapping.
 *
 * Deliberately does NOT assert on [AppearanceStore]'s `initialized` "only
 * once per process" behaviour directly — [AppearanceStore] is a JVM-wide
 * singleton whose state is not reset between test methods (same limitation as
 * the pre-existing `DevicesOnlineState`), so a test asserting exact seed
 * timing would be run-order-dependent. [committedAppearanceFrom] carries that
 * seeding LOGIC in a pure, order-independent, directly-testable form instead.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AppearanceStateTest {

    @Test
    fun `dark and light theme modes resolve regardless of the system setting`() {
        assertTrue(resolveIsDark(ThemeMode.DARK, systemInDark = false))
        assertTrue(resolveIsDark(ThemeMode.DARK, systemInDark = true))
        assertFalse(resolveIsDark(ThemeMode.LIGHT, systemInDark = true))
        assertFalse(resolveIsDark(ThemeMode.LIGHT, systemInDark = false))
    }

    @Test
    fun `system theme mode reacts to the OS dark-theme signal`() {
        assertTrue(resolveIsDark(ThemeMode.SYSTEM, systemInDark = true))
        assertFalse(resolveIsDark(ThemeMode.SYSTEM, systemInDark = false))
    }

    @Test
    fun `committedAppearanceFrom reads all three axes off Settings`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val settings = Settings(context)
        settings.themeMode = ThemeMode.LIGHT
        settings.accent = AccentColor.BLUE
        settings.translucency = false

        val appearance = committedAppearanceFrom(settings)

        assertEquals(ThemeMode.LIGHT, appearance.themeMode)
        assertEquals(AccentColor.BLUE, appearance.accent)
        assertFalse(appearance.translucency)
    }

    @Test
    fun `publish makes the new appearance immediately readable from committed`() {
        val published = CommittedAppearance(ThemeMode.LIGHT, AccentColor.TEAL, translucency = false)
        AppearanceStore.publish(published)
        assertEquals(published, AppearanceStore.committed.value)
    }

    @Test
    fun `publishing a structurally-equal appearance is a no-op value-wise`() {
        val appearance = CommittedAppearance(ThemeMode.DARK, AccentColor.ROSE, translucency = true)
        AppearanceStore.publish(appearance)
        AppearanceStore.publish(appearance.copy()) // new instance, same fields
        assertEquals(appearance, AppearanceStore.committed.value)
    }

    /**
     * S4 carried review finding (b), fixed in the S4 review-fix pass: D5 says
     * "Draft never feeds committed state before Save" — SettingsActivity's
     * Discard path is exactly "mutate local draft state, then never call
     * [AppearanceDraft.commit]". The draft/commit boundary is now the real,
     * production [AppearanceDraft] class (extracted from SettingsActivity's
     * Composable-local `mutableStateOf` fields — see its kdoc), so this test
     * exercises the ACTUAL production code path rather than a re-declared
     * local that merely echoes its own literals.
     */
    @Test
    fun `discarding a draft change never touches AppearanceStore committed state`() {
        val baseline = CommittedAppearance(ThemeMode.DARK, AccentColor.INDIGO, translucency = true)
        AppearanceStore.publish(baseline)

        // Mutate the draft's backing values (mirrors editing the Display tab),
        // then discard — i.e. AppearanceDraft.commit() is deliberately never
        // called below.
        var draftThemeMode = ThemeMode.SYSTEM
        var draftAccent = AccentColor.TEAL
        var draftTranslucency = true
        AppearanceDraft(
            themeMode = { draftThemeMode },
            accent = { draftAccent },
            translucency = { draftTranslucency },
        )
        draftThemeMode = ThemeMode.LIGHT
        draftAccent = AccentColor.ROSE
        draftTranslucency = false

        // Discard: no `commit()` call was ever made — AppearanceStore.committed
        // must still be exactly the pre-edit baseline.
        assertEquals(baseline, AppearanceStore.committed.value)
    }

    @Test
    fun `committing a draft publishes its current values app-wide`() {
        AppearanceStore.publish(CommittedAppearance(ThemeMode.DARK, AccentColor.INDIGO, translucency = true))

        var draftThemeMode = ThemeMode.LIGHT
        var draftAccent = AccentColor.ROSE
        var draftTranslucency = false
        val draft = AppearanceDraft(
            themeMode = { draftThemeMode },
            accent = { draftAccent },
            translucency = { draftTranslucency },
        )

        draft.commit()

        assertEquals(
            CommittedAppearance(ThemeMode.LIGHT, AccentColor.ROSE, translucency = false),
            AppearanceStore.committed.value,
        )

        // A later, un-committed mutation of the draft's backing values must
        // NOT retroactively change what was already published.
        draftThemeMode = ThemeMode.SYSTEM
        assertEquals(ThemeMode.LIGHT, AppearanceStore.committed.value.themeMode)
    }
}
