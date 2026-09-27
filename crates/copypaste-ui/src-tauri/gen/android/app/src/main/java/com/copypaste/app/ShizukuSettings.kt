package com.copypaste.app

import android.content.ComponentName
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import java.util.concurrent.atomic.AtomicBoolean
import rikka.shizuku.Shizuku

internal const val SHIZUKU_SETTINGS_TIMEOUT_MILLIS = 3_000L

internal enum class ShizukuSettingsResult(val changed: Boolean) {
    CONNECTED_CHANGED(true),
    CONNECTED_REFUSED(false),
    DISCONNECTED(false),
    NULL_BINDING(false),
    BINDING_DIED(false),
    BIND_EXCEPTION(false),
    TIMEOUT(false),
}

internal class ShizukuSettingsCompletion(
    private val cancelTimeout: () -> Unit,
    private val releaseService: () -> Unit,
    private val completion: (Boolean) -> Unit,
) {
    private val completed = AtomicBoolean(false)

    fun complete(result: ShizukuSettingsResult) {
        if (!completed.compareAndSet(false, true)) return
        cancelTimeout()
        try {
            releaseService()
        } catch (_: RuntimeException) {
            // Shizuku can die during teardown; the operation still fails closed.
        }
        completion(result.changed)
    }
}

object ShizukuSettings {
    private val mainHandler = Handler(Looper.getMainLooper())

    private fun serviceArgs() = Shizuku.UserServiceArgs(
        ComponentName(BuildConfig.APPLICATION_ID, ShizukuSettingsService::class.java.name),
    )
        .daemon(false)
        .tag("copypaste-clipboard-settings")
        .processNameSuffix("clipboard-settings")
        .debuggable(BuildConfig.DEBUG)
        .version(BuildConfig.VERSION_CODE)

    fun setClipboardAccessNotifications(suppressed: Boolean, completion: (Boolean) -> Unit) {
        mainHandler.post {
            bindUserService(completion) { service ->
                service.setClipboardAccessNotifications(suppressed)
            }
        }
    }

    fun preparePersistentCaptureState(packageName: String, completion: (Boolean) -> Unit) {
        mainHandler.post {
            bindUserService(completion) { service ->
                service.preparePersistentCaptureState(packageName)
            }
        }
    }

    fun refreshClipCascadeSetup(packageName: String, completion: (Boolean) -> Unit) {
        mainHandler.post {
            bindUserService(completion) { service ->
                service.refreshClipCascadeSetup(packageName)
            }
        }
    }

    fun clipboardAccessNotifications(completion: (Boolean?) -> Unit) {
        mainHandler.post {
            if (!hasPermission()) {
                completion(null)
                return@post
            }
            val args = serviceArgs()
            lateinit var connection: ServiceConnection
            lateinit var timeout: Runnable
            val complete = AtomicBoolean(false)
            fun finish(value: Boolean?) {
                if (!complete.compareAndSet(false, true)) return
                mainHandler.removeCallbacks(timeout)
                try {
                    Shizuku.unbindUserService(args, connection, true)
                } catch (_: RuntimeException) {
                } finally {
                    try {
                        Shizuku.unbindUserService(args, connection, false)
                    } catch (_: RuntimeException) {
                    }
                }
                completion(value)
            }
            timeout = Runnable { finish(null) }
            connection = object : ServiceConnection {
                override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                    val service = binder?.let(IShizukuSettingsService.Stub::asInterface)
                    val value = try {
                        if (binder?.pingBinder() == true && service != null) {
                            when (service.clipboardAccessNotifications()) {
                                0 -> true
                                1 -> false
                                else -> null
                            }
                        } else null
                    } catch (_: Exception) {
                        null
                    }
                    finish(value)
                }

                override fun onServiceDisconnected(name: ComponentName?) = finish(null)
                override fun onNullBinding(name: ComponentName?) = finish(null)
                override fun onBindingDied(name: ComponentName?) = finish(null)
            }
            if (!mainHandler.postDelayed(timeout, SHIZUKU_SETTINGS_TIMEOUT_MILLIS)) {
                finish(null)
                return@post
            }
            try {
                Shizuku.bindUserService(args, connection)
            } catch (_: RuntimeException) {
                finish(null)
            }
        }
    }

    private fun bindUserService(
        completion: (Boolean) -> Unit,
        operation: (IShizukuSettingsService) -> Boolean,
    ) {
        if (!hasPermission()) {
            completion(false)
            return
        }

        val args = serviceArgs()
        lateinit var connection: ServiceConnection
        lateinit var timeout: Runnable
        val result = ShizukuSettingsCompletion(
            cancelTimeout = { mainHandler.removeCallbacks(timeout) },
            releaseService = {
                // remove=true does not clear Shizuku 13.1.5's local connection cache.
                try {
                    Shizuku.unbindUserService(args, connection, true)
                } finally {
                    Shizuku.unbindUserService(args, connection, false)
                }
            },
            completion = completion,
        )
        timeout = Runnable { result.complete(ShizukuSettingsResult.TIMEOUT) }

        connection = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                val service = binder?.let(IShizukuSettingsService.Stub::asInterface)
                val changed = try {
                    binder?.pingBinder() == true &&
                        service != null &&
                        operation(service)
                } catch (_: Exception) {
                    false
                }
                result.complete(
                    if (changed) ShizukuSettingsResult.CONNECTED_CHANGED
                    else ShizukuSettingsResult.CONNECTED_REFUSED,
                )
            }

            override fun onServiceDisconnected(name: ComponentName?) =
                result.complete(ShizukuSettingsResult.DISCONNECTED)

            override fun onNullBinding(name: ComponentName?) =
                result.complete(ShizukuSettingsResult.NULL_BINDING)

            override fun onBindingDied(name: ComponentName?) =
                result.complete(ShizukuSettingsResult.BINDING_DIED)
        }

        if (!mainHandler.postDelayed(timeout, SHIZUKU_SETTINGS_TIMEOUT_MILLIS)) {
            result.complete(ShizukuSettingsResult.TIMEOUT)
            return
        }
        try {
            Shizuku.bindUserService(args, connection)
        } catch (_: RuntimeException) {
            result.complete(ShizukuSettingsResult.BIND_EXCEPTION)
        }
    }

    private fun hasPermission(): Boolean = try {
        Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
    } catch (_: RuntimeException) {
        false
    }
}
