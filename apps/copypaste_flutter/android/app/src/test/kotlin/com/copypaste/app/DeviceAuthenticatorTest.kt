package com.copypaste.app

import android.app.Activity
import android.app.KeyguardManager
import android.hardware.biometrics.BiometricPrompt
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
class DeviceAuthenticatorTest {
    @Test fun eachApiLevelUsesAPromptThatAcceptsTheScreenLockCredential() {
        assertEquals(DeviceAuthenticator.Prompt.ConfirmCredential, DeviceAuthenticator.prompt(24))
        // API 28 BiometricPrompt has no credential fallback.
        assertEquals(DeviceAuthenticator.Prompt.ConfirmCredential, DeviceAuthenticator.prompt(28))
        assertEquals(DeviceAuthenticator.Prompt.BiometricWithCredentialFallback, DeviceAuthenticator.prompt(29))
        assertEquals(DeviceAuthenticator.Prompt.BiometricOrCredential, DeviceAuthenticator.prompt(30))
        assertEquals(DeviceAuthenticator.Prompt.BiometricOrCredential, DeviceAuthenticator.prompt(36))
    }

    @Test fun promptErrorsMapToLockoutUnavailableOrCancelled() {
        assertEquals(
            DeviceAuthenticator.Outcome.LockedOut,
            DeviceAuthenticator.outcome(BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT_PERMANENT),
        )
        assertEquals(
            DeviceAuthenticator.Outcome.Unavailable,
            DeviceAuthenticator.outcome(BiometricPrompt.BIOMETRIC_ERROR_NO_DEVICE_CREDENTIAL),
        )
        assertEquals(
            DeviceAuthenticator.Outcome.Cancelled,
            DeviceAuthenticator.outcome(BiometricPrompt.BIOMETRIC_ERROR_USER_CANCELED),
        )
    }

    @Config(sdk = [26])
    @Test fun beforeBiometricPromptTheConfirmCredentialScreenDecides() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val keyguard = shadowOf(activity.getSystemService(KeyguardManager::class.java))
        val authenticator = DeviceAuthenticator(activity)
        val outcomes = mutableListOf<DeviceAuthenticator.Outcome>()

        keyguard.setIsDeviceSecure(false)
        assertEquals(DeviceAuthenticator.Availability.NotConfigured, DeviceAuthenticator.availability(activity))
        assertTrue(authenticator.authenticate("Unlock secret items") { outcomes += it })
        assertEquals(listOf(DeviceAuthenticator.Outcome.Unavailable), outcomes)

        keyguard.setIsDeviceSecure(true)
        assertEquals(DeviceAuthenticator.Availability.Available, DeviceAuthenticator.availability(activity))
        assertTrue(authenticator.authenticate("Unlock secret items") { outcomes += it })
        assertFalse(authenticator.authenticate("Copy a secret item") { outcomes += it })
        val started = shadowOf(activity).nextStartedActivityForResult
        assertEquals("android.app.action.CONFIRM_DEVICE_CREDENTIAL", started.intent.action)
        assertEquals(DeviceAuthenticator.CONFIRM_CREDENTIAL_REQUEST, started.requestCode)

        assertFalse(authenticator.onActivityResult(started.requestCode + 1, Activity.RESULT_OK))
        assertTrue(authenticator.onActivityResult(started.requestCode, Activity.RESULT_OK))
        assertEquals(
            listOf(DeviceAuthenticator.Outcome.Unavailable, DeviceAuthenticator.Outcome.Success),
            outcomes,
        )
        assertNull(shadowOf(activity).nextStartedActivityForResult)
    }
}
