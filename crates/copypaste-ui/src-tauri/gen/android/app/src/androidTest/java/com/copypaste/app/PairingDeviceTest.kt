package com.copypaste.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Until
import java.io.File
import java.util.regex.Pattern
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Pairing values are inaccessible because labelled buttons replace their text
 * in the accessibility tree. Copying the ceremony URI is the product-supported
 * way to expose them to the shell-driven joiner.
 *
 * Only the joiner receives a dialable address. It drives every sync while the
 * creator stays alive until both directions have crossed.
 */
@LargeTest
@RunWith(AndroidJUnit4::class)
class PairingDeviceTest {
    private val app = DeviceHarness()
    private val args = InstrumentationRegistry.getArguments()

    @Test fun pairedDevicesExchangeClipsInBothDirections() {
        val role = requireArg("peerRole")
        val ownPre = requireArg("ownCanary")
        val remotePre = requireArg("remoteCanary")
        val ownLive = "$ownPre-live"
        val remoteLive = "$remotePre-live"

        app.launch()
        // Captured before the pairing exists, so arriving on the far side
        // proves a backfill rather than a live hand-off.
        app.capture(ownPre)

        when (role) {
            "creator" -> create()
            "joiner" -> join()
            else -> throw AssertionError("unknown peerRole: $role")
        }
        if (role == "joiner") {
            assertTrue("the creator never finished its live capture", app.waitUntil(SYNC_MS) {
                app.shell("run-as ${app.context.packageName} ls $CREATOR_LIVE_READY")
                    .endsWith(CREATOR_LIVE_READY)
            })
        }
        // Ordering the live captures makes the creator's version the older one
        // and exercises the cursor regression deterministically.
        app.capture(ownLive)
        File(app.context.filesDir, LIVE_NAME).writeText("ready")
        assertTrue("the peer never finished its live capture", app.waitUntil(SYNC_MS) {
            app.shell("run-as ${app.context.packageName} ls $SYNC_READY").endsWith(SYNC_READY)
        })

        awaitRemote(remotePre, role)
        if (role == "joiner") {
            awaitRemote(remoteLive, role)
            val cursorFile = File(app.context.dataDir, "sync-cursors.json")
            val cursorWrittenAt = cursorFile.lastModified()
            app.tap("Devices")
            app.tapEnabledExact("Sync now")
            assertTrue("the follow-up incremental sync did not finish", app.waitUntil(FORM_MS) {
                cursorFile.lastModified() > cursorWrittenAt
            })
            assertCanaries(ownPre, ownLive, remotePre, remoteLive)
            File(app.context.filesDir, JOINER_DONE_NAME).writeText("done")
            return
        }

        assertTrue("the joiner never finished its assertions", app.waitUntil(SYNC_MS) {
            app.shell("run-as ${app.context.packageName} ls $JOINER_DONE").endsWith(JOINER_DONE)
        })
        val refresh = "$ownPre-refresh"
        app.capture(refresh)
        awaitRemote(remoteLive, role)
        assertCanaries(ownPre, ownLive, remotePre, remoteLive)
        app.assertNoCanaryLeak(refresh)
    }

    private fun create() {
        app.tap("Devices")
        app.tap("Show code")
        assertTrue("the pairing ceremony never minted a code", app.awaitText("Pairing code", MINT_MS))
        assertTrue("the ceremony exposed no address to dial", app.awaitText("Connection address", MINT_MS))

        app.tap("Copy pairing details")
        val uri = app.waitFor(CLIP_MS) { app.clipboardText().takeIf { it.startsWith("copypaste://pair") } }
        assertNotNull("Copy pairing details did not put a pairing URI on the clipboard", uri)
        handoff().writeText(uri!!)

        // The sheet stays up until the script has taken the details, because
        // closing it is what the product treats as abandoning the ceremony.
        assertTrue("the harness never collected the pairing details", app.waitUntil(HANDOFF_MS) {
            app.shell("run-as ${app.context.packageName} ls $CLAIMED").endsWith(CLAIMED)
        })
        assertTrue("the joiner never confirmed pairing", app.waitUntil(PAIR_MS) {
            app.shell("run-as ${app.context.packageName} ls $PEER_READY").endsWith(PEER_READY)
        })
        app.tap("Done")
        assertTrue("no peer ever redeemed the pairing code", app.awaitText("device paired", PAIR_MS))
    }

