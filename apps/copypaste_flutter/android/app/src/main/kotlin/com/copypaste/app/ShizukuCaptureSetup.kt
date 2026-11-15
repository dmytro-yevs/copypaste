package com.copypaste.app

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import rikka.shizuku.Shizuku
import java.util.concurrent.Executors

internal data class ShizukuFacts(
    val supported: Boolean,
    val installed: Boolean,
    val running: Boolean,
    val permission: Boolean,
) {
    fun asMap(): Map<String, Boolean> = mapOf(
        "supported" to supported,
        "installed" to installed,
        "running" to running,
        "permission" to permission,
    )
}

internal class ShizukuCaptureSetup(
    private val context: Context,
    private val sms: Boolean = false,
    private val onChanged: () -> Unit,
) {
    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor()
    private val permissionRequest = if (sms) 4921 else 4919
    private var pending: ((Boolean) -> Unit)? = null
    private var cancelGrants: (() -> Unit)? = null
    private var disposed = false
    private val binderReceived = Shizuku.OnBinderReceivedListener { onChanged() }
    private val binderDead = Shizuku.OnBinderDeadListener {
        cancelGrants?.invoke()
        if (pending != null) {
            val completion = pending
            pending = null
            completion?.invoke(false)
        }
        onChanged()
    }

    private val permissionListener =
        Shizuku.OnRequestPermissionResultListener { requestCode, grantResult ->
            if (requestCode != permissionRequest) return@OnRequestPermissionResultListener
            onChanged()
            val completion = pending ?: return@OnRequestPermissionResultListener
            if (grantResult != PackageManager.PERMISSION_GRANTED) {
                pending = null
                completion(false)
                return@OnRequestPermissionResultListener
            }
            applyGrants(completion)
        }

    init {
        Shizuku.addRequestPermissionResultListener(permissionListener)
        Shizuku.addBinderReceivedListenerSticky(binderReceived)
        Shizuku.addBinderDeadListener(binderDead)
    }

    fun dispose() {
        disposed = true
        Shizuku.removeRequestPermissionResultListener(permissionListener)
        Shizuku.removeBinderReceivedListener(binderReceived)
        Shizuku.removeBinderDeadListener(binderDead)
        cancelGrants?.invoke()
        pending?.invoke(false)
        pending = null
        worker.shutdownNow()
    }

    fun facts(): ShizukuFacts {
        val running = runCatching(Shizuku::pingBinder).getOrDefault(false)
        val permission = running && runCatching {
            Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
        }.getOrDefault(false)
        return ShizukuFacts(
            supported = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R,
            installed = isInstalled(),
            running = running,
            permission = permission,
        )
    }

    fun requestAndApply(completion: (Boolean) -> Unit) {
        if (disposed || pending != null) {
            completion(false)
            return
        }
        val facts = facts()
        if (!facts.supported || !facts.installed || !facts.running) {
            completion(false)
            return
        }
        pending = completion
        if (facts.permission) {
            applyGrants(completion)
            return
        }
        if (runCatching { Shizuku.shouldShowRequestPermissionRationale() }.getOrDefault(false)) {
            pending = null
            completion(false)
            return
        }
        try {
            Shizuku.requestPermission(permissionRequest)
        } catch (_: RuntimeException) {
            pending = null
            completion(false)
        }
    }

    private fun applyGrants(completion: (Boolean) -> Unit) {
        val args = Shizuku.UserServiceArgs(
            ComponentName(context.packageName, ShizukuGrantService::class.java.name),
        )
            .daemon(false)
            .tag(if (sms) "copypaste-sms-grants" else "copypaste-capture-grants")
            .processNameSuffix(if (sms) "sms-grants" else "capture-grants")
            .debuggable(BuildConfig.DEBUG)
            .version(BuildConfig.VERSION_CODE)
        lateinit var connection: ServiceConnection
        var finished = false
        fun finish(success: Boolean) {
            if (finished) return
            finished = true
            cancelGrants = null
            main.removeCallbacksAndMessages(connection)
            runCatching { Shizuku.unbindUserService(args, connection, true) }
            runCatching { Shizuku.unbindUserService(args, connection, false) }
            pending = null
            completion(success)
            onChanged()
        }
        connection = object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                if (finished || disposed) return
                val service = binder?.let(IShizukuGrantService.Stub::asInterface)
                // Binder waits and shell commands must never block Android's UI thread.
                worker.execute {
                    val applied = try {
                        binder?.pingBinder() == true &&
                            service != null &&
                            if (sms) service.applySmsGrants(context.packageName,
                                android.os.Process.myUserHandle().hashCode(), AndroidSmsAccess.otpOpSupported(context))
                            else service.applyCaptureGrants(context.packageName)
                    } catch (_: Exception) {
                        false
                    }
                    main.post { finish(applied) }
                }
            }

            override fun onServiceDisconnected(name: ComponentName?) = finish(false)
            override fun onNullBinding(name: ComponentName?) = finish(false)
            override fun onBindingDied(name: ComponentName?) = finish(false)
        }
        cancelGrants = { finish(false) }
        main.postAtTime({ finish(false) }, connection, android.os.SystemClock.uptimeMillis() + timeoutMs)
        try {
            Shizuku.bindUserService(args, connection)
        } catch (_: RuntimeException) {
            finish(false)
        }
    }

    private fun isInstalled(): Boolean = try {
        context.packageManager.getPackageInfo(shizukuPackage, 0)
        true
    } catch (_: PackageManager.NameNotFoundException) {
        false
    }

    companion object {
        private const val timeoutMs = 5_000L
        const val shizukuPackage = "moe.shizuku.privileged.api"
    }
}
