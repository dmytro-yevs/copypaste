package com.copypaste.app

import android.content.Context
import android.provider.Settings
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class ClipboardNoticeSettingTest {
    private val context: Context get() = RuntimeEnvironment.getApplication()

    @Before
    fun clear() = ClipboardNoticeSetting.stopObserving(context)

    @After
    fun tearDown() = ClipboardNoticeSetting.stopObserving(context)

    /**
     * The probe rides on every drain, so an uncached read is a ContentResolver
     * query 86,400 times a day. Written behind the cache's back, so a second
     * read would see the new value.
     */
    @Test
    fun theSettingIsReadOnceWhileTheObserverCanRetractIt() {
        ClipboardNoticeSetting.observe(context)
        write(suppressed = true)
        assertTrue(ClipboardNoticeSetting.suppressed(context))

        write(suppressed = false)

        assertTrue(
            "the probe went back to the ContentResolver",
            ClipboardNoticeSetting.suppressed(context),
        )
    }

    /**
     * Our own write goes through the Shizuku user service and the observer arrives after
     * we have already answered. Reporting a suppression that did not happen is
     * the same class of lie as reporting capture that is not running.
     */
    @Test
    fun ourOwnWriteDropsTheCacheBeforeTheProbeIsAnswered() {
        ClipboardNoticeSetting.observe(context)
        write(suppressed = true)
        assertTrue(ClipboardNoticeSetting.suppressed(context))

        write(suppressed = false)
        ClipboardNoticeSetting.invalidate()

        assertFalse(ClipboardNoticeSetting.suppressed(context))
    }

    /** Without an observer the value would freeze at its first reading. */
    @Test
    fun nothingIsRememberedWhileNoObserverCouldRetractIt() {
        write(suppressed = true)
        assertTrue(ClipboardNoticeSetting.suppressed(context))

        write(suppressed = false)

        assertFalse(ClipboardNoticeSetting.suppressed(context))
    }

    /** The default is "the notice is shown", which is the safe direction. */
    @Test
    fun anUnsetSettingIsNotSuppressed() {
        assertFalse(ClipboardNoticeSetting.suppressed(context))
    }

    private fun write(suppressed: Boolean) {
        Settings.Secure.putInt(
            context.contentResolver,
            ClipboardNoticeSetting.NAME,
            if (suppressed) 0 else 1,
        )
    }
}
