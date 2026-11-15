package com.copypaste.app

import java.util.concurrent.CopyOnWriteArraySet

/** Process-local capture-state invalidation; listeners exist only for an EventChannel subscription. */
internal object AndroidCaptureSignals {
    private val listeners = CopyOnWriteArraySet<() -> Unit>()
    fun add(listener: () -> Unit) { listeners.add(listener) }
    fun remove(listener: () -> Unit) { listeners.remove(listener) }
    fun changed() { listeners.forEach { it() } }
}
