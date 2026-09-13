package com.copypaste.app

import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class PairingDeepLinksTest {
    @Test
    fun encodeAndParseRoundTripThePairingFields() {
        val uri = PairingDeepLinks.encode("0123-4567-89AB-CDEF", "192.0.2.1:47654")
        assertTrue(uri!!.startsWith("copypaste://pair"))
        val payload = PairingDeepLinks.parse(Uri.parse(uri))
        assertTrue(payload!!.contains("\"code\":\"0123-4567-89AB-CDEF\""))
        assertTrue(payload.contains("\"listen_addr\":\"192.0.2.1:47654\""))
    }

    @Test
    fun httpsAndEmptyFieldsAreRejected() {
        assertNull(PairingDeepLinks.parse(Uri.parse("https://example.com/pair?code=a&listen_addr=b")))
        assertNull(PairingDeepLinks.parse(Uri.parse("copypaste://pair?code=&listen_addr=host:1")))
        assertNull(PairingDeepLinks.encode("", "host:1"))
    }

    @Test
    fun takeClearsThePendingLinkOnce() {
        val intent = android.content.Intent(android.content.Intent.ACTION_VIEW)
        intent.data = Uri.parse(PairingDeepLinks.encode("secret-code", "192.0.2.8:47654"))
        PairingDeepLinks.offer(intent)
        val first = PairingDeepLinks.take()
        assertEquals(PairingDeepLinks.parse(intent.data), first)
        assertNull(PairingDeepLinks.take())
    }
}
