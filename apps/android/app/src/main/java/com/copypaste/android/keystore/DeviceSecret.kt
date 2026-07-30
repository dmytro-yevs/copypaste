package com.copypaste.android.keystore

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import java.io.File
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The 32-byte device secret every key in CopyPaste is derived from, held so
 * that it never exists on disk in the clear.
 *
 * ## Why this is written out rather than taken from a library
 *
 * `CLAUDE.md` rule 1 says reach for a maintained package first and write down
 * the reason when you do not. The reason here is exemption 1 — no maintained
 * package provides this any more. `androidx.security:security-crypto` was the
 * answer; its 1.0.0 line is deprecated and its 1.1.0 line sat in alpha for
 * years without shipping, so depending on it means depending on something
 * Google has said it is not maintaining. What is left is the platform API, and
 * that is what this file calls.
 *
 * ## What actually protects the secret
 *
 * The Android Keystore will not hand back raw key material — a hardware-backed
 * key is non-exportable by design, which is the whole point of it. So the
 * Keystore does not *hold* the device secret; it holds a wrapping key that
 * never leaves the security hardware, and the secret is stored beside it as
 * ciphertext:
 *
 * ```text
 * AndroidKeyStore ── AES-256-GCM wrapping key (non-exportable)
 *                       │
 *                       └─ unwraps ─▶ device-secret.v2.bin  (IV ‖ ciphertext ‖ tag)
 *                                        │
 *                                        └─ 32 bytes ─▶ Keyring (HKDF) ─▶ db key, item key
 * ```
 *
 * Copying `device-secret.v2.bin` off the device — by a backup, by an ADB pull,
 * by another app that finds a way into this app's storage — yields nothing.
 * The file is only meaningful next to a key that cannot be copied.
 *
 * ## The rule this file must not break
 *
 * **A failure to unwrap is never a licence to mint a fresh secret.** Manifest
 * 02 I-20: only an unambiguous "there is no entry" authorises creation. If the
 * wrapped file is present but will not open, minting a new secret would leave a
 * SQLCipher database on disk that nothing can ever decrypt again, and would
 * report it as a clean first run. So this throws, the app says the history is
 * locked, and the encrypted database is left exactly where it is.
 */
object DeviceSecret {

    /** Bytes of device secret. Matches `copypaste_core::crypto::KEY_LEN`. */
    private const val SECRET_BYTES = 32

    /**
     * Keystore alias of the wrapping key.
     *
     * `-v2` for the same reason the database file carries it (`CLAUDE.md`
     * rule 3): a v0.4.x install's key material is a different alias, is never
     * touched, and still works if the user goes back.
     */
    private const val WRAP_KEY_ALIAS = "com.copypaste.device-secret-wrap-v2"

    /** The wrapped secret, in the app's private (scoped) storage. */
    private const val WRAPPED_FILE = "device-secret.v2.bin"

    private const val KEYSTORE = "AndroidKeyStore"
    private const val TRANSFORM = "AES/GCM/NoPadding"

    /** 128-bit GCM tag — the maximum, and what the platform defaults to. */
    private const val TAG_BITS = 128

    /**
     * Load the device secret, creating it on genuine first run.
     *
     * @param filesDir the app's private directory ([android.content.Context.getFilesDir]).
     * @throws DeviceSecretUnavailable when a secret exists but cannot be
     *   unwrapped. Never returns a freshly minted secret in that case.
     */
    fun loadOrCreate(filesDir: File): ByteArray {
        val wrapped = File(filesDir, WRAPPED_FILE)

        if (!wrapped.exists()) {
            // The one unambiguous "no entry exists". Everything else throws.
            return create(wrapped)
        }

        return try {
            unwrap(wrapped.readBytes())
        } catch (e: KeyPermanentlyInvalidatedException) {
            // The user removed and re-added their screen lock, or restored onto
            // new hardware. The wrapping key is gone for good; the database is
            // not recoverable and must not be silently replaced.
            throw DeviceSecretUnavailable(Reason.KEY_INVALIDATED, e)
        } catch (e: GeneralSecurityException) {
            throw DeviceSecretUnavailable(Reason.UNREADABLE, e)
        } catch (e: IllegalArgumentException) {
            throw DeviceSecretUnavailable(Reason.UNREADABLE, e)
        }
    }

