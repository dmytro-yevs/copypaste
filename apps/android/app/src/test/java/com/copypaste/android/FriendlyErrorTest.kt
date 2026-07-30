package com.copypaste.android

import com.copypaste.android.data.friendlyMessage
import com.copypaste.ffi.CopyPasteException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The error mapping, on the JVM.
 *
 * ─────────────────────────────────────────────────────────────────────────────
 * THIS TEST HAS NEVER BEEN RUN. There is no Android SDK on the machine it was
 * written on, so no Gradle task has executed. It is here because the property
 * it checks is the one most worth checking, not because it has passed.
 * ─────────────────────────────────────────────────────────────────────────────
 *
 * It needs no device: `CopyPasteException` is a plain Kotlin sealed class and
 * `friendlyMessage` returns an `Int`, so `testDebugUnitTest` covers it.
 */
class FriendlyErrorTest {

    /**
     * Every exception the FFI can raise, so a new variant in `copypaste-ffi`
     * shows up here as a compile error in the `when` rather than as an app that
     * says "Something went wrong" for a case someone wrote copy for.
     */
    private val allErrors = listOf(
        CopyPasteException.BadDeviceSecret(),
        CopyPasteException.Locked(),
        CopyPasteException.Storage(),
        CopyPasteException.Crypto(),
        CopyPasteException.ItemNotFound(),
        CopyPasteException.EmptyContent(),
        CopyPasteException.InvalidPairingCode(),
        CopyPasteException.InvalidAddress(),
        CopyPasteException.PairingRefused(),
        CopyPasteException.PeerNotFound(),
        CopyPasteException.PeerStore(),
        CopyPasteException.LegacyData(),
        CopyPasteException.PeerAddressUnknown(),
        CopyPasteException.PeerUnreachable(),
        CopyPasteException.SyncUnavailable(),
    )

    @Test
    fun `every core error maps to a real string resource`() {
        for (error in allErrors) {
            assertNotEquals("unmapped: $error", 0, friendlyMessage(error))
        }
    }

    @Test
    fun `no core error carries text of its own`() {
        // The structural half of manifest 06 INV-12: `copypaste-ffi` gives its
        // error variants no fields, so `message` is empty. Even code that
        // ignored the mapping and rendered `e.message` could not leak a path.
        for (error in allErrors) {
            assertTrue(
                "$error carries a message: ${error.message}",
                error.message.isNullOrEmpty(),
            )
        }
    }

    @Test
    fun `an unknown throwable falls back rather than rendering itself`() {
        // The message here is deliberately something that would be a disclosure
        // if it were ever shown.
        val leaky = IllegalStateException("/data/user/0/com.copypaste.android/files/x.db")
        assertEquals(R.string.error_unexpected, friendlyMessage(leaky))
    }

    @Test
    fun `distinct conditions get distinct copy`() {
        // "Locked" and "the file is from an older version" are different things
        // for a user to do, and folding them together is how a recoverable
        // situation gets treated as a broken one.
        assertNotEquals(
            friendlyMessage(CopyPasteException.Locked()),
            friendlyMessage(CopyPasteException.LegacyData()),
        )
        assertNotEquals(
            friendlyMessage(CopyPasteException.PeerAddressUnknown()),
            friendlyMessage(CopyPasteException.PeerUnreachable()),
        )
    }
}
