package com.copypaste.android.ui.theme

import android.os.Build
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * android-design-system "Translucency and blur rendering policy" requirement
 * (D7): real backdrop blur only on API 31+ AND translucency enabled; opaque
 * fallback for legacy API or translucency off.
 */
class BlurPolicyTest {

    @Test
    fun `modern device with translucency on resolves to real backdrop`() {
        assertEquals(BlurMode.REAL_BACKDROP, resolveBlurMode(translucencyEnabled = true, sdkInt = Build.VERSION_CODES.S))
        assertEquals(BlurMode.REAL_BACKDROP, resolveBlurMode(translucencyEnabled = true, sdkInt = 34))
    }

    @Test
    fun `legacy API falls back to opaque even with translucency on`() {
        assertEquals(BlurMode.OPAQUE_FALLBACK, resolveBlurMode(translucencyEnabled = true, sdkInt = 30))
        assertEquals(BlurMode.OPAQUE_FALLBACK, resolveBlurMode(translucencyEnabled = true, sdkInt = 26))
    }

    @Test
    fun `translucency off falls back to opaque regardless of API level`() {
        assertEquals(BlurMode.OPAQUE_FALLBACK, resolveBlurMode(translucencyEnabled = false, sdkInt = 34))
    }
}
