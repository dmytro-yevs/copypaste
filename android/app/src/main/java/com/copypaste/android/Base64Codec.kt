package com.copypaste.android

/**
 * Seam over [android.util.Base64] (CopyPaste-vp63.36).
 *
 * `android.util.Base64` is an Android-framework class: under the plain JUnit4
 * (no Robolectric) unit-test setup used by this module
 * (`testOptions.unitTests.isReturnDefaultValues = true` in `app/build.gradle`),
 * every unmocked Android method returns a default value instead of running
 * real logic — for `Base64.encodeToString`/`Base64.decode` that means silently
 * returning `null` rather than encoding/decoding anything. That would make
 * every KEK-wrap/roster/identity round-trip test in
 * [KeystoreSecretStore]/[PeerRosterStore]/[P2pIdentityStore] pass its input
 * through as silently-dropped nulls instead of exercising the real
 * serialization logic.
 *
 * [AndroidBase64Codec] is the production default (delegates verbatim to
 * `android.util.Base64` with the SAME flags used previously — no behavior
 * change). Tests inject a JVM-only fake instead.
 */
interface Base64Codec {
    fun encode(data: ByteArray, flags: Int): String
    fun decode(data: String, flags: Int): ByteArray
}

/** Production [Base64Codec] — delegates verbatim to [android.util.Base64]. */
object AndroidBase64Codec : Base64Codec {
    override fun encode(data: ByteArray, flags: Int): String =
        android.util.Base64.encodeToString(data, flags)

    override fun decode(data: String, flags: Int): ByteArray =
        android.util.Base64.decode(data, flags)
}
