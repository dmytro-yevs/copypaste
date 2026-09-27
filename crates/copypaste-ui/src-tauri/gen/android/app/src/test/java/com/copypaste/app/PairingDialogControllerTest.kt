package com.copypaste.app

import android.graphics.Bitmap
import android.os.Bundle
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.WebView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.android.controller.ActivityController
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowDialog
import java.util.concurrent.TimeUnit

class PairingTestActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        setTheme(R.style.Theme_copypaste_ui)
        super.onCreate(savedInstanceState)
    }
}

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class PairingDialogControllerTest {
    private lateinit var activityController: ActivityController<PairingTestActivity>
    private lateinit var activity: PairingTestActivity

    @Before
    fun setUp() {
        activityController = Robolectric.buildActivity(PairingTestActivity::class.java).setup()
        activity = activityController.get()
    }

    @After
    fun tearDown() {
        activityController.close()
    }

    @Test
    fun inv13PayloadNeverBecomesViewOrAccessibilityText() {
        val payload = "{\"version\":1,\"code\":\"SECRET-CODE\",\"listen_addr\":\"192.0.2.1:47654\"}"
        var rendered: String? = null
        val renderer = PairingQrRenderer { value, _ ->
            rendered = value
            Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)
        }
        val dialogs = PairingDialogController(activity, renderer)

        assertTrue(dialogs.presentInvite(payload, "SECRET-CODE", 120))
        val dialog = latestDialog()
        assertSecure(dialog)
        assertNoViewValue(dialog.window!!.decorView, payload)
        assertTrue(allViews(dialog.window!!.decorView).none { it is WebView })
        assertEquals(payload, rendered)
        assertNoViewValue(dialog.window!!.decorView, payload)
        assertNoViewValue(dialog.window!!.decorView, "SECRET-CODE")
        assertTrue(allText(dialog.window!!.decorView).none { it.contains("reveal", ignoreCase = true) })
        assertNull(dialog.findViewById(R.id.pairing_code))
        assertEquals("Pairing QR code", dialog.findViewById<View>(R.id.pairing_qr)!!.contentDescription)
        assertEquals(View.VISIBLE, dialog.findViewById<View>(R.id.pairing_qr)!!.visibility)
    }

    @Test
    fun renderFailureDoesNotOpenAPanel() {
        val dialogs = PairingDialogController(activity, PairingQrRenderer { _, _ ->
            throw IllegalStateException("render failure")
        })

        assertFalse(dialogs.presentInvite("payload", "CODE", 120))
        assertNull(ShadowDialog.getLatestDialog()?.takeIf { it.isShowing })
    }

    @Test
    fun awaitingConfirmationProgressDoesNotOpenAbortingDialog() {
        val dialogs = PairingDialogController(activity)
        var aborted = 0

        assertTrue(
            dialogs.presentProgress(
                "securing_connection",
                "Securing the connection",
                "Keep both devices nearby while CopyPaste establishes a secure connection.",
                active = true,
            ) { aborted += 1 },
        )
        val progress = latestDialog()
        assertTrue(progress.isShowing)

        assertTrue(
            dialogs.presentProgress(
                "compare_codes",
                "Compare security codes",
                "Confirm the code in the native security prompt.",
                active = true,
            ) { aborted += 1 },
        )
        shadowOf(Looper.getMainLooper()).idle()
        assertFalse(progress.isShowing)
        assertEquals(0, aborted)
        assertNull(ShadowDialog.getLatestDialog()?.takeIf { it.isShowing })
    }

    @Test
    fun scannerFailureExplainsHowToRecoverWithoutExposingCredentials() {
        val dialogs = PairingDialogController(activity)

        dialogs.presentScanFailure()

        val dialog = latestDialog()
        assertSecure(dialog)
        assertEquals(
            "Can’t open QR scanner",
            dialog.findViewById<TextView>(androidx.appcompat.R.id.alertTitle)?.text?.toString(),
        )
        assertTrue(
            dialog.findViewById<TextView>(android.R.id.message)
                ?.text
                ?.contains("Google Play services") == true,
        )
        assertNoViewValue(dialog.window!!.decorView, "SECRET-CODE")
    }

    @Test
    fun inviteAndProgressCancelAbortTheCeremony() {
        val dialogs = PairingDialogController(activity)
        var aborted = 0

        assertTrue(dialogs.presentInvite("payload", "CODE", 120) { aborted += 1 })
        latestDialog().getButton(AlertDialog.BUTTON_NEGATIVE).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(1, aborted)

        assertTrue(
            dialogs.presentProgress(
                "securing_connection",
                "Securing the connection",
                "Keep both devices nearby while CopyPaste establishes a secure connection.",
                active = true,
            ) { aborted += 1 },
        )
        latestDialog().getButton(AlertDialog.BUTTON_NEGATIVE).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(2, aborted)
    }

    @Test
    fun waitingRetainsQrAndHandshakingUpdatesTheSameProtectedPanel() {
        var aborted = 0
        val renderer = PairingQrRenderer { _, _ ->
            Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)
        }
        val dialogs = PairingDialogController(activity, renderer)
        assertTrue(dialogs.presentInvite("payload", "CODE", 120) { aborted += 1 })
        val invite = latestDialog()
        assertEquals(View.VISIBLE, invite.findViewById<View>(R.id.pairing_qr)!!.visibility)

        assertTrue(
            dialogs.presentProgress(
                "waiting_for_peer",
                "Waiting for a device",
                "Waiting for the other device to join.",
                active = true,
            ) { aborted += 1 },
        )
        assertTrue(invite.isShowing)
        assertEquals(View.VISIBLE, invite.findViewById<View>(R.id.pairing_qr)!!.visibility)
        assertEquals(0, aborted)

        assertTrue(
            dialogs.presentProgress(
                "securing_connection",
                "Securing the connection",
                "Keep both devices nearby while CopyPaste establishes a secure connection.",
                active = true,
            ) { aborted += 1 },
        )
        assertTrue(invite.isShowing)
        assertTrue(latestDialog() === invite)
        assertEquals(0, aborted)
        assertNull(invite.findViewById(R.id.pairing_qr))
        assertTrue(
            allText(invite.window!!.decorView)
                .contains("Keep both devices nearby while CopyPaste establishes a secure connection."),
        )
    }

    @Test
    fun sasIsInertAccessibleAndEveryDecisionHasATouchTarget() {
        val decisions = mutableListOf<String>()
        val dialogs = PairingDialogController(activity)
        assertTrue(
            dialogs.confirm("123456", "Unverified Phone", "responder", 60_000, decisions::add),
        )
        val dialog = latestDialog()
        val sas = dialog.findViewById<LinearLayout>(R.id.pairing_sas)!!
        assertEquals("Security code: 123456", sas.contentDescription)
        assertEquals(6, sas.childCount)
        for (index in 0 until sas.childCount) {
            val digit = sas.getChildAt(index) as TextView
            assertFalse(digit.isTextSelectable)
            assertFalse(digit.isLongClickable)
            assertEquals(View.IMPORTANT_FOR_ACCESSIBILITY_NO, digit.importantForAccessibility)
        }
        val minimum = (48 * activity.resources.displayMetrics.density).toInt()
        for (which in listOf(AlertDialog.BUTTON_POSITIVE, AlertDialog.BUTTON_NEGATIVE, AlertDialog.BUTTON_NEUTRAL)) {
            assertTrue(dialog.getButton(which).minimumHeight >= minimum)
            assertTrue(dialog.getButton(which).minimumWidth >= minimum)
        }
        assertEquals("Codes match — confirm pairing", dialog.getButton(AlertDialog.BUTTON_POSITIVE).contentDescription)
        assertSecure(dialog)

        dialog.getButton(AlertDialog.BUTTON_POSITIVE).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(listOf("accept"), decisions)
    }

    @Test
    fun mismatchCancelAndDeadlineRefreshAreExplicitAndMutuallyExclusive() {
        val dialogs = PairingDialogController(activity)
        val decisions = mutableListOf<String>()

        dialogs.confirm("123456", null, "initiator", 60_000, decisions::add)
        latestDialog().getButton(AlertDialog.BUTTON_NEGATIVE).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(listOf("reject"), decisions)

        dialogs.confirm("123456", null, "initiator", 60_000, decisions::add)
        latestDialog().getButton(AlertDialog.BUTTON_NEUTRAL).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(listOf("reject", "cancel"), decisions)

        dialogs.confirm("123456", null, "initiator", 100, decisions::add)
        val confirmation = latestDialog()
        shadowOf(Looper.getMainLooper()).idleFor(100, TimeUnit.MILLISECONDS)
        assertEquals(listOf("reject", "cancel", "refresh"), decisions)
        assertFalse(confirmation.isShowing)
        assertNull(confirmation.findViewById<LinearLayout>(R.id.pairing_sas)?.contentDescription)
        assertEquals(0, confirmation.findViewById<LinearLayout>(R.id.pairing_sas)?.childCount)
        assertTrue(confirmation.getButton(AlertDialog.BUTTON_POSITIVE).visibility != View.VISIBLE)
        assertTrue(confirmation.getButton(AlertDialog.BUTTON_NEGATIVE).visibility != View.VISIBLE)

        dialogs.presentProgress(
            "timed_out",
            "Pairing timed out",
            "Check that both devices are on the same network and try again.",
            active = false,
        )
        val timedOut = latestDialog()
        assertTrue(allText(timedOut.window!!.decorView).contains("Pairing timed out"))
        assertNull(timedOut.findViewById<View>(R.id.pairing_sas))
        assertTrue(timedOut.getButton(AlertDialog.BUTTON_POSITIVE)?.visibility != View.VISIBLE)
    }

    @Test
    fun inviteDeadlineErasesQrAndRequestsAuthoritativeRefresh() {
        var qr: Bitmap? = null
        var aborted = 0
        var refreshed = 0
        val renderer = PairingQrRenderer { _, _ ->
            val source = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)
            requireNotNull(source.copy(Bitmap.Config.ARGB_8888, false)).also {
                source.recycle()
                qr = it
            }
        }
        val dialogs = PairingDialogController(activity, renderer)
        dialogs.presentInvite(
            "payload",
            "code",
            1,
            onRefresh = { refreshed += 1 },
            onAbort = { aborted += 1 },
        )
        val invite = latestDialog()
        shadowOf(Looper.getMainLooper()).idleFor(1, TimeUnit.SECONDS)

        assertEquals(0, aborted)
        assertEquals(1, refreshed)
        assertTrue(invite.isShowing)
        assertTrue(qr!!.isRecycled)
        assertTrue(invite.findViewById<View>(R.id.pairing_qr)!!.visibility != View.VISIBLE)
        assertTrue(allText(invite.window!!.decorView).contains("Checking pairing status…"))
    }

    @Test
    fun immutableQrDismissesWithoutAnEraseColorCrash() {
        var qr: Bitmap? = null
        val renderer = PairingQrRenderer { _, _ ->
            val source = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)
            requireNotNull(source.copy(Bitmap.Config.ARGB_8888, false)).also {
                source.recycle()
                qr = it
            }
        }
        val dialogs = PairingDialogController(activity, renderer)

        assertTrue(dialogs.presentInvite("payload", "code", 120))
        latestDialog().dismiss()
        dialogs.destroy()
        shadowOf(Looper.getMainLooper()).idle()

        assertTrue(qr!!.isRecycled)
    }

    @Test
    fun lifecycleCleanupCancelsConfirmationAndErasesQr() {
        var qr: Bitmap? = null
        val renderer = PairingQrRenderer { _, _ ->
            Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888).also { qr = it }
        }
        val dialogs = PairingDialogController(activity, renderer)
        dialogs.presentInvite("payload", "code", 120)

        val decisions = mutableListOf<String>()
        dialogs.confirm("654321", null, null, 60_000, decisions::add)
        val confirmation = latestDialog()
        dialogs.destroy()

        assertEquals(listOf("cancel"), decisions)
        assertFalse(confirmation.isShowing)
        assertTrue(qr!!.isRecycled)
        assertFalse(
            dialogs.presentProgress(
                "securing_connection",
                "Securing the connection",
                "Keep both devices nearby while CopyPaste establishes a secure connection.",
                active = true,
            ) {},
        )
    }

    @Test
    fun terminalProgressRendersCanonicalCopyAndAnExplicitCloseAction() {
        val dialogs = PairingDialogController(activity)
        val untrusted = "failed at /data/user/0/name with 192.0.2.1"
        var aborted = 0

        assertTrue(
            dialogs.presentProgress(
                "failed",
                "Pairing failed",
                "Pairing failed. No device was paired.",
                active = false,
            ) { aborted += 1 },
        )
        val dialog = latestDialog()
        assertNoViewValue(dialog.window!!.decorView, untrusted)
        assertTrue(allText(dialog.window!!.decorView).contains("Pairing failed. No device was paired."))
        assertEquals("Close", dialog.getButton(AlertDialog.BUTTON_NEGATIVE).text.toString())
        assertEquals(View.VISIBLE, dialog.getButton(AlertDialog.BUTTON_NEGATIVE).visibility)
        dialog.getButton(AlertDialog.BUTTON_NEGATIVE).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(0, aborted)
    }

    @Test
    fun progressPayloadValidationRejectsUnknownOrMalformedValues() {
        val known = PairingProgressSemantics().apply { messageId = "unreachable" }
        val unknown = PairingProgressSemantics().apply { messageId = "future_daemon_state" }
        val safe = PairingProgressCopy().apply {
            title = "Pairing couldn't finish"
            detail = "Could not reach the other device. Check that both devices are on the same network."
        }

        assertTrue(known.isKnown())
        assertFalse(unknown.isKnown())
        assertTrue(safe.isSafe())
        assertFalse(PairingProgressCopy().isSafe())
    }

    private fun latestDialog(): AlertDialog = ShadowDialog.getLatestDialog() as AlertDialog

    private fun assertSecure(dialog: AlertDialog) {
        val flags = dialog.window!!.attributes.flags
        assertTrue(flags and WindowManager.LayoutParams.FLAG_SECURE != 0)
    }

    private fun assertNoViewValue(root: View, forbidden: String) {
        for (view in allViews(root)) {
            val values = listOf(
                (view as? TextView)?.text?.toString(),
                view.contentDescription?.toString(),
                view.tag?.toString(),
            )
            assertTrue("found secret in $values", values.none { it?.contains(forbidden) == true })
        }
    }

    private fun allText(root: View): List<String> = allViews(root).mapNotNull {
        (it as? TextView)?.text?.toString()
    }

    private fun allViews(root: View): List<View> = buildList {
        add(root)
        if (root is ViewGroup) {
            for (index in 0 until root.childCount) addAll(allViews(root.getChildAt(index)))
        }
    }
}
