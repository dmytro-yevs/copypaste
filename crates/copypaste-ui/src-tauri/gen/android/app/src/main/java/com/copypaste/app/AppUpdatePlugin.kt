package com.copypaste.app

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileInputStream

/** Stages a verified APK through PackageInstaller; it never opens ACTION_VIEW. */
@TauriPlugin
class AppUpdatePlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingInvoke: Invoke? = null

    init { instance = this }

    private fun installPermissionRequired(): Boolean {
        return Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !activity.packageManager.canRequestPackageInstalls()
    }

    private fun openInstallPermission() {
        activity.startActivity(
            Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES)
                .setData(Uri.parse("package:${activity.packageName}"))
        )
    }

    @Command
    fun prepareInstall(invoke: Invoke) {
        try {
            if (installPermissionRequired()) {
                openInstallPermission()
                invoke.resolve(JSObject().put("status", "permission_required"))
            } else {
                invoke.resolve(JSObject().put("status", "success"))
            }
        } catch (_: Throwable) {
            invoke.reject("The update permission could not be opened.")
        }
    }

    @Command
    fun stageAndInstall(invoke: Invoke) {
        if (pendingInvoke != null) {
            invoke.reject("Another update action is already running.")
            return
        }
        val path = invoke.getArgs().optString("path", "")
        val apk = File(path)
        if (!apk.isFile || apk.name != "copypaste-update.apk") {
            invoke.reject("The update could not be staged.")
            return
        }
        if (apk.canonicalFile.parentFile != activity.cacheDir.canonicalFile ||
            apk.name != "copypaste-update.apk"
        ) {
            invoke.reject("The update could not be staged.")
            return
        }
        var session: PackageInstaller.Session? = null
        try {
            if (installPermissionRequired()) {
                openInstallPermission()
                invoke.resolve(JSObject().put("status", "permission_required"))
                return
            }
            val installer = activity.packageManager.packageInstaller
            val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
            params.setAppPackageName(activity.packageName)
            val sessionId = installer.createSession(params)
            session = installer.openSession(sessionId)
            FileInputStream(apk).use { input ->
                session!!.openWrite("base.apk", 0, apk.length()).use { output ->
                    input.copyTo(output)
                    session!!.fsync(output)
                }
            }
            val intent = Intent(activity, UpdateReceiver::class.java)
                .setPackage(activity.packageName)
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
            val pending = PendingIntent.getBroadcast(activity, sessionId, intent, flags)
            pendingInvoke = invoke
            session!!.commit(pending.intentSender)
            session!!.close()
            session = null
        } catch (_: Throwable) {
            pendingInvoke = null
            try { session?.abandon() } catch (_: Throwable) { }
            try { session?.close() } catch (_: Throwable) { }
            invoke.reject("The update could not be staged.")
        }
    }

    fun complete(status: Int, confirmation: Intent?) {
        val invoke = pendingInvoke ?: return
        when (status) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                confirmation?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                if (confirmation != null) activity.startActivity(confirmation)
            }
            PackageInstaller.STATUS_SUCCESS -> {
                pendingInvoke = null
                invoke.resolve(JSObject().put("status", "success"))
            }
            else -> {
                pendingInvoke = null
                invoke.reject("The update could not be installed.")
            }
        }
    }

    companion object {
        @Volatile var instance: AppUpdatePlugin? = null
    }
}

class UpdateReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, -1)
        val confirmation = if (status == PackageInstaller.STATUS_PENDING_USER_ACTION && Build.VERSION.SDK_INT >= 33) {
                intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
            } else if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra(Intent.EXTRA_INTENT)
            } else null
        AppUpdatePlugin.instance?.complete(status, confirmation)
        if (status == PackageInstaller.STATUS_SUCCESS) {
            context.packageManager.getLaunchIntentForPackage(context.packageName)?.let { launch ->
                launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
                context.startActivity(launch)
            }
        }
    }
}
