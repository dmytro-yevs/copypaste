package com.copypaste.app

import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.*
import org.junit.Test

class ForegroundCaptureOwnerTest {
    private class NativeHost {
        private val lock = Any()
        private var reads = 0
        private var revoked = false
        private val completions = mutableListOf<(Boolean) -> Unit>()
        var failDrain = false
        fun read(action: () -> Unit): Boolean {
            synchronized(lock) {
                if (revoked) return false
                reads += 1
            }
            try { action() } finally {
                val callbacks = synchronized(lock) {
                    reads -= 1
                    if (revoked && reads == 0) takeCompletions() else emptyList()
                }
                callbacks.forEach { it(!failDrain) }
            }
            return true
        }
        fun close(completion: (Boolean) -> Unit) {
            val callbacks = synchronized(lock) {
                revoked = true
                completions.add(completion)
                if (reads == 0) takeCompletions() else emptyList()
            }
            callbacks.forEach { it(!failDrain) }
        }
        private fun takeCompletions() = completions.toList().also { completions.clear() }
    }

    @Test
    fun replacementActivityDisableWaitsForPredecessorAndBlocksConcurrentAcquisition() {
        val opened = AtomicInteger()
        val owner = ForegroundCaptureOwner(
            open = { opened.incrementAndGet(); NativeHost() },
            close = NativeHost::close,
        )
        val predecessor = owner.acquire(null)!!
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val reader = Thread {
            assertTrue(predecessor.read { entered.countDown(); release.await() })
        }.also(Thread::start)
        entered.await()
        owner.retire(predecessor)
        val replacement = owner.acquire(null)!!
        val firstResults = mutableListOf<Boolean>()
        val repeatedResults = mutableListOf<Boolean>()
        owner.disable(firstResults::add)
        owner.disable(repeatedResults::add)
        assertTrue(firstResults.isEmpty())
        assertTrue(repeatedResults.isEmpty())
        assertNull(owner.acquire(replacement))
        assertNull(owner.acquire(null))
        assertEquals(2, opened.get())
        assertFalse(replacement.read { fail("Replacement was not revoked") })
        release.countDown()
        reader.join()
        assertEquals(listOf(true), firstResults)
        assertEquals(listOf(true), repeatedResults)
        assertNotNull(owner.acquire(null))
    }

    @Test
    fun disableWaitsForOpeningHostAndRevokesItBeforeAcknowledgement() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val native = NativeHost()
        val owner = ForegroundCaptureOwner(
            open = { entered.countDown(); release.await(); native },
            close = NativeHost::close,
        )
        var acquired: NativeHost? = null
        val acquisition = Thread { acquired = owner.acquire(null) }.also(Thread::start)
        entered.await()
        val results = mutableListOf<Boolean>()
        owner.disable(results::add)
        assertTrue(results.isEmpty())
        assertNull(owner.acquire(null))
        release.countDown()
        acquisition.join()
        assertNull(acquired)
        assertEquals(listOf(true), results)
        assertFalse(native.read { fail("Opening host escaped disable") })
    }

    @Test
    fun refusedEnableIsFalseAndFreshEligibleEventCanRecoverWithoutTimer() {
        var admissionAvailable = false
        var opens = 0
        var reads = 0
        val owner = ForegroundCaptureOwner(
            open = { opens += 1; if (admissionAvailable) NativeHost() else null },
            close = NativeHost::close,
        )
        val refused = owner.acquire(null)
        assertFalse(refused != null)
        assertEquals(1, opens)
        admissionAvailable = true
        val recovered = owner.acquire(refused)
        assertNotNull(recovered)
        assertTrue(recovered!!.read { reads += 1 })
        assertEquals(1, reads)
        assertSame(recovered, owner.acquire(recovered))
        assertEquals(2, opens)
    }

    @Test
    fun completionFailureCannotLoseAnotherStopOrReopenDuringPublication() {
        val owner = ForegroundCaptureOwner(open = { NativeHost() }, close = NativeHost::close)
        val host = owner.acquire(null)!!
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val reader = Thread { host.read { entered.countDown(); release.await() } }.also(Thread::start)
        entered.await()
        val first = mutableListOf<Boolean>()
        val second = mutableListOf<Boolean>()
        var admittedDuringAcknowledgement = true
        owner.disable { result ->
            first.add(result)
            admittedDuringAcknowledgement = owner.acquire(null) != null
            throw IllegalStateException("Reply failed")
        }
        owner.disable(second::add)
        release.countDown()
        reader.join()
        assertEquals(listOf(true), first)
        assertEquals(listOf(true), second)
        assertFalse(admittedDuringAcknowledgement)
        assertNotNull(owner.acquire(null))
    }

    @Test
    fun failedDrainCannotBecomeSuccessOrReopenForegroundAdmission() {
        val owner = ForegroundCaptureOwner(open = { NativeHost() }, close = NativeHost::close)
        owner.acquire(null)!!.failDrain = true
        val results = mutableListOf<Boolean>()
        owner.disable(results::add)
        assertEquals(listOf(false), results)
        assertNull(owner.acquire(null))
        owner.disable(results::add)
        assertEquals(listOf(false, false), results)
    }
}
