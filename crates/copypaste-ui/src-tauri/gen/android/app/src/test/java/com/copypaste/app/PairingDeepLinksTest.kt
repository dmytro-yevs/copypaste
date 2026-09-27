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
    fun parseForwardsTheCanonicalPairingUriWithoutRebuildingCredentials() {
        val uri = "copypaste://pair?v=1&code=0123-4567-89AB-CDEF&listen_addr=192.0.2.1%3A47654"
        val payload = PairingDeepLinks.parse(Uri.parse(uri))
        assertEquals(uri, payload)
    }

    @Test
    fun httpsAndEmptyFieldsAreRejected() {
        assertNull(PairingDeepLinks.parse(Uri.parse("https://example.com/pair?code=a&listen_addr=b")))
        assertNull(PairingDeepLinks.parse(Uri.parse("copypaste://not-pair?code=a&listen_addr=b")))
    }

    @Test
    fun takeClearsThePendingLinkOnce() {
        val intent = android.content.Intent(android.content.Intent.ACTION_VIEW)
        intent.data = Uri.parse("copypaste://pair?v=1&code=secret-code&listen_addr=192.0.2.8%3A47654")
        PairingDeepLinks.offer(intent)
        val first = PairingDeepLinks.take()
        assertEquals(PairingDeepLinks.parse(intent.data), first)
        assertNull(PairingDeepLinks.take())
    }
}
