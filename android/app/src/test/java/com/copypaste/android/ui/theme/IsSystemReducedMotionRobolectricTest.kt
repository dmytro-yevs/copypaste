package com.copypaste.android.ui.theme

import android.provider.Settings as AndroidSettings
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * task 0.12/2.7 "Deps + CI wiring": proves the pinned Robolectric dependency
 * actually resolves and runs against this toolchain (design.md: "verified
 * empirically when S2 wires it"), using a real Context/ContentResolver
 * instead of the `isReturnDefaultValues`-stubbed one plain JVM unit tests
 * get — [isSystemReducedMotion] reads `Settings.Global.ANIMATOR_DURATION_SCALE`
 * via `context.contentResolver`, which the plain-JVM stub cannot exercise.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class IsSystemReducedMotionRobolectricTest {

    @Test
    fun `default animator duration scale is not reduced motion`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        // Robolectric's default ANIMATOR_DURATION_SCALE is unset/1x, not 0x.
        assertFalse(isSystemReducedMotion(context))
    }

    @Test
    fun `animator duration scale of zero is reduced motion`() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        AndroidSettings.Global.putFloat(context.contentResolver, AndroidSettings.Global.ANIMATOR_DURATION_SCALE, 0f)
        org.junit.Assert.assertTrue(isSystemReducedMotion(context))
    }
}
