package com.copypaste.app.widget

import com.copypaste.app.JsonRuntimeAccess
import com.copypaste.app.RuntimeAccessWorker
import java.util.concurrent.Executor
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class WidgetRefreshTest {
    private class FakeTimer : WidgetTimer {
        val posted = mutableListOf<Runnable>()
        override fun post(delayMs: Long, task: Runnable) {
            assertEquals(WidgetRefreshScheduler.DEBOUNCE_MS, delayMs)
            posted += task
        }
        override fun cancel(task: Runnable) {
            posted.remove(task)
        }
        fun fire() = posted.toList().also { posted.clear() }.forEach(Runnable::run)
    }

    private class FakeSurface(var ids: IntArray, var readable: Boolean = true) : WidgetSurface {
        val shown = mutableListOf<Pair<Int, WidgetSnapshot>>()
        override fun instances() = ids
        override fun config(id: Int) = WidgetConfig()
        override fun rowLimit(id: Int) = 5
        override fun storeReadable() = readable
        override fun show(id: Int, config: WidgetConfig, snapshot: WidgetSnapshot) {
            shown += id to snapshot
        }
    }

    private val requests = mutableListOf<String>()
    private val timer = FakeTimer()
    private val direct = Executor { it.run() }
    private val runtime = RuntimeAccessWorker(
        JsonRuntimeAccess { raw ->
            val method = JSONObject(raw).getString("method")
            requests += method
            when (method) {
                "get_private_mode" -> """{"id":0,"ok":true,"data":{"private_mode":{"private_mode":false,"private_mode_epoch":0}}}"""
                else -> """{"id":0,"ok":true,"data":{"page":{"items":[{"id":"a","content":"hello","content_type":"text/plain","created_at":1,"pinned":false}],"skipped_undecryptable":0}}}"""
            }
        },
        direct,
        direct,
    )

    @Test fun aBurstOfChangesRefreshesOnceAfterTheWindow() {
        val surface = FakeSurface(intArrayOf(1, 2))
        val updater = WidgetUpdater(surface, runtime, timer)

        repeat(50) { updater.changed() }

        assertEquals(1, timer.posted.size)
        assertTrue(requests.isEmpty())
        timer.fire()
        // Two instances with one configuration share one load.
        assertEquals(1, requests.count { it == "history_query" })
        assertEquals(listOf(1, 2), surface.shown.map { it.first })
        assertEquals("hello", (surface.shown.first().second as WidgetSnapshot.Rows).rows.single().text)
    }

    @Test fun withoutChangesOrInstancesNothingIsScheduledOrRead() {
        val surface = FakeSurface(intArrayOf())
        val updater = WidgetUpdater(surface, runtime, timer)

        updater.changed()

        assertTrue(timer.posted.isEmpty())
        assertTrue(requests.isEmpty())
        assertTrue(surface.shown.isEmpty())
    }

    @Test fun clearingEmptiesEveryInstanceAtOnceWithoutReadingTheStore() {
        val surface = FakeSurface(intArrayOf(3, 4))
        val updater = WidgetUpdater(surface, runtime, timer)
        updater.changed()

        updater.cleared()

        assertEquals(listOf(3 to WidgetSnapshot.cleared, 4 to WidgetSnapshot.cleared), surface.shown)
        assertTrue(requests.isEmpty())
        assertEquals("the reload after a clear is one coalesced refresh", 1, timer.posted.size)
    }

    @Test fun beforeTheFirstUnlockTheStoreIsNeverOpened() {
        val surface = FakeSurface(intArrayOf(5), readable = false)
        var done = false

        WidgetUpdater(surface, runtime, timer).refresh(intArrayOf(5)) { done = true }

        assertTrue(done)
        assertEquals(listOf(5 to WidgetSnapshot.Unavailable), surface.shown)
        assertTrue(requests.isEmpty())
    }
}
