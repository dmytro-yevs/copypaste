package com.copypaste.app

import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import android.os.ParcelFileDescriptor
import android.os.Process
import androidx.annotation.Keep
import java.util.concurrent.atomic.AtomicBoolean

/** Owns native inference only; the main process retains history and keys. */
@Keep
class InferenceWorkerService : Service() {
    private val opened = AtomicBoolean(false)
    private val binder = object : IInferenceWorker.Stub() {
        override fun open(channel: ParcelFileDescriptor): Int {
            check(Binder.getCallingUid() == Process.myUid())
            check(opened.compareAndSet(false, true))
            val descriptor = channel.detachFd()
            Thread({
                try { runWorker(descriptor) }
                finally { Process.killProcess(Process.myPid()) }
            }, "semantic-inference").start()
            return Process.myPid()
        }

        override fun stop() {
            check(Binder.getCallingUid() == Process.myUid())
            Process.killProcess(Process.myPid())
        }
    }

    override fun onBind(intent: Intent?): IBinder = binder

    override fun onDestroy() {
        super.onDestroy()
        // Unbinding alone permits Android to cache this process and its native
        // allocator. Only the inference service occupies this process.
        Process.killProcess(Process.myPid())
    }

    companion object {
        init { System.loadLibrary("copypaste_flutter_bridge") }
        @JvmStatic private external fun runWorker(descriptor: Int)
    }
}
