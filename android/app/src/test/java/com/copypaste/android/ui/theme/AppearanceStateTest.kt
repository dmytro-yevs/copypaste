package com.copypaste.android.ui.theme

import android.content.Context
import androidx.test.core.app.ApplicationProvider
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
}
