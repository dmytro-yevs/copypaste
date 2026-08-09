package com.copypaste.app

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AlertDialog
import com.google.android.material.button.MaterialButton
import com.google.android.material.dialog.MaterialAlertDialogBuilder

internal class PairingDialogController(
    private val activity: Activity,
    private val qrRenderer: PairingQrRenderer = ZxingPairingQrRenderer(),
    private val confirmTimeoutMs: Long = 60_000L,
) {
    private val handler = Handler(Looper.getMainLooper())
    private var activeDialog: AlertDialog? = null
    private var activeQr: Bitmap? = null
    private var pendingDecision: ((String) -> Unit)? = null
    private var timeout: Runnable? = null
    private var destroyed = false

    fun presentInvite(payload: String, code: String, expiresInSecs: Long): Boolean {
        if (destroyed || payload.isEmpty() || code.isEmpty()) return false
        dismissActive()
        val root = column()
        val qr = ImageView(activity).apply {
            id = R.id.pairing_qr
            contentDescription = activity.getString(R.string.pairing_qr_label)
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
            visibility = View.GONE
            adjustViewBounds = true
        }
        val codeView = TextView(activity).apply {
            id = R.id.pairing_code
            contentDescription = activity.getString(R.string.pairing_code_label)
            text = activity.getString(R.string.pairing_code_value, code)
            setTextIsSelectable(false)
            isLongClickable = false
            visibility = View.GONE
            gravity = Gravity.CENTER
        }
        val reveal = MaterialButton(activity).apply {
            id = R.id.pairing_reveal
            text = activity.getString(R.string.pairing_reveal)
            contentDescription = activity.getString(R.string.pairing_reveal_label)
            minimumHeight = dp(MIN_TOUCH_DP)
            setOnClickListener {
                val bitmap = runCatching {
                    qrRenderer.render(payload, dp(QR_SIZE_DP))
                }.getOrNull() ?: return@setOnClickListener
                activeQr = bitmap
                qr.setImageBitmap(bitmap)
                qr.visibility = View.VISIBLE
                codeView.visibility = View.VISIBLE
                visibility = View.GONE
            }
        }
        root.addView(reveal, matchWidth())
        root.addView(qr, centered(dp(QR_SIZE_DP)))
        root.addView(codeView, matchWidth())
        root.addView(text(activity.getString(R.string.pairing_expires, expiresInSecs)))
        show(
            MaterialAlertDialogBuilder(activity)
                .setTitle(R.string.pairing_invite_title)
                .setView(root)
                .setNegativeButton(R.string.pairing_cancel, null)
                .create(),
        )
        return true
    }

    fun presentProgress(state: String): Boolean {
        if (destroyed) return false
        dismissActive()
        val message = when (state) {
            "waiting_for_peer" -> R.string.pairing_waiting
            "handshaking" -> R.string.pairing_handshaking
            "awaiting_confirmation" -> R.string.pairing_awaiting_confirmation
            "confirmed" -> R.string.pairing_confirmed
            "rejected" -> R.string.pairing_rejected
            "cancelled" -> R.string.pairing_cancelled
            "timed_out" -> R.string.pairing_timed_out
            "failed" -> R.string.pairing_failed
            else -> R.string.pairing_idle
        }
        show(
            MaterialAlertDialogBuilder(activity)
                .setTitle(R.string.pairing_progress_title)
                .setMessage(message)
                .setNegativeButton(R.string.pairing_cancel, null)
                .create(),
        )
        return true
    }

    fun confirm(sas: String, peerName: String?, role: String?, decision: (String) -> Unit): Boolean {
        if (destroyed || sas.length != 6 || sas.any { it !in '0'..'9' }) return false
        dismissActive()
        pendingDecision = decision
        val root = column()
        root.addView(text(activity.getString(R.string.pairing_confirm_instruction)))
        root.addView(sasView(sas), matchWidth())
        root.addView(text(activity.getString(R.string.pairing_unverified_label)).apply {
            contentDescription = activity.getString(R.string.pairing_unverified_label)
        })
        if (!peerName.isNullOrBlank()) root.addView(text(peerName))
        val title = if (role == "responder") R.string.pairing_confirm_responder else R.string.pairing_confirm_initiator
        val dialog = MaterialAlertDialogBuilder(activity)
            .setTitle(title)
            .setView(root)
            .setPositiveButton(R.string.pairing_match) { _, _ -> deliver("accept") }
            .setNegativeButton(R.string.pairing_mismatch) { _, _ -> deliver("reject") }
            .setNeutralButton(R.string.pairing_cancel) { _, _ -> deliver("cancel") }
            .create()
        dialog.setOnCancelListener { deliver("cancel") }
        show(dialog)
        dialog.getButton(AlertDialog.BUTTON_POSITIVE).contentDescription =
            activity.getString(R.string.pairing_match_label)
        for (which in listOf(AlertDialog.BUTTON_POSITIVE, AlertDialog.BUTTON_NEGATIVE, AlertDialog.BUTTON_NEUTRAL)) {
            dialog.getButton(which).apply {
                minimumHeight = dp(MIN_TOUCH_DP)
                minimumWidth = dp(MIN_TOUCH_DP)
            }
        }
        timeout = Runnable {
            deliver("cancel")
            dialog.dismiss()
            presentProgress("timed_out")
        }.also { handler.postDelayed(it, confirmTimeoutMs) }
        return true
    }

    fun destroy() {
        if (destroyed) return
        destroyed = true
        deliver("cancel")
        dismissActive()
    }

    private fun show(dialog: AlertDialog) {
        activeDialog = dialog
        dialog.setOnDismissListener {
            if (activeDialog === dialog) {
                activeDialog = null
                deliver("cancel")
                clearQr()
            }
        }
        dialog.show()
        dialog.window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        dialog.getButton(AlertDialog.BUTTON_NEGATIVE)?.apply {
            minimumHeight = dp(MIN_TOUCH_DP)
            minimumWidth = dp(MIN_TOUCH_DP)
        }
    }

    private fun dismissActive() {
        timeout?.let(handler::removeCallbacks)
        timeout = null
        activeDialog?.dismiss()
        activeDialog = null
        deliver("cancel")
        clearQr()
    }

    private fun deliver(value: String) {
        val callback = pendingDecision ?: return
        pendingDecision = null
        timeout?.let(handler::removeCallbacks)
        timeout = null
        callback(value)
    }

    private fun clearQr() {
        activeQr?.eraseColor(Color.WHITE)
        activeQr?.recycle()
        activeQr = null
    }

    private fun sasView(sas: String): LinearLayout = LinearLayout(activity).apply {
        id = R.id.pairing_sas
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER
        contentDescription = activity.getString(R.string.pairing_sas_label, sas)
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
        for (digit in sas) {
            addView(TextView(activity).apply {
                text = digit.toString()
                textSize = 28f
                gravity = Gravity.CENTER
                setTextIsSelectable(false)
                isLongClickable = false
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, LinearLayout.LayoutParams(dp(40), dp(MIN_TOUCH_DP)))
        }
    }

    private fun column() = LinearLayout(activity).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        setPadding(dp(24), dp(8), dp(24), 0)
    }

    private fun text(value: String) = TextView(activity).apply {
        text = value
        setPadding(0, dp(8), 0, dp(8))
    }

    private fun matchWidth() = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    )

    private fun centered(size: Int) = LinearLayout.LayoutParams(size, size).apply {
        gravity = Gravity.CENTER_HORIZONTAL
    }

    private fun dp(value: Int): Int = (value * activity.resources.displayMetrics.density).toInt()

    private companion object {
        const val MIN_TOUCH_DP = 48
        const val QR_SIZE_DP = 240
    }
}
