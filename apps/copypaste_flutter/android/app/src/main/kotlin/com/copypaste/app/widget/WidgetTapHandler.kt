package com.copypaste.app.widget

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.widget.Toast
import com.copypaste.app.DeviceAuthenticator
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeReply
import org.json.JSONObject

/**
 * Copies a tapped widget row (quick-access W2) from the quick-access activity.
 * Secrets and concealed snippets need the device owner first; the runtime
 * writes them with the sensitive flag. Finishes [activity] when done.
 */
internal class WidgetTapHandler(
    private val activity: Activity,
    private val runtime: RuntimeAccessWorker = RuntimeAccessWorker.native(activity),
) {
    private val authenticator = DeviceAuthenticator(activity)

    fun start(intent: Intent) {
        val id = intent.getStringExtra(WidgetIntents.EXTRA_ITEM_ID)
        val snippet = intent.getBooleanExtra(WidgetIntents.EXTRA_SNIPPET, false)
        val tap = WidgetTap.entries.firstOrNull { it.name == intent.getStringExtra(WidgetIntents.EXTRA_TAP) }
        if (id == null || tap == null) return activity.finish()
        when (tap) {
            WidgetTap.Copy -> copy(id, snippet)
            WidgetTap.AuthenticateAndCopy -> authenticateThenCopy(id, snippet)
            // The template form lives in the app until it has a native one.
            WidgetTap.FillTemplate -> {
                activity.packageManager.getLaunchIntentForPackage(activity.packageName)
                    ?.let(activity::startActivity)
                activity.finish()
            }
        }
    }

    fun onActivityResult(requestCode: Int, resultCode: Int): Boolean =
        authenticator.onActivityResult(requestCode, resultCode)

    fun dispose() = authenticator.dispose()

    /** Without a screen lock there is no owner to verify, as everywhere else on the device. */
    private fun authenticateThenCopy(id: String, snippet: Boolean) {
        if (DeviceAuthenticator.availability(activity) == DeviceAuthenticator.Availability.NotConfigured) {
            return copy(id, snippet)
        }
        authenticator.authenticate(activity.getString(R.string.widget_auth_reason)) { outcome ->
            if (outcome == DeviceAuthenticator.Outcome.Success) copy(id, snippet) else activity.finish()
        }
    }

    private fun copy(id: String, snippet: Boolean) {
        val (method, params) = copyRequest(id, snippet)
        runtime.request(method, params) { reply ->
            // Android 13+ confirms clipboard writes itself.
            when {
                reply !is RuntimeReply.Ok -> toast(R.string.widget_copy_failed)
                Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU -> toast(R.string.widget_copied)
            }
            activity.finish()
        }
    }

    private fun toast(message: Int) =
        Toast.makeText(activity.applicationContext, message, Toast.LENGTH_SHORT).show()

    companion object {
        /** A clip copy promotes its row (K33); a snippet is rendered without fields. */
        fun copyRequest(id: String, snippet: Boolean): Pair<String, JSONObject> =
            if (snippet) {
                "snippets" to JSONObject().put(
                    "op",
                    JSONObject()
                        .put("op", "use")
                        .put("id", id)
                        .put("values", JSONObject())
                        .put("clipboard", JSONObject().put("kind", "none"))
                        .put("delivery", "copy"),
                )
            } else {
                "copy" to JSONObject().put("id", id)
            }
    }
}