    /**
     * Delete the secret and the wrapping key.
     *
     * Only for the "forget everything on this device" path, and only alongside
     * deleting the database: on its own this makes the history permanently
     * unreadable rather than gone, which is a worse state than either.
     */
    fun destroy(filesDir: File) {
        File(filesDir, WRAPPED_FILE).delete()
        runCatching {
            KeyStore.getInstance(KEYSTORE).apply { load(null) }.deleteEntry(WRAP_KEY_ALIAS)
        }
    }

    // ---------------------------------------------------------------- internals

    private fun create(wrapped: File): ByteArray {
        val secret = ByteArray(SECRET_BYTES)
        // The platform CSPRNG. Not `Random`, and not seeded by us — seeding it
        // is the classic way to make it worse.
        SecureRandom().nextBytes(secret)

        val cipher = Cipher.getInstance(TRANSFORM).apply {
            // No IV is supplied: `setRandomizedEncryptionRequired` is on by
            // default, so the Keystore generates one and refuses a caller's.
            // That is the guarantee that no two wraps share an IV.
            init(Cipher.ENCRYPT_MODE, wrappingKey())
        }
        val iv = cipher.iv
        val ciphertext = cipher.doFinal(secret)

        // Written atomically. A torn write here loses the whole history, and
        // "the file is half there" is indistinguishable from "the file is
        // damaged", which is the one state this class must never guess about.
        val tmp = File(wrapped.parentFile, "${wrapped.name}.tmp")
        tmp.writeBytes(byteArrayOf(iv.size.toByte()) + iv + ciphertext)
        check(tmp.renameTo(wrapped)) { "could not place the wrapped device secret" }

        return secret
    }

    private fun unwrap(blob: ByteArray): ByteArray {
        require(blob.isNotEmpty()) { "empty" }
        val ivLen = blob[0].toInt()
        require(ivLen in 1..16 && blob.size > 1 + ivLen) { "malformed" }

        val iv = blob.copyOfRange(1, 1 + ivLen)
        val ciphertext = blob.copyOfRange(1 + ivLen, blob.size)

        val key = existingWrappingKey() ?: throw DeviceSecretUnavailable(Reason.KEY_INVALIDATED)
        val secret = Cipher.getInstance(TRANSFORM).run {
            init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, iv))
            doFinal(ciphertext)
        }

        // GCM already authenticated it; this only catches a file written by
        // something that is not this code.
        require(secret.size == SECRET_BYTES) { "wrong length" }
        return secret
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }

    private fun existingWrappingKey(): SecretKey? =
        keyStore().getKey(WRAP_KEY_ALIAS, null) as? SecretKey

    private fun wrappingKey(): SecretKey {
        existingWrappingKey()?.let { return it }

        val spec = KeyGenParameterSpec.Builder(
            WRAP_KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .apply {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    // Ask for the secure element when the device has one. It is
                    // a request, not a requirement: `setIsStrongBoxBacked(true)`
                    // makes key generation *throw* on hardware without
                    // StrongBox, so the fallback below is not optional.
                    setIsStrongBoxBacked(true)
                }
            }
            .build()

        return try {
            generate(spec)
        } catch (_: Exception) {
            // No StrongBox. TEE-backed, or software-backed on a device with
            // neither — still better than a key sitting in our own file.
            generate(
                KeyGenParameterSpec.Builder(
                    WRAP_KEY_ALIAS,
                    KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                )
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build(),
            )
        }
    }

    private fun generate(spec: KeyGenParameterSpec): SecretKey =
        KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
            .apply { init(spec) }
            .generateKey()

    /** Why the secret could not be produced. */
    enum class Reason {
        /** The Keystore key is gone; the existing database can never be opened. */
        KEY_INVALIDATED,

        /** The wrapped file is damaged, truncated, or not ours. */
        UNREADABLE,
    }
}

/**
 * The device secret exists but cannot be recovered.
 *
 * Carries no path and no cause text that could reach a user — the [Reason] is
 * what the UI switches on, exactly as it does for `CopyPasteException`
 * (manifest 06 INV-12).
 */
class DeviceSecretUnavailable(
    val reason: DeviceSecret.Reason,
    cause: Throwable? = null,
) : Exception(null, cause)
