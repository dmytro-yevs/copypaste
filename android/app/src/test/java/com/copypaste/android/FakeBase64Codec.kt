package com.copypaste.android

import java.util.Base64

/**
 * Real (JVM-side) [Base64Codec] for JUnit4 tests.
 *
 * CopyPaste-vp63.36: `android.util.Base64` is unusable in this module's plain
 * JUnit4 (no Robolectric) unit tests — see [Base64Codec] doc — so tests inject
 * this [java.util.Base64]-backed implementation instead. Flags are ignored
 * (this fake always encodes without line-wrapping); no test in this module
 * depends on the MIME-style line-wrap behavior of `Base64.DEFAULT`.
 */
object FakeBase64Codec : Base64Codec {
    override fun encode(data: ByteArray, flags: Int): String =
        Base64.getEncoder().encodeToString(data)

    override fun decode(data: String, flags: Int): ByteArray =
        Base64.getDecoder().decode(data)
}
