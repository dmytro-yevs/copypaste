package com.copypaste.app

import android.app.Activity
import android.app.KeyguardManager
import android.content.Context
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.UserNotAuthenticatedException
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey

/**
 * Verifies the device owner with biometrics or the screen-lock credential for
 * one activity. Uses only framework APIs; the host activity forwards
 * [onActivityResult] for the confirm-credential screen used before API 29.
 * A prompt's success counts only once a Keystore key that requires user
 * authentication encrypts a fresh challenge, so a forged callback is refused.
 */
internal class DeviceAuthenticator(
    private val activity: Activity,
    private val keys: VerificationKeys = KeystoreVerificationKeys,
) {
    enum class Availability(val wire: String) {
        Available("available"),
        NotConfigured("notConfigured"),
    }

    enum class Outcome(val wire: String) {
        Success("success"),
        Cancelled("cancelled"),
        Unavailable("unavailable"),
        LockedOut("lockedOut"),
    }

    enum class Prompt { BiometricOrCredential, BiometricWithCredentialFallback, ConfirmCredential }

    private var completion: ((Outcome) -> Unit)? = null
    private var cancellation: CancellationSignal? = null

    val busy: Boolean get() = completion != null

    /** Returns false without prompting while another verification is open. */
    fun authenticate(reason: String, completion: (Outcome) -> Unit): Boolean {
        if (busy) return false
        this.completion = completion
        val title = activity.getString(R.string.device_auth_title)
        val started = runCatching {
            when (prompt(Build.VERSION.SDK_INT)) {
                Prompt.ConfirmCredential -> confirmCredential(title, reason)
                else -> biometricPrompt(title, reason)
            }
        }.getOrDefault(false)
        if (!started) finish(Outcome.Unavailable)
        return true
    }

    fun onActivityResult(requestCode: Int, resultCode: Int): Boolean {
        if (requestCode != CONFIRM_CREDENTIAL_REQUEST) return false
        finish(if (resultCode == Activity.RESULT_OK) timeBoundProof() else Outcome.Cancelled)
        return true
    }

    /** Closes an open prompt without reporting; the host is going away. */
    fun dispose() {
        completion = null
        cancellation?.cancel()
        cancellation = null
    }

    private fun confirmCredential(title: String, reason: String): Boolean {
        val intent = activity.getSystemService(KeyguardManager::class.java)
            ?.createConfirmDeviceCredentialIntent(title, reason) ?: return false
        @Suppress("DEPRECATION")
        activity.startActivityForResult(intent, CONFIRM_CREDENTIAL_REQUEST)
        return true
    }

    @Suppress("DEPRECATION", "NewApi")
    private fun biometricPrompt(title: String, reason: String): Boolean {
        val builder = BiometricPrompt.Builder(activity).setTitle(title).setDescription(reason)
        val cipher = if (prompt(Build.VERSION.SDK_INT) == Prompt.BiometricOrCredential) {
            builder.setAllowedAuthenticators(
                BiometricManager.Authenticators.BIOMETRIC_STRONG or
                    BiometricManager.Authenticators.DEVICE_CREDENTIAL,
            )
            keyCipher(perUse = true)
        } else {
            builder.setDeviceCredentialAllowed(true)
            null
        }
        val signal = CancellationSignal()
        val callback = object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                val returned = result.cryptoObject?.cipher
                finish(if (cipher == null) timeBoundProof() else cryptoProof(cipher, returned))
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) =
                finish(outcome(errorCode))
        }
        cancellation = signal
        val biometric = builder.build()
        if (cipher == null) {
            biometric.authenticate(signal, activity.mainExecutor, callback)
        } else {
            biometric.authenticate(BiometricPrompt.CryptoObject(cipher), signal, activity.mainExecutor, callback)
        }
        return true
    }

    /**
     * Credential prompts before API 30 cannot carry a CryptoObject, so these
     * use a key that only works for a few seconds after the owner verified.
     */
    private fun timeBoundProof(): Outcome = prove { keyCipher(perUse = false) }

    /** An invalidated key (lock screen or biometrics changed) is recreated once. */
    private fun keyCipher(perUse: Boolean): Cipher = try {
        keys.cipher(perUse)
    } catch (_: KeyPermanentlyInvalidatedException) {
        keys.delete(perUse)
        keys.cipher(perUse)
    }

    private fun finish(outcome: Outcome) {
        cancellation = null
        val completion = completion ?: return
        this.completion = null
        completion(outcome)
    }

    companion object {
        const val CONFIRM_CREDENTIAL_REQUEST = 48124
        private const val CHALLENGE_BYTES = 32

        /** Succeeds only when the prompt returned the cipher it was handed and that cipher still works. */
        internal fun cryptoProof(handed: Cipher, returned: Cipher?): Outcome =
            if (returned === handed) prove { handed } else Outcome.Cancelled

        /** Not verified when the key refuses to encrypt a fresh challenge. */
        private fun prove(cipher: () -> Cipher): Outcome = try {
            cipher().doFinal(ByteArray(CHALLENGE_BYTES).also(SecureRandom()::nextBytes))
            Outcome.Success
        } catch (_: UserNotAuthenticatedException) {
            Outcome.Cancelled
        } catch (_: Exception) {
            Outcome.Unavailable
        }

        /** API 28 BiometricPrompt cannot fall back to the credential, so it confirms the credential instead. */
        fun prompt(sdk: Int): Prompt = when {
            sdk >= Build.VERSION_CODES.R -> Prompt.BiometricOrCredential
            sdk == Build.VERSION_CODES.Q -> Prompt.BiometricWithCredentialFallback
            else -> Prompt.ConfirmCredential
        }

        /** Every prompt accepts the screen-lock credential, so a secure lock screen is enough. */
        fun availability(context: Context): Availability =
            if (context.getSystemService(KeyguardManager::class.java)?.isDeviceSecure == true) {
                Availability.Available
            } else {
                Availability.NotConfigured
            }

        fun outcome(errorCode: Int): Outcome = when (errorCode) {
            BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT,
            BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT_PERMANENT -> Outcome.LockedOut
            BiometricPrompt.BIOMETRIC_ERROR_HW_UNAVAILABLE,
            BiometricPrompt.BIOMETRIC_ERROR_HW_NOT_PRESENT,
            BiometricPrompt.BIOMETRIC_ERROR_NO_BIOMETRICS,
            BiometricPrompt.BIOMETRIC_ERROR_NO_DEVICE_CREDENTIAL -> Outcome.Unavailable
            else -> Outcome.Cancelled
        }
    }
}

