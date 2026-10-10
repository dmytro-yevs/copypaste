package com.copypaste.app

import android.content.ComponentName
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class InferenceWorkerContractTest {
    @Test
    fun inferenceIsPrivateAndOwnsADedicatedProcess() {
        val application = RuntimeEnvironment.getApplication()
        val info = application.packageManager.getServiceInfo(
            ComponentName(application.packageName, "com.copypaste.app.InferenceWorkerService"), 0,
        )
        assertFalse(info.exported)
        assertEquals(application.packageName + ":inference", info.processName)
        assertEquals(application.applicationInfo.uid, info.applicationInfo.uid)
    }
}
