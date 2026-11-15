package com.copypaste.app.widget

import android.os.Handler
import android.os.Looper
import com.copypaste.app.RuntimeAccessWorker
import java.util.concurrent.atomic.AtomicBoolean

/** Posts and cancels delayed work: a main-thread [Handler] in the app. */
internal interface WidgetTimer {
    fun post(delayMs: Long, task: Runnable)
    fun cancel(task: Runnable)
}

internal class HandlerWidgetTimer(private val handler: Handler = Handler(Looper.getMainLooper())) : WidgetTimer {
    override fun post(delayMs: Long, task: Runnable) {
        handler.postDelayed(task, delayMs)
    }

    override fun cancel(task: Runnable) = handler.removeCallbacks(task)
}

/**
 * Coalesces runtime change signals into refreshes: the first change schedules
 * one refresh [delayMs] later and every change before it joins that refresh.
 * Nothing is scheduled while no widget instance exists, and no timer exists
 * without a change (quick-access W3, R1).
 */
internal class WidgetRefreshScheduler(
    private val timer: WidgetTimer,
    private val hasInstances: () -> Boolean,
    private val refresh: () -> Unit,
    private val delayMs: Long = DEBOUNCE_MS,
) {
    private val pending = AtomicBoolean(false)
    private val fire = Runnable { if (pending.compareAndSet(true, false)) refresh() }

    /** Safe from any thread, including the runtime's upcall thread. */
    fun changed() {
        if (!hasInstances()) return
        if (pending.compareAndSet(false, true)) timer.post(delayMs, fire)
    }

    fun cancel() {
        if (pending.getAndSet(false)) timer.cancel(fire)
    }

    companion object {
        const val DEBOUNCE_MS = 2_000L
    }
}

/** The launcher side of the widget: its instances and how one is shown. */
internal interface WidgetSurface {
    fun instances(): IntArray
    fun config(id: Int): WidgetConfig
    /** Rows that fit the instance's current height. */
    fun rowLimit(id: Int): Int
    /** Before the first unlock the encrypted store must not be opened. */
    fun storeReadable(): Boolean
    fun show(id: Int, config: WidgetConfig, snapshot: WidgetSnapshot)
}

/**
 * Keeps every widget instance current. Ordinary changes are coalesced by
 * [WidgetRefreshScheduler]; a clear (history cleared, device removed) empties
 * every instance at once without touching the runtime, then reloads.
 */
internal class WidgetUpdater(
    private val surface: WidgetSurface,
    private val runtime: RuntimeAccessWorker,
    timer: WidgetTimer,
) {
    private val scheduler = WidgetRefreshScheduler(
        timer,
        hasInstances = { surface.instances().isNotEmpty() },
        refresh = { refresh(surface.instances()) },
    )

    fun changed() = scheduler.changed()

    fun cleared() {
        scheduler.cancel()
        val ids = surface.instances()
        ids.forEach { surface.show(it, surface.config(it), WidgetSnapshot.cleared) }
        scheduler.changed()
    }

    /** Loads and shows [ids] now; [done] runs once they are shown. */
    fun refresh(ids: IntArray, done: () -> Unit = {}) {
        if (ids.isEmpty()) return done()
        val requests = ids.map { Triple(it, surface.config(it), surface.rowLimit(it)) }
        if (!surface.storeReadable()) {
            requests.forEach { (id, config) -> surface.show(id, config, WidgetSnapshot.Unavailable) }
            return done()
        }
        runtime.perform({ access ->
            val loader = WidgetLoader(access)
            val loaded = HashMap<Pair<WidgetConfig, Int>, WidgetSnapshot>()
            requests.map { (id, config, limit) ->
                Triple(id, config, loaded.getOrPut(config to limit) { loader.load(config, limit) })
            }
        }) { results ->
            results.forEach { (id, config, snapshot) -> surface.show(id, config, snapshot) }
            done()
        }
    }
}
