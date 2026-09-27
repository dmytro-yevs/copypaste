package com.copypaste.app

import android.content.Context
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class ClipboardNoticeSettingTest {
    private val context: Context get() = RuntimeEnvironment.getApplication()

    @Before
    fun clear() = ClipboardNoticeSetting.stopObserving(context)

    @After
    fun tearDown() = ClipboardNoticeSetting.stopObserving(context)

    @Test
    fun verifiedPrivilegedReadIsKeptForProbes() {
        ClipboardNoticeSetting.observe(context)
        ClipboardNoticeSetting.publishForTest(true)

        assertTrue(ClipboardNoticeSetting.suppressed(context))
    }

    @Test
    fun unavailableSettingIsNotReportedAsSuppressed() {
        ClipboardNoticeSetting.observe(context)
        ClipboardNoticeSetting.publishForTest(null)

        assertFalse(ClipboardNoticeSetting.suppressed(context))
        assertFalse(ClipboardNoticeSetting.suppressed(context))
        assertFalse(shouldRefreshClipboardNotice(observing = true, resolved = true))
    }

    @Test
    fun invalidationDoesNotRetainAClaimFromBeforeTheWrite() {
        ClipboardNoticeSetting.observe(context)
        ClipboardNoticeSetting.publishForTest(true)
        assertTrue(ClipboardNoticeSetting.suppressed(context))

        ClipboardNoticeSetting.invalidate()

        assertFalse(ClipboardNoticeSetting.suppressed(context))
    }
}
