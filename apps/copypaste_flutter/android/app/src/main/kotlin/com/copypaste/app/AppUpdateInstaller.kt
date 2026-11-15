package com.copypaste.app

import android.app.Activity
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import java.io.File
import java.util.UUID

internal class AppUpdateInstaller(private val context: Context) {
    val preferences: SharedPreferences = context.getSharedPreferences(
        "app_update_installation", Context.MODE_PRIVATE,
    )
    private val installer = context.packageManager.packageInstaller

    fun start(file: File, versionCode: Long, versionName: String) {
        check(preferences.getString(stateKey, null) != installing) {
            "An update installation is already active."
        }
        val params = PackageInstaller.SessionParams(
            PackageInstaller.SessionParams.MODE_FULL_INSTALL,
        ).apply {
            setAppPackageName(context.packageName)
            setSize(file.length())
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_REQUIRED)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                setPackageSource(PackageInstaller.PACKAGE_SOURCE_DOWNLOADED_FILE)
            }
        }
        val sessionId = installer.createSession(params)
        val callbackUri = Uri.parse("copypaste-update://result/${UUID.randomUUID()}")
        try {
            check(preferences.edit().clear()
                .putInt(sessionKey, sessionId)
                .putLong(versionKey, versionCode)
                .putString(versionNameKey, versionName)
                .putString(callbackKey, callbackUri.toString())
                .putString(stateKey, installing)
                .commit()) { "Could not save the installation session." }
            installer.openSession(sessionId).use { session ->
                session.openWrite("base.apk", 0, file.length()).use { output ->
                    file.inputStream().use { input -> input.copyTo(output) }
                    session.fsync(output)
                }
                val callback = PendingIntent.getBroadcast(
                    context,
                    sessionId,
                    Intent(context, AppUpdateResultReceiver::class.java).apply {
                        action = resultAction
                        data = callbackUri
                    },
                    PendingIntent.FLAG_UPDATE_CURRENT or
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                            PendingIntent.FLAG_MUTABLE
                        } else 0,
                )
                session.commit(callback.intentSender)
                if (state() == installing) {
                    preferences.edit().putBoolean(committedKey, true).commit()
                }
            }
        } catch (error: Exception) {
            runCatching { installer.abandonSession(sessionId) }
            preferences.edit().clear().commit()
            throw error
        }
    }

    fun receive(intent: Intent) {
        if (state() != installing || intent.action != resultAction ||
            intent.dataString != preferences.getString(callbackKey, null) ||
            intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID, -1) !=
            preferences.getInt(sessionKey, -1)
        ) return
        val status = intent.getIntExtra(
            PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE,
        )
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            val confirmation = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
            }
            if (confirmation == null) {
                finish("installation_failed")
            } else {
                preferences.edit()
                    .putString(confirmationKey, confirmation.toUri(Intent.URI_INTENT_SCHEME))
                    .commit()
            }
        } else {
            finish(appUpdateResultCode(status))
        }
    }

    fun confirm(activity: Activity) {
        val encoded = preferences.getString(confirmationKey, null) ?: return
        preferences.edit().remove(confirmationKey).commit()
        try {
            activity.startActivity(Intent.parseUri(encoded, Intent.URI_INTENT_SCHEME))
        } catch (_: Exception) {
            runCatching { installer.abandonSession(preferences.getInt(sessionKey, -1)) }
            finish("installation_failed")
        }
    }

    fun restore() {
        if (preferences.getString(stateKey, null) != installing) return
        val info = installer.getSessionInfo(preferences.getInt(sessionKey, -1))
        if (info != null) {
            // A process can die after staging but before committing the session.
            val committed = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                info.isSealed
            } else preferences.getBoolean(committedKey, false)
            if (!committed) {
                installer.abandonSession(info.sessionId)
                finish("installation_interrupted")
            }
            return
        }
        @Suppress("DEPRECATION")
        val installed = context.packageManager.getPackageInfo(context.packageName, 0)
        val version = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            installed.longVersionCode
        } else {
            @Suppress("DEPRECATION")
            installed.versionCode.toLong()
        }
        finish(if (version >= preferences.getLong(versionKey, Long.MAX_VALUE) &&
            installed.versionName == preferences.getString(versionNameKey, null)
        ) {
            "installed"
        } else "installation_interrupted")
    }

    fun state(): String? = preferences.getString(stateKey, null)

    fun clear() {
        preferences.edit().clear().commit()
    }

    private fun finish(code: String) {
        preferences.edit().remove(confirmationKey).putString(stateKey, code).commit()
    }

    companion object {
        const val stateKey = "state"
        const val confirmationKey = "confirmation"
        const val installing = "installing"
        private const val resultAction = "com.copypaste.app.UPDATE_INSTALL_RESULT"
        private const val sessionKey = "session"
        private const val versionKey = "version"
        private const val versionNameKey = "version_name"
        private const val committedKey = "committed"
        private const val callbackKey = "callback"
    }
}
