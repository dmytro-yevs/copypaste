package com.copypaste.app

import java.util.concurrent.CopyOnWriteArraySet

/** Process-local state invalidation; listeners are held only while subscribed. */
internal object ScreenshotCaptureSignals {
    private val listeners = CopyOnWriteArraySet<() -> Unit>()
    fun add(listener: () -> Unit) { listeners.add(listener) }
    fun remove(listener: () -> Unit) { listeners.remove(listener) }
    fun changed() { listeners.forEach { it() } }
}
