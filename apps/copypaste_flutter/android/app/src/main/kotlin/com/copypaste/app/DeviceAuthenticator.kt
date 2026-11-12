package com.copypaste.app

import android.app.Activity
import android.app.KeyguardManager
import android.content.Context
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal

/**
 * Verifies the device owner with biometrics or the screen-lock credential for
 * one activity. Uses only framework APIs; the host activity forwards
 * [onActivityResult] for the confirm-credential screen used before API 29.
 */
internal class DeviceAuthenticator(private val activity: Activity) {
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
        finish(if (resultCode == Activity.RESULT_OK) Outcome.Success else Outcome.Cancelled)
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
        if (prompt(Build.VERSION.SDK_INT) == Prompt.BiometricOrCredential) {
            builder.setAllowedAuthenticators(
                BiometricManager.Authenticators.BIOMETRIC_STRONG or
                    BiometricManager.Authenticators.DEVICE_CREDENTIAL,
            )
        } else {
            builder.setDeviceCredentialAllowed(true)
        }
        val signal = CancellationSignal()
        cancellation = signal
        builder.build().authenticate(
            signal,
            activity.mainExecutor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) =
                    finish(Outcome.Success)

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) =
                    finish(outcome(errorCode))
            },
        )
        return true
    }

    private fun finish(outcome: Outcome) {
        cancellation = null
        val completion = completion ?: return
        this.completion = null
        completion(outcome)
    }

    companion object {
        const val CONFIRM_CREDENTIAL_REQUEST = 48124

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
