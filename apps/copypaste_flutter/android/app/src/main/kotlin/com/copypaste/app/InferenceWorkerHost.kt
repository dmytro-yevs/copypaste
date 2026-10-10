package com.copypaste.app

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import android.os.Looper
import android.os.ParcelFileDescriptor
import androidx.annotation.Keep
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

/** JNI launcher. Binding callbacks never perform inference or wait for replies. */
@Keep
internal object InferenceWorkerHost {
    private lateinit var context: Context
    private val nextId = AtomicLong(1)
    private val sessions = ConcurrentHashMap<Long, Session>()

    fun initialize(application: Context) { context = application.applicationContext }

    @JvmStatic fun openWorker(): LongArray {
        check(Looper.myLooper() != Looper.getMainLooper())
        val session = Session(context)
        val pair = ParcelFileDescriptor.createSocketPair()
        try {
            check(session.bind())
            check(session.connected.await(30, TimeUnit.SECONDS))
            val remote = checkNotNull(session.remote)
            check(remote.open(pair[1]) > 0)
            val id = nextId.getAndIncrement()
            sessions[id] = session
            return longArrayOf(pair[0].detachFd().toLong(), id)
        } catch (_: Exception) {
            session.close()
            return longArrayOf(-1, 0)
        } finally {
            pair.forEach { it.close() }
        }
    }

    @JvmStatic fun closeWorker(id: Long) { sessions.remove(id)?.close() }

    private class Session(private val context: Context) : ServiceConnection {
        val connected = CountDownLatch(1)
        private val died = CountDownLatch(1)
        private val closed = AtomicBoolean(false)
        @Volatile var remote: IInferenceWorker? = null
        @Volatile private var bound = false

        fun bind(): Boolean {
            bound = context.bindService(Intent(context, InferenceWorkerService::class.java), this, Context.BIND_AUTO_CREATE)
            return bound
        }

        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            remote = IInferenceWorker.Stub.asInterface(binder)
            try { binder.linkToDeath({ died.countDown() }, 0) }
            catch (_: Exception) { died.countDown() }
            connected.countDown()
            if (closed.get()) {
                // A delayed binding must not leave an orphan after launch timed out.
                Thread({ terminate() }, "inference-launch-cancel").start()
            }
        }

        override fun onServiceDisconnected(name: ComponentName) { died.countDown(); connected.countDown() }
        override fun onNullBinding(name: ComponentName) { connected.countDown() }
        override fun onBindingDied(name: ComponentName) { died.countDown(); connected.countDown() }

        fun close() {
            if (!closed.compareAndSet(false, true)) return
            terminate()
        }

        private fun terminate() {
            val service = remote
            try {
                if (service != null) {
                    try { service.stop() } catch (_: Exception) { /* Process death closes Binder. */ }
                    check(died.await(5, TimeUnit.SECONDS)) { "Inference process did not terminate." }
                }
            } finally {
                if (bound) {
                    bound = false
                    try { context.unbindService(this) } catch (_: IllegalArgumentException) { }
                }
            }
        }
    }
}
