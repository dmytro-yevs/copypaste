package com.copypaste.android

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.lang.reflect.Field
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Tests for the lifecycle-bound thumbnail scope fix (CopyPaste-3ox2).
 *
 * The fix replaces unstructured `CoroutineScope(Dispatchers.Default).launch { … }`
 * with `(boundScope ?: fallback).launch(Dispatchers.Default) { … }` in
 * [FgsSyncLoop.storeSyncedItem] and [SyncManager.ingestRelaySseItem]. The two
 * load-bearing properties this guards are:
 *
 *  1. A task launched on the bound scope is CANCELLED when that scope's Job is
 *     cancelled (i.e. when the FGS is destroyed) — proving captured image bytes
 *     can be released instead of leaking on a root scope.
 *  2. When no scope is bound (unit tests / stub mode) the fallback still runs so
 *     direct callers do not crash.
 *
 * [FgsSyncLoop]/[SyncManager] require Android SDK constructors, so the field
 * lifecycle itself is asserted structurally via reflection; the cancellation
 * SEMANTICS are exercised against a real coroutine scope mirroring the exact
 * `(boundScope ?: fallback).launch(…)` expression the production code uses.
 *
 * Pure JVM test — no Android runtime required.
 */
class ScopeCancellationTest {

    // ── Field-contract assertions (structural) ────────────────────────────────

    private fun field(clazz: Class<*>, name: String): Field =
        clazz.getDeclaredField(name).also { it.isAccessible = true }

    @Test
    fun fgsSyncLoop_hasNullableCoroutineScopeField() {
        val f = field(FgsSyncLoop::class.java, "fgsScope")
        assertEquals(
            "fgsScope must be a CoroutineScope",
            "kotlinx.coroutines.CoroutineScope",
            f.type.name,
        )
    }

    @Test
    fun syncManager_hasNullableCoroutineScopeFieldAndBindScope() {
        val f = field(SyncManager::class.java, "thumbnailScope")
        assertEquals(
            "thumbnailScope must be a CoroutineScope",
            "kotlinx.coroutines.CoroutineScope",
            f.type.name,
        )
        val bind = SyncManager::class.java.getDeclaredMethod(
            "bindScope",
            CoroutineScope::class.java,
        )
        assertNotNull("bindScope(CoroutineScope) must exist", bind)
    }

    // ── Cancellation SEMANTICS (behavioural) ──────────────────────────────────

    /**
     * Mirrors the production expression `(boundScope ?: fallback).launch(Default)`.
     * Cancelling the bound scope's Job must cancel the in-flight thumbnail task
     * before it completes — releasing any captured bytes.
     */
    @Test
    fun boundScope_cancellationCancelsInFlightTask() = runBlocking {
        val completed = AtomicBoolean(false)
        val parentJob = Job()
        val boundScope: CoroutineScope? = CoroutineScope(parentJob + Dispatchers.Default)

        val task = (boundScope ?: CoroutineScope(Dispatchers.Default))
            .launch(Dispatchers.Default) {
                // Simulate the decode/compress step that holds captured bytes alive.
                delay(10_000)
                completed.set(true)
            }

        // FGS destroyed → scope cancelled before the task can finish.
        parentJob.cancel()
        task.join()

        assertTrue("task must observe cancellation", task.isCancelled)
        assertFalse("cancelled task must NOT complete its body", completed.get())
    }

    /**
     * When the scope is unbound (null) the `?: fallback` branch must still run the
     * task to completion — the stub-mode / unit-test path.
     */
    @Test
    fun unboundScope_fallbackStillRunsTask() = runBlocking {
        val ran = AtomicBoolean(false)
        val boundScope: CoroutineScope? = null

        val task = (boundScope ?: CoroutineScope(SupervisorJob() + Dispatchers.Default))
            .launch(Dispatchers.Default) {
                ran.set(true)
            }
        task.join()

        assertTrue("fallback scope must execute the task body", ran.get())
        assertFalse("fallback task must complete, not cancel", task.isCancelled)
    }
}
