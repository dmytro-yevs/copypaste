package com.copypaste.app

import java.io.InputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.*
import org.junit.Test

class CaptureResourceAdmissionTest {
    @Test fun byteBudgetRejectsOverloadAndReleasesExactlyOnce() {
        val budget = CaptureBytes(32L * 1024 * 1024)
        val lease = budget.reserve(4L * 1024 * 1024 * 8)!!
        assertNull(budget.reserve(1))
        lease.close()
        val next = budget.reserve(32L * 1024 * 1024)!!
        lease.close()
        assertNull(budget.reserve(1))
        next.close()
    }
    @Test fun inputCapacityRemainsReservedUntilBlockingCloseCompletes() {
        val slots = CaptureSlots(1)
        val entered = CountDownLatch(1)
        val unblock = CountDownLatch(1)
        val closes = AtomicInteger()
        val input = CaptureInput(object : InputStream() {
            override fun read() = -1
            override fun close() { closes.incrementAndGet(); entered.countDown(); unblock.await() }
        }, slots.reserve()!!)
        val thread = Thread { input.close() }.apply { start() }
        assertTrue(entered.await(2, TimeUnit.SECONDS))
        input.close()
        assertNull(slots.reserve())
        unblock.countDown()
        thread.join(2000)
        assertEquals(1, closes.get())
        slots.reserve()!!.close()
    }

    @Test fun blockedHeavyAdmissionWakesOnCancellation() {
        val admission = CaptureHeavyAdmission()
        assertTrue(admission.acquire { true })
        val active = java.util.concurrent.atomic.AtomicBoolean(true)
        val waiting = CountDownLatch(1)
        var accepted = true
        val thread = Thread {
            waiting.countDown()
            accepted = admission.acquire(active::get)
        }.apply { start() }
        assertTrue(waiting.await(2, TimeUnit.SECONDS))
        active.set(false)
        admission.wake()
        thread.join(2000)
        assertFalse(thread.isAlive)
        assertFalse(accepted)
        admission.release()
        assertTrue(admission.acquire { true })
        admission.release()
    }

    @Test fun blockedInputAdmissionWakesOnCancellation() {
        val slots = CaptureSlots(1)
        val reserved = slots.reserve()!!
        val active = java.util.concurrent.atomic.AtomicBoolean(true)
        val waiting = CountDownLatch(1)
        var accepted: CaptureSlots.Lease? = null
        val thread = Thread {
            waiting.countDown()
            accepted = slots.await(active::get)
        }.apply { start() }
        assertTrue(waiting.await(2, TimeUnit.SECONDS))
        active.set(false)
        slots.wake()
        thread.join(2000)
        assertFalse(thread.isAlive)
        assertNull(accepted)
        reserved.close()
        slots.reserve()!!.close()
    }

    @Test fun cleanupLaneAcceptsAllReservedWorkWhileWorkersBlock() {
        val executor = captureCleanupExecutor(3)
        val entered = CountDownLatch(2)
        val release = CountDownLatch(1)
        val completed = CountDownLatch(5)
        try {
            repeat(2) { executor.execute { entered.countDown(); release.await(); completed.countDown() } }
            assertTrue(entered.await(2, TimeUnit.SECONDS))
            repeat(3) { executor.execute { completed.countDown() } }
        } finally { release.countDown() }
        assertTrue(completed.await(2, TimeUnit.SECONDS))
        executor.shutdown()
    }
}
