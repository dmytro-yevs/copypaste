package com.copypaste.app.widget

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.widget.Toast
import com.copypaste.app.DeviceAuthenticator
import com.copypaste.app.ModuleDelivery
import com.copypaste.app.ModuleOutput
import com.copypaste.app.ModuleSurfaces
import com.copypaste.app.R
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.RuntimeReply
import com.copypaste.app.deliverTextOp
import org.json.JSONObject

/**
 * Copies a tapped widget row (quick-access W2) from the quick-access activity.
 * Secret clips need the device owner first; the runtime writes them with the
 * sensitive flag. A module result runs the surface's activate command: its
 * delivery is copied (after authentication when it asks for it), and a form
 * opens the app. Finishes [activity] when done.
 */
internal class WidgetTapHandler(
    private val activity: Activity,
    private val runtime: RuntimeAccessWorker = RuntimeAccessWorker.native(activity),
) {
    private val authenticator = DeviceAuthenticator(activity)

    fun start(intent: Intent) {
        val id = intent.getStringExtra(WidgetIntents.EXTRA_ITEM_ID)
        val module = intent.getStringExtra(WidgetIntents.EXTRA_MODULE)
        val tap = WidgetTap.entries.firstOrNull { it.name == intent.getStringExtra(WidgetIntents.EXTRA_TAP) }
        if (id == null || tap == null) return activity.finish()
        if (module != null) return activate(module, id)
        when (tap) {
            WidgetTap.Copy -> copy(id)
            WidgetTap.AuthenticateAndCopy -> authenticated { copy(id) }
        }
    }

    fun onActivityResult(requestCode: Int, resultCode: Int): Boolean =
        authenticator.onActivityResult(requestCode, resultCode)

    fun dispose() = authenticator.dispose()

    /** Without a screen lock there is no owner to verify, as everywhere else on the device. */
    private fun authenticated(action: () -> Unit) {
        if (DeviceAuthenticator.availability(activity) == DeviceAuthenticator.Availability.NotConfigured) {
            return action()
        }
        authenticator.authenticate(activity.getString(R.string.widget_auth_reason)) { outcome ->
            if (outcome == DeviceAuthenticator.Outcome.Success) action() else activity.finish()
        }
    }

    /** A clip copy promotes its row (K33). */
    private fun copy(id: String) {
        runtime.request("copy", JSONObject().put("id", id)) { reply -> done(reply is RuntimeReply.Ok) }
    }

    private fun activate(module: String, id: String) {
        runtime.perform({ access ->
            val surface = ModuleSurfaces.quickPaste(access)?.firstOrNull { it.moduleId == module }
            surface?.let { ModuleSurfaces.invoke(access, module, it.activateCommand, JSONObject().put("id", id)) }
        }) { output ->
            when (output) {
                is ModuleOutput.Deliver -> {
                    val delivery = output.delivery
                    if (delivery.requireAuth) authenticated { write(module, delivery) } else write(module, delivery)
                }
                // Forms are filled in in the app.
                is ModuleOutput.View -> {
                    activity.packageManager.getLaunchIntentForPackage(activity.packageName)?.let(activity::startActivity)
                    activity.finish()
                }
                else -> done(false)
            }
        }
    }

    /** Through the runtime: a secret is written as sensitive and never reaches History. */
    private fun write(module: String, delivery: ModuleDelivery) =
        runtime.request("clips", deliverTextOp(delivery, module, "copy")) { reply -> done(reply is RuntimeReply.Ok) }

    private fun done(copied: Boolean) {
        // Android 13+ confirms clipboard writes itself.
        when {
            !copied -> toast(R.string.widget_copy_failed)
            Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU -> toast(R.string.widget_copied)
        }
        activity.finish()
    }

    private fun toast(message: Int) =
        Toast.makeText(activity.applicationContext, message, Toast.LENGTH_SHORT).show()
}