    private fun join() {
        app.tap("Devices")
        app.tap("Join device")
        assertTrue("the join form did not expose three fields", app.awaitText("Pairing code", FORM_MS))
        val expected = listOf(requireArg("pairCode"), requireArg("pairAddr"), requireArg("securityCode"))
        app.setField("pairing-code", expected[0])
        app.setField("pairing-address", expected[1])
        app.setField("pairing-security-code", expected[2])
        app.reveal("Security code")
        val populated = app.onScreen()
        expected.forEach { value ->
            assertTrue("the join form did not retain '$value'. $populated", populated.contains(value))
        }
        app.reveal("I compared the security code on both devices.")
        val acknowledgementSelector = By.res(Pattern.compile(".*pairing-security-ack"))
        val acknowledgement = app.device.wait(Until.findObject(acknowledgementSelector), FORM_MS)
        assertNotNull("the security acknowledgement was missing", acknowledgement)
        acknowledgement!!.click()
        assertNotNull(
            "the security acknowledgement was not checked",
            app.device.wait(
                Until.findObject(By.res(Pattern.compile(".*pairing-security-ack")).checked(true)),
                FORM_MS,
            ),
        )
        app.tapEnabledExact("Verify and pair")
        assertTrue("pairing never completed. ${app.onScreen()}", app.awaitText("device paired", PAIR_MS))
        File(app.context.filesDir, PAIRED_NAME).writeText("paired")
    }

    /** The joiner is the only side that can start a session, so it is the only
     *  side that presses anything here. */
    private fun awaitRemote(canary: String, role: String) {
        val deadline = System.currentTimeMillis() + SYNC_MS
        while (System.currentTimeMillis() < deadline) {
            app.openHistory()
            if (app.awaitText(canary, POLL_MS)) return
            if (role == "joiner") {
                app.tap("Devices")
                app.tapEnabledExact("Sync now")
                app.awaitText("Synced", POLL_MS)
            }
        }
        app.openHistory()
        assertTrue("peer clip never arrived: $canary", app.awaitText(canary, POLL_MS))
    }

    private fun assertCanaries(ownPre: String, ownLive: String, remotePre: String, remoteLive: String) {
        app.openHistory()
        app.assertVisible(ownPre)
        app.assertVisible(ownLive)
        app.assertNoCanaryLeak(ownPre)
        app.assertNoCanaryLeak(ownLive)
        app.assertNoCanaryLeak(remotePre)
        app.assertNoCanaryLeak(remoteLive)
    }

    private fun handoff(): File =
        File(app.context.filesDir, HANDOFF_NAME)

    private fun requireArg(name: String): String =
        args.getString(name)?.takeIf { it.isNotBlank() }
            ?: throw AssertionError("missing instrumentation argument: $name")

    companion object {
        const val HANDOFF_NAME = "e2e-pairing.txt"
        /** Touched by the driving script once it has read the handoff file. */
        const val CLAIMED = "files/e2e-claimed"
        const val PEER_READY = "files/e2e-peer-ready"
        const val CREATOR_LIVE_READY = "files/e2e-creator-live-ready"
        const val PAIRED_NAME = "e2e-paired"
        const val LIVE_NAME = "e2e-live"
        const val SYNC_READY = "files/e2e-sync-ready"
        const val JOINER_DONE_NAME = "e2e-joiner-done"
        const val JOINER_DONE = "files/e2e-joiner-done"
        private const val MINT_MS = 60_000L
        private const val CLIP_MS = 20_000L
        private const val HANDOFF_MS = 180_000L
        private const val PAIR_MS = 240_000L
        private const val FORM_MS = 30_000L
        private const val SYNC_MS = 300_000L
        private const val POLL_MS = 8_000L
    }
}
