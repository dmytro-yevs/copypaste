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
import androidx.appcompat.app.AlertDialog
import androidx.core.view.setMargins
import com.google.android.material.button.MaterialButton
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.textview.MaterialTextView
import kotlin.math.min

internal class PairingDialogController(
    private val activity: Activity,
    private val qrRenderer: PairingQrRenderer = ZxingPairingQrRenderer(),
) {
    private val handler = Handler(Looper.getMainLooper())
    private var activeDialog: AlertDialog? = null
    private var activeQr: Bitmap? = null
    private var pendingDecision: ((String) -> Unit)? = null
    private var onAbort: (() -> Unit)? = null
    private var abortOnDismiss = true
    private var showingInvite = false
    private var timeout: Runnable? = null
    private var destroyed = false

    fun presentInvite(
        payload: String,
        code: String,
        expiresInSecs: Long,
        onRefresh: (() -> Unit)? = null,
        onAbort: (() -> Unit)? = null,
    ): Boolean {
        if (destroyed || payload.isEmpty() || code.isEmpty() || expiresInSecs <= 0) return false
        dismissActive()
        this.onAbort = onAbort
        showingInvite = true
        val root = column()
        val qrSize = min(
            dim(R.dimen.copypaste_pairing_qr_size),
            activity.resources.displayMetrics.widthPixels - dim(R.dimen.copypaste_space_6) * 2,
        )
        val qr = ImageView(activity).apply {
            id = R.id.pairing_qr
            contentDescription = activity.getString(R.string.pairing_qr_label)
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
            visibility = View.GONE
            adjustViewBounds = true
            scaleType = ImageView.ScaleType.FIT_CENTER
            setPadding(space(3), space(3), space(3), space(3))
            setBackgroundResource(R.drawable.copypaste_pairing_qr_background)
            clipToOutline = true
        }
        val reveal = MaterialButton(
            activity,
            null,
            com.google.android.material.R.attr.materialButtonOutlinedStyle,
        ).apply {
            id = R.id.pairing_reveal
            text = activity.getString(R.string.pairing_reveal)
            contentDescription = activity.getString(R.string.pairing_reveal_label)
            minimumHeight = touchTarget()
            setOnClickListener {
                val bitmap = runCatching {
                    qrRenderer.render(payload, qrSize)
                }.getOrNull() ?: return@setOnClickListener
                activeQr = bitmap
                qr.setImageBitmap(bitmap)
                qr.visibility = View.VISIBLE
                visibility = View.GONE
            }
        }
        root.addView(reveal, matchWidth())
        root.addView(qr, centered(qrSize))
        val expires = label(activity.getString(R.string.pairing_expires, expiresInSecs))
        root.addView(expires)
        val dialog = MaterialAlertDialogBuilder(activity)
            .setTitle(R.string.pairing_invite_title)
            .setView(root)
            .setNegativeButton(R.string.pairing_cancel) { dialog, _ -> dialog.dismiss() }
            .create()
        show(dialog)
        timeout = Runnable {
            if (activeDialog !== dialog) return@Runnable
            reveal.setOnClickListener(null)
            reveal.visibility = View.GONE
            qr.setImageDrawable(null)
            qr.contentDescription = null
            qr.visibility = View.GONE
            clearQr()
            expires.text = activity.getString(R.string.pairing_checking_status)
            expires.contentDescription = activity.getString(R.string.pairing_checking_status)
            this.onAbort = null
            showingInvite = false
            dialog.getButton(AlertDialog.BUTTON_NEGATIVE).text =
                activity.getString(R.string.pairing_close)
            timeout = null
            onRefresh?.invoke()
        }.also { handler.postDelayed(it, expiresInSecs * 1_000L) }
        return true
    }

    fun presentProgress(
        messageId: String,
        title: String,
        detail: String,
        active: Boolean,
        onAbort: (() -> Unit)? = null,
    ): Boolean {
        if (destroyed || title.isBlank() || detail.isBlank()) return false
        if (messageId == "waiting_for_peer" && showingInvite && activeDialog?.isShowing == true) {
            return true
        }
        // SAS confirmation is owned by confirm(). Opening a modal progress sheet
        // here would block WebView Confirm and abort on dismiss before the user
        // can compare codes — close any prior progress without aborting.
        if (messageId == "compare_codes") {
            dismissActive()
            this.onAbort = null
            showingInvite = false
            return true
        }
        dismissActive()
        this.onAbort = if (active) onAbort else null
        showingInvite = false
        val builder = MaterialAlertDialogBuilder(activity)
            .setTitle(title)
            .setMessage(detail)
        if (active) {
            builder.setNegativeButton(R.string.pairing_cancel) { dialog, _ -> dialog.dismiss() }
        }
        show(builder.create())
        return true
    }

    fun confirm(
        sas: String,
        peerName: String?,
        role: String?,
        expiresInMs: Long,
        decision: (String) -> Unit,
    ): Boolean {
        if (destroyed || sas.length != 6 || sas.any { it !in '0'..'9' }) return false
        if (expiresInMs <= 0) {
            decision("refresh")
            return true
        }
        dismissActive()
        pendingDecision = decision
        onAbort = null
        showingInvite = false
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
                minimumHeight = touchTarget()
                minimumWidth = touchTarget()
            }
        }
        timeout = Runnable {
            val sasView = dialog.findViewById<LinearLayout>(R.id.pairing_sas)
            sasView?.contentDescription = null
            sasView?.removeAllViews()
            sasView?.visibility = View.GONE
            dialog.getButton(AlertDialog.BUTTON_POSITIVE).visibility = View.GONE
            dialog.getButton(AlertDialog.BUTTON_NEGATIVE).visibility = View.GONE
            root.getChildAt(0)?.let { instruction ->
                if (instruction is MaterialTextView) {
                    instruction.text = activity.getString(R.string.pairing_checking_status)
                    instruction.contentDescription = activity.getString(R.string.pairing_checking_status)
                }
            }
            dialog.getButton(AlertDialog.BUTTON_NEUTRAL).text =
                activity.getString(R.string.pairing_close)
            deliver("refresh")
            dialog.dismiss()
        }.also { handler.postDelayed(it, expiresInMs) }
        return true
    }

    fun destroy() {
        if (destroyed) return
        destroyed = true
        fireAbort()
        deliver("cancel")
        dismissActive()
    }

    private fun show(dialog: AlertDialog) {
        activeDialog = dialog
        dialog.setOnDismissListener {
            if (activeDialog === dialog) {
                activeDialog = null
                if (abortOnDismiss) fireAbort()
                deliver("cancel")
                clearQr()
                showingInvite = false
            }
        }
        dialog.show()
        dialog.window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        dialog.getButton(AlertDialog.BUTTON_NEGATIVE)?.apply {
            minimumHeight = touchTarget()
            minimumWidth = touchTarget()
        }
    }

    private fun dismissActive() {
        abortOnDismiss = false
        timeout?.let(handler::removeCallbacks)
        timeout = null
        activeDialog?.dismiss()
        activeDialog = null
        abortOnDismiss = true
        deliver("cancel")
        clearQr()
        showingInvite = false
    }

    private fun fireAbort() {
        val callback = onAbort ?: return
        onAbort = null
        callback()
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
            addView(MaterialTextView(activity).apply {
                text = digit.toString()
                setTextAppearance(R.style.TextAppearance_CopyPaste_SecurityCode)
                gravity = Gravity.CENTER
                setBackgroundResource(R.drawable.copypaste_pairing_digit_background)
                setTextIsSelectable(false)
                isLongClickable = false
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, LinearLayout.LayoutParams(
                dim(R.dimen.copypaste_pairing_sas_digit_width),
                touchTarget(),
            ).apply { setMargins(space(1) / 2) })
        }
    }

    private fun column() = LinearLayout(activity).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        setPadding(space(6), space(2), space(6), 0)
    }

    private fun text(value: String) = MaterialTextView(activity).apply {
        text = value
        setTextAppearance(R.style.TextAppearance_CopyPaste_Body)
        setPadding(0, space(2), 0, space(2))
    }

    private fun label(value: String) = MaterialTextView(activity).apply {
        text = value
        setTextAppearance(R.style.TextAppearance_CopyPaste_Label)
        gravity = Gravity.CENTER
        setPadding(0, space(2), 0, 0)
    }

    private fun matchWidth() = LinearLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT,
        ViewGroup.LayoutParams.WRAP_CONTENT,
    )

    private fun centered(size: Int) = LinearLayout.LayoutParams(size, size).apply {
        gravity = Gravity.CENTER_HORIZONTAL
    }

    private fun dim(id: Int): Int = activity.resources.getDimensionPixelSize(id)

    private fun space(step: Int): Int = dim(when (step) {
        1 -> R.dimen.copypaste_space_1
        2 -> R.dimen.copypaste_space_2
        3 -> R.dimen.copypaste_space_3
        else -> R.dimen.copypaste_space_6
    })

    private fun touchTarget(): Int = dim(R.dimen.copypaste_touch_target)
}
