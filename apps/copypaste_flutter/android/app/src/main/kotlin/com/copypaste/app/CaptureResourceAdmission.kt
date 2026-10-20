package com.copypaste.app

import java.io.InputStream
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** Reserves capacity before accepting work, so cleanup never falls back to the caller. */
internal class CaptureSlots(size: Int) {
    private val lock = java.lang.Object()
    private var available = size
    fun reserve(): Lease? = synchronized(lock) { take() }
    fun await(active: () -> Boolean): Lease? = synchronized(lock) {
        while (available == 0 && active()) lock.wait()
        if (active()) take() else null
    }
    fun wake() = synchronized(lock) { lock.notifyAll() }
    private fun take(): Lease? {
        if (available == 0) return null
        available--
        return Lease { synchronized(lock) { available++; lock.notifyAll() } }
    }
    internal class Lease(private val release: () -> Unit) {
        private val closed = AtomicBoolean()
        fun close() { if (closed.compareAndSet(false, true)) release() }
    }
}

internal class CaptureBytes(private val maximum: Long) {
    private var reserved = 0L
    @Synchronized fun reserve(bytes: Long): CaptureSlots.Lease? {
        if (bytes < 0 || bytes > maximum - reserved) return null
        reserved += bytes
        return CaptureSlots.Lease { synchronized(this) { reserved -= bytes } }
    }
}

internal fun captureCleanupExecutor(capacity: Int): ThreadPoolExecutor =
    ThreadPoolExecutor(2, 2, 30, TimeUnit.SECONDS, ArrayBlockingQueue(capacity)).apply {
        allowCoreThreadTimeOut(true)
    }

/** A blocked provider close retains its slot until the close actually finishes. */
internal class CaptureInput(private val source: InputStream, private val slot: CaptureSlots.Lease) : InputStream() {
    private val closing = AtomicBoolean()
    override fun read(): Int = source.read()
    override fun read(buffer: ByteArray, offset: Int, length: Int): Int = source.read(buffer, offset, length)
    override fun close() {
        if (!closing.compareAndSet(false, true)) return
        try { source.close() } finally { slot.close() }
    }
}

internal class CaptureHeavyAdmission {
    private val lock = java.lang.Object()
    private var occupied = false
    fun acquire(active: () -> Boolean): Boolean = synchronized(lock) {
        while (occupied && active()) lock.wait()
        if (!active()) return false
        occupied = true
        true
    }
    fun release() = synchronized(lock) { occupied = false; lock.notifyAll() }
    fun wake() = synchronized(lock) { lock.notifyAll() }
}