/** Keystore keys that work only after the owner verifies; host tests fake them. */
internal interface VerificationKeys {
    /** An encrypting cipher; creates the key when it is missing. */
    fun cipher(perUse: Boolean): Cipher

    fun delete(perUse: Boolean)
}

/** Non-exportable AES keys in AndroidKeyStore; they guard nothing but the proof. */
internal object KeystoreVerificationKeys : VerificationKeys {
    private const val PROVIDER = "AndroidKeyStore"
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val VALIDITY_SECONDS = 10

    override fun cipher(perUse: Boolean): Cipher {
        val key = store().getKey(alias(perUse), null) as? SecretKey ?: generate(perUse)
        return Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, key) }
    }

    override fun delete(perUse: Boolean) = store().deleteEntry(alias(perUse))

    private fun store() = KeyStore.getInstance(PROVIDER).apply { load(null) }

    private fun alias(perUse: Boolean) = if (perUse) "device_auth_per_use" else "device_auth_time_bound"

    @Suppress("DEPRECATION", "NewApi")
    private fun generate(perUse: Boolean): SecretKey {
        val spec = KeyGenParameterSpec.Builder(alias(perUse), KeyProperties.PURPOSE_ENCRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setUserAuthenticationRequired(true)
            .apply {
                if (perUse) {
                    setUserAuthenticationParameters(
                        0,
                        KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL,
                    )
                } else {
                    setUserAuthenticationValidityDurationSeconds(VALIDITY_SECONDS)
                }
            }
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
            .apply { init(spec) }
            .generateKey()
    }
}
