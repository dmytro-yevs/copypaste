package com.copypaste.app

import android.content.ComponentName
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.IBinder
import java.util.concurrent.atomic.AtomicBoolean
import rikka.shizuku.Shizuku

object ShizukuSettings {
    private fun serviceArgs() = Shizuku.UserServiceArgs(
        ComponentName(BuildConfig.APPLICATION_ID, ShizukuSettingsService::class.java.name),
    )
        .daemon(false)
        .tag("copypaste-clipboard-settings")
        .processNameSuffix("clipboard-settings")
        .debuggable(BuildConfig.DEBUG)
        .version(BuildConfig.VERSION_CODE)

    fun setClipboardAccessNotifications(suppressed: Boolean, completion: (Boolean) -> Unit) {
        if (!hasPermission()) {
            completion(false)
            return
        }

        val args = serviceArgs()
        val completed = AtomicBoolean(false)
        lateinit var connection: ServiceConnection

        fun complete(changed: Boolean) {
            if (!completed.compareAndSet(false, true)) return
            try {
                Shizuku.unbindUserService(args, connection, true)
            } catch (_: RuntimeException) {
                // The result still fails closed if Shizuku died before teardown.
            }
            completion(changed)
        }

        connection = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                val service = binder?.let(IShizukuSettingsService.Stub::asInterface)
                val changed = try {
                    binder?.pingBinder() == true &&
                        service?.setClipboardAccessNotifications(suppressed) == true
                } catch (_: Exception) {
                    false
                }
                complete(changed)
            }

            override fun onServiceDisconnected(name: ComponentName?) = complete(false)
        }

        try {
            Shizuku.bindUserService(args, connection)
        } catch (_: RuntimeException) {
            complete(false)
        }
    }

    private fun hasPermission(): Boolean = try {
        Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
    } catch (_: RuntimeException) {
        false
    }
}
