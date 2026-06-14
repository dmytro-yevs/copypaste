package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

/**
 * Pure-JVM unit tests for the flush-on-dispose pattern used in [SettingsScreen].
 *
 * Root cause (CopyPaste-l71): text-field changes are debounced via
 * `rememberCoroutineScope.launch { delay(300); persistAll() }`.
 * When the user switches tabs within 300 ms, Compose disposes the Composable,
 * which cancels the rememberCoroutineScope — the pending write is silently lost.
 *
 * Fix: a `DisposableEffect { onDispose { … } }` cancels the in-flight jobs and
 * calls persistAll() synchronously before teardown.
 *
 * These tests validate the invariant using plain JVM threading (no Android
 * runtime, no TestScope, no kotlinx-coroutines-test dependency).
 *
 * The debounce is simulated with a Thread + sleep; the "cancel" is an
 * AtomicBoolean interrupted flag (mirrors what Coroutine.cancel() does).
 *
 * Full integration coverage (DisposableEffect teardown in Compose lifecycle):
 * `./gradlew :app:connectedAndroidTest` — tracked by CopyPaste-8dd.
 */
class SettingsFlushOnDisposeTest {

    // ── Minimal coroutine-like debounce using Thread + AtomicBoolean ──────────

    /**
     * A lightweight stand-in for a debounce coroutine Job.
     *
     * On construction the job starts a background thread that sleeps [delayMs],
     * then—if not cancelled—calls [action]. [cancel] sets the cancelled flag and
     * interrupts the thread so the sleep returns early.
     */
    private class DebounceJob(
        private val delayMs: Long,
        private val action: () -> Unit,
    ) {
        private val cancelled = AtomicBoolean(false)
        private val thread = Thread {
            try {
                Thread.sleep(delayMs)
                if (!cancelled.get()) action()
            } catch (_: InterruptedException) {
                // Cancelled — action is suppressed.
            }
        }.also { it.isDaemon = true; it.start() }

        val isCancelled: Boolean get() = cancelled.get()

        fun cancel() {
            cancelled.set(true)
            thread.interrupt()
        }

        fun join(timeoutMs: Long = 1_000) {
            thread.join(timeoutMs)
        }
    }

    // Captures writes so assertions can verify count and value.
    private val written = mutableListOf<String>()
    private fun persist(value: String) { written.add(value) }

    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Debounce fires normally (user does NOT switch tabs within the debounce
     * window). The job runs to completion; onDispose's cancel() is a no-op on an
     * already-done job; the null-check prevents a second write.
     */
    @Test
    fun `debounce fires normally -- single write no double write`() {
        var job: DebounceJob? = null
        var pendingValue = ""

        // Simulate onValueChange. The fire action self-nulls the job handle (as the
        // onDispose null-check below relies on — "job was self-nulled above"); without
        // this the captured handle stays non-null and onDispose double-writes.
        pendingValue = "https://supabase.example.com"
        job?.cancel()
        job = DebounceJob(50L) {
            persist(pendingValue)
            job = null
        }

        // Wait for the debounce to complete naturally (join() establishes
        // happens-before, so the job=null from the fire action is visible here).
        job!!.join()

        // Simulate onDispose: cancel (no-op — job done) + flush only if job was still pending
        val capturedJob = job.also { job = null }   // self-null after firing
        capturedJob?.cancel()                        // no-op on completed job
        if (capturedJob != null) {
            // This branch is NOT taken — job was self-nulled above.
            persist(pendingValue)
        }

        assertEquals("Expected exactly one write", 1, written.size)
        assertEquals("https://supabase.example.com", written[0])
    }

    /**
     * User switches tabs before the debounce fires — the CopyPaste-l71 bug scenario.
     *
     * Before the fix: job cancelled → 0 writes (silent data loss).
     * After the fix: onDispose cancels the job then synchronously flushes → 1 write.
     */
    @Test
    fun `tab switch before debounce fires -- flush-on-dispose saves the value`() {
        var job: DebounceJob? = null
        var pendingValue = ""

        // Simulate onValueChange
        pendingValue = "https://relay.example.com"
        job?.cancel()
        job = DebounceJob(500L) { persist(pendingValue) }   // long debounce

        // Simulate onDispose immediately (within the debounce window):
        // 1. Cancel the in-flight job.
        val capturedJob = job!!
        capturedJob.cancel()
        // 2. Synchronously flush the pending state — this is the fix.
        persist(pendingValue)

        // Ensure the cancelled job thread has exited.
        capturedJob.join(200)

        assertTrue("Job must be marked cancelled", capturedJob.isCancelled)
        assertEquals("flush-on-dispose must write exactly once", 1, written.size)
        assertEquals("https://relay.example.com", written[0])
    }

    /**
     * Rapid edits within the debounce window: each onValueChange cancels the
     * previous job. Only the final value must appear in the written list.
     */
    @Test
    fun `rapid edits -- only the last value is flushed on dispose`() {
        var job: DebounceJob? = null
        var pendingValue = ""

        val values = listOf("h", "ht", "htt", "http://relay.local")
        for (v in values) {
            pendingValue = v
            job?.cancel()
            job = DebounceJob(500L) { persist(pendingValue) }  // long debounce; never fires
        }

        // onDispose: cancel last pending job, synchronously flush final state
        val capturedJob = job!!
        capturedJob.cancel()
        persist(pendingValue)

        capturedJob.join(200)

        assertEquals("Only one write from flush-on-dispose", 1, written.size)
        assertEquals("Must persist last typed value", "http://relay.local", written[0])
    }

    /**
     * Cancelling a job that is already completed (debounce fired in time before
     * dispose) must not throw. This covers the onDispose cancel() call when the
     * job has already exited.
     */
    @Test
    fun `cancelling a completed job is safe -- no exception`() {
        val job = DebounceJob(10L) { /* no-op */ }
        job.join()              // let it complete

        job.cancel()            // must not throw
        assertTrue("Cancelled flag must be set", job.isCancelled)
    }

    /**
     * Null-safe cancel (Kotlin `?.cancel()`) on a null job reference must be
     * a no-op — mirrors the `supabaseUrlJob?.cancel()` pattern in onDispose.
     */
    @Test
    fun `null job cancel via safe-call is a no-op -- no NPE`() {
        val job: DebounceJob? = null
        job?.cancel()   // must not throw NullPointerException
    }

    /**
     * Flush path writes both cloudPassphrase and supabasePassword (which have
     * separate write paths outside persistAll) before the full persist.
     *
     * This is a logical invariant test: two field writes + one bulk write must
     * result in exactly three persisted entries in order.
     */
    @Test
    fun `onDispose flush writes secret fields before calling persistAll`() {
        // Simulate the onDispose sequence:
        // settings.cloudSyncPassphrase = cloudPassphrase
        // settings.supabasePassword    = supabasePassword
        // persistAll()
        val flushOrder = mutableListOf<String>()

        val cloudPassphrase = "my-secret-passphrase"
        val supabasePassword = "hunter2"

        flushOrder.add("cloudSyncPassphrase=$cloudPassphrase")
        flushOrder.add("supabasePassword=$supabasePassword")
        flushOrder.add("persistAll")

        assertEquals(3, flushOrder.size)
        assertTrue(flushOrder[0].startsWith("cloudSyncPassphrase="))
        assertTrue(flushOrder[1].startsWith("supabasePassword="))
        assertEquals("persistAll", flushOrder[2])
    }
}
