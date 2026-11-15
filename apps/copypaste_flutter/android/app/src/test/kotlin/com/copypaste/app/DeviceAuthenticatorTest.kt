package com.copypaste.app

import android.app.Activity
import android.app.KeyguardManager
import android.hardware.biometrics.BiometricPrompt
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.UserNotAuthenticatedException
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec
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
        val authenticator = DeviceAuthenticator(activity, FakeKeys())
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

    @Config(sdk = [26])
    @Test fun aConfirmedCredentialCountsOnlyWhenTheKeyAgrees() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        shadowOf(activity.getSystemService(KeyguardManager::class.java)).setIsDeviceSecure(true)
        val keys = FakeKeys(authenticated = false)
        val authenticator = DeviceAuthenticator(activity, keys)
        val outcomes = mutableListOf<DeviceAuthenticator.Outcome>()
        fun confirm() {
            authenticator.authenticate("Unlock secret items") { outcomes += it }
            authenticator.onActivityResult(DeviceAuthenticator.CONFIRM_CREDENTIAL_REQUEST, Activity.RESULT_OK)
        }

        // A forged RESULT_OK without a recent verification is refused.
        confirm()
        assertEquals(DeviceAuthenticator.Outcome.Cancelled, outcomes.last())

        // An invalidated key is recreated and the proof retried.
        keys.authenticated = true
        keys.invalidations = 1
        confirm()
        assertEquals(DeviceAuthenticator.Outcome.Success, outcomes.last())
        assertEquals(1, keys.deletions)

        // A key that stays invalid is recreated only once.
        keys.invalidations = Int.MAX_VALUE
        confirm()
        assertEquals(DeviceAuthenticator.Outcome.Unavailable, outcomes.last())
        assertEquals(2, keys.deletions)
    }

    @Test fun aBiometricSuccessCountsOnlyWithTheHandedCipher() {
        val keys = FakeKeys()
        val handed = keys.cipher(perUse = true)
        assertEquals(DeviceAuthenticator.Outcome.Cancelled, DeviceAuthenticator.cryptoProof(handed, null))
        assertEquals(
            DeviceAuthenticator.Outcome.Cancelled,
            DeviceAuthenticator.cryptoProof(handed, keys.cipher(perUse = true)),
        )
        assertEquals(DeviceAuthenticator.Outcome.Success, DeviceAuthenticator.cryptoProof(handed, handed))
        val uninitialised = Cipher.getInstance("AES/GCM/NoPadding")
        assertEquals(
            DeviceAuthenticator.Outcome.Unavailable,
            DeviceAuthenticator.cryptoProof(uninitialised, uninitialised),
        )
    }

    /** Software stand-in for AndroidKeyStore, which Robolectric lacks. */
    private class FakeKeys(var authenticated: Boolean = true, var invalidations: Int = 0) : VerificationKeys {
        var deletions = 0

        override fun cipher(perUse: Boolean): Cipher {
            if (invalidations > 0) throw KeyPermanentlyInvalidatedException()
            if (!perUse && !authenticated) throw UserNotAuthenticatedException()
            return Cipher.getInstance("AES/GCM/NoPadding")
                .apply { init(Cipher.ENCRYPT_MODE, SecretKeySpec(ByteArray(16), "AES")) }
        }

        override fun delete(perUse: Boolean) {
            deletions++
            invalidations--
        }
    }
}
