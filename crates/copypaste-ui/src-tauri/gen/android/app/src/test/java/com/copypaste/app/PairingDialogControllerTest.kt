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
        assertNull(rendered)

        dialog.findViewById<View>(R.id.pairing_reveal)!!.performClick()
        assertEquals(payload, rendered)
        assertNoViewValue(dialog.window!!.decorView, payload)
        assertEquals("Pairing QR code", dialog.findViewById<View>(R.id.pairing_qr)!!.contentDescription)
    }

    @Test
    fun sasIsInertAccessibleAndEveryDecisionHasATouchTarget() {
        val decisions = mutableListOf<String>()
        val dialogs = PairingDialogController(activity)
        assertTrue(dialogs.confirm("123456", "Unverified Phone", "responder", decisions::add))
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
    fun mismatchCancelAndTimeoutAreExplicitAndMutuallyExclusive() {
        val dialogs = PairingDialogController(activity, confirmTimeoutMs = 100)
        val decisions = mutableListOf<String>()

        dialogs.confirm("123456", null, "initiator", decisions::add)
        latestDialog().getButton(AlertDialog.BUTTON_NEGATIVE).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(listOf("reject"), decisions)

        dialogs.confirm("123456", null, "initiator", decisions::add)
        latestDialog().getButton(AlertDialog.BUTTON_NEUTRAL).performClick()
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(listOf("reject", "cancel"), decisions)

        dialogs.confirm("123456", null, "initiator", decisions::add)
        val confirmation = latestDialog()
        shadowOf(Looper.getMainLooper()).idleFor(100, TimeUnit.MILLISECONDS)
        assertEquals(listOf("reject", "cancel", "cancel"), decisions)
        assertFalse(confirmation.isShowing)
        val timedOut = latestDialog()
        assertTrue(allText(timedOut.window!!.decorView).contains("Pairing timed out."))
        assertNull(timedOut.findViewById<View>(R.id.pairing_sas))
        assertTrue(timedOut.getButton(AlertDialog.BUTTON_POSITIVE)?.visibility != View.VISIBLE)
    }

    @Test
    fun lifecycleCleanupCancelsConfirmationAndErasesQr() {
        var qr: Bitmap? = null
        val renderer = PairingQrRenderer { _, _ ->
            Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888).also { qr = it }
        }
        val dialogs = PairingDialogController(activity, renderer)
        dialogs.presentInvite("payload", "code", 120)
        latestDialog().findViewById<View>(R.id.pairing_reveal)!!.performClick()

        val decisions = mutableListOf<String>()
        dialogs.confirm("654321", null, null, decisions::add)
        val confirmation = latestDialog()
        dialogs.destroy()

        assertEquals(listOf("cancel"), decisions)
        assertFalse(confirmation.isShowing)
        assertTrue(qr!!.isRecycled)
        assertFalse(dialogs.presentProgress("handshaking"))
    }

    @Test
    fun progressCopyIsFixedAndDoesNotEchoUntrustedInput() {
        val dialogs = PairingDialogController(activity)
        val untrusted = "failed at /data/user/0/name with 192.0.2.1"

        assertTrue(dialogs.presentProgress(untrusted))
        val dialog = latestDialog()
        assertNoViewValue(dialog.window!!.decorView, untrusted)
        assertTrue(allText(dialog.window!!.decorView).contains("Pairing is ready."))
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
