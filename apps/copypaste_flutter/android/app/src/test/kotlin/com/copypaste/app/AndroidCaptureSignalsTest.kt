package com.copypaste.app

import io.flutter.plugin.common.EventChannel
import java.util.ArrayDeque
import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidCaptureSignalsTest {
    private class Sink : EventChannel.EventSink {
        val values = mutableListOf<Map<String, Any>>()
        override fun success(event: Any?) {
            @Suppress("UNCHECKED_CAST")
            values += event as Map<String, Any>
        }
        override fun error(code: String, message: String?, details: Any?) = Unit
        override fun endOfStream() = Unit
    }

    @Test fun statePublisherSendsInitialSignalChangesAndStopsAfterUnsubscribe() {
        var running = false
        val publisher = AndroidCaptureStatePublisher(
            state = { mapOf("serviceRunning" to running, "observedAtMs" to 99L) },
            now = { 42L },
        )
        val sink = Sink()
        val listener: () -> Unit = publisher::publish
        AndroidCaptureSignals.add(listener)
        publisher.listen(sink)
        running = true
        AndroidCaptureSignals.changed()
        AndroidCaptureSignals.changed()
        AndroidCaptureSignals.remove(listener)
        publisher.cancel()
        running = false
        AndroidCaptureSignals.changed()

        assertEquals(2, sink.values.size)
        assertEquals(false, sink.values[0]["serviceRunning"])
        assertEquals(true, sink.values[1]["serviceRunning"])
        assertEquals(42L, sink.values[1]["observedAtMs"])
    }

    @Test fun listenersReceiveChangesOnlyWhileRegistered() {
        var calls = 0
        val listener: () -> Unit = { calls++ }
        AndroidCaptureSignals.add(listener)
        try {
            AndroidCaptureSignals.changed()
            assertEquals(1, calls)
        } finally {
            AndroidCaptureSignals.remove(listener)
        }
        AndroidCaptureSignals.changed()
        assertEquals(1, calls)
    }

    @Test fun dispatcherCoalescesBurstsAndDropsQueuedWorkAfterCancel() {
        val queue = ArrayDeque<Runnable>()
        var published = 0
        val dispatcher = AndroidCaptureSignalDispatcher(
            post = { task -> queue.add(task); true },
            remove = { task -> queue.remove(task) },
            publish = { published++ },
        )
        repeat(10) { dispatcher.changed() }
        assertEquals(1, queue.size)
        queue.removeFirst().run()
        assertEquals(1, published)
        dispatcher.changed()
        dispatcher.cancel()
        assertEquals(0, queue.size)
    }
}
