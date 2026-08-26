package com.copypaste.app

import android.content.ClipData
import android.content.ClipDescription
import android.os.PersistableBundle
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class ClipSensitivityTest {
    @Test
    fun aClipMarkedSensitiveIsNotCapturedAsText() {
        val clip = ClipData.newPlainText("label", "password-from-manager")
        val extras = PersistableBundle()
        extras.putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true)
        clip.description.extras = extras

        assertTrue(ClipSensitivity.isSensitive(clip))
        assertNull(ClipSensitivity.asText(clip))
    }

    @Test
    fun anOrdinaryClipIsCapturedAsText() {
        val clip = ClipData.newPlainText("label", "ordinary copy")

        assertFalse(ClipSensitivity.isSensitive(clip))
        assertEquals("ordinary copy", ClipSensitivity.asText(clip))
    }

    @Test
    fun theStringSensitiveExtraIsHonoured() {
        val clip = ClipData.newPlainText("label", "secret")
        val extras = PersistableBundle()
        extras.putBoolean("android.content.extra.IS_SENSITIVE", true)
        clip.description.extras = extras

        assertTrue(ClipSensitivity.isSensitive(clip))
        assertNull(ClipSensitivity.asText(clip))
    }
}
