package com.copypaste.app

import android.os.Looper
import android.view.View
import android.view.WindowManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ClipboardFloatingActivityTest {
    @Test
    fun focusedWindowHasContentAndFinishesAfterFocusDispatch() {
        val controller = Robolectric.buildActivity(ClipboardFloatingActivity::class.java).create()
        val activity = controller.get()
        val attributes = activity.window.attributes
        assertEquals(1, attributes.width)
        assertEquals(1, attributes.height)
        assertTrue(attributes.flags and WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE != 0)
        assertFalse(attributes.flags and WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE != 0)
        assertNotNull(activity.findViewById<View>(android.R.id.content))

        activity.onWindowFocusChanged(true)
        activity.onWindowFocusChanged(true)
        assertFalse(activity.isFinishing)
        shadowOf(Looper.getMainLooper()).idle()
        assertTrue(activity.isFinishing)
        controller.destroy()
    }
}
