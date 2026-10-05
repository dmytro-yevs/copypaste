package com.copypaste.app

import android.content.Context
import java.io.IOException
import java.io.InputStream
import java.util.ArrayDeque
import java.util.concurrent.CountDownLatch
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.*
import org.junit.Test

class AndroidClipboardPendingTest {
    private class NativeRuntime : AndroidClipboardReader.PendingRuntime {
        val tasks = ArrayDeque<Runnable>()
        val main = ArrayDeque<() -> Unit>()
        val closes = ArrayDeque<InputStream>()
        val abandoned = mutableListOf<Long>()
        val scopes = AtomicInteger()
        val exitedScopes = AtomicInteger()
        var feedback = 0
        var hostCloses = 0
        var drainAcknowledgements = 0
        var beforeEnqueue: () -> Unit = {}
        var rejectEnqueue = false
        var refuseScope = false
        var throwCompletion = false
        var throwFeedback = false
        var cancellation: () -> Unit = {}
        override fun enqueue(task: Runnable) {
            if (rejectEnqueue) throw RejectedExecutionException()
            beforeEnqueue()
            tasks.add(task)
        }
        override fun remove(task: Runnable) { tasks.remove(task) }
        override fun postMain(action: () -> Unit) { main.add(action) }
        override fun closeInput(input: InputStream) { closes.add(input) }
        override fun abandon(token: Long) { abandoned.add(token); cancellation() }
        override fun scoped(token: Long, completion: Boolean, contentType: String, callback: CaptureCallback): Boolean {
            if (token <= 0 || refuseScope) return false
            scopes.incrementAndGet()
            try {
                if (completion && throwCompletion) throw IllegalStateException("JNI transport failure")
                return try { callback.run(4096); true } catch (_: Exception) { false }
            } finally {
                scopes.decrementAndGet()
                exitedScopes.incrementAndGet()
                if (hostCloses > 0 && scopes.get() == 0) drainAcknowledgements = 1
            }
        }
        override fun onCaptured(context: Context?) {
            if (throwFeedback) throw IllegalStateException("JNI feedback failure")
            feedback += 1
        }
        override fun closeHost(host: AndroidClipboardReader.Host) {
            host.closed.set(true)
            hostCloses += 1
            if (scopes.get() == 0) drainAcknowledgements = 1
        }
        fun runWorker() { tasks.removeFirst().run() }
        fun runMain() { while (main.isNotEmpty()) main.removeFirst().invoke() }
    }

    private data class Fixture(
        val host: AndroidClipboardReader.Host,
        val runtime: NativeRuntime,
        val pending: AndroidClipboardReader.Pending,
        val results: MutableList<Boolean>,
    )
    private fun fixture(attach: Boolean = true): Fixture {
        val host = AndroidClipboardReader.Host(11)
        val runtime = NativeRuntime()
        val results = mutableListOf<Boolean>()
        val pending = AndroidClipboardReader.Pending(host, results::add, runtime)
        host.pending.add(pending)
        runtime.cancellation = pending::cancel
        if (attach) pending.attach(12)
        return Fixture(host, runtime, pending, results)
    }
    private fun assertFailedOnce(f: Fixture) {
        f.runtime.runMain()
        assertEquals(listOf(false), f.results)
        assertEquals(listOf(12L), f.runtime.abandoned)
        assertEquals(1, f.runtime.hostCloses)
        assertEquals(0, f.runtime.feedback)
        assertEquals(0, f.runtime.scopes.get())
        assertTrue(f.host.pending.isEmpty())
        assertEquals(0L, f.pending.token)
    }

    @Test
    fun cancellationBeforeAttachAndAfterEnqueueNeverRunsRetainedWork() {
        for (beforeAttach in listOf(true, false)) {
            val f = fixture(attach = !beforeAttach)
            var reads = 0
            if (beforeAttach) { f.pending.cancel(); f.pending.attach(12) }
            f.pending.enqueue(AndroidClipboardReader.Snapshot(text = "retained")) { reads += 1; true }
            f.pending.cancel()
            f.pending.cancel()
            assertTrue(f.runtime.tasks.isEmpty())
            assertFailedOnce(f)
            assertEquals(0, reads)
        }
    }

    @Test
    fun cancellationDuringEnqueueCannotRestorePayloadOrPublishTwice() {
        val f = fixture()
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        f.runtime.beforeEnqueue = { entered.countDown(); release.await() }
        var reads = 0
        val enqueuer = Thread {
            f.pending.enqueue(AndroidClipboardReader.Snapshot(text = "retained")) { reads += 1; true }
        }.also(Thread::start)
        entered.await()
        f.pending.cancel()
        release.countDown()
        enqueuer.join()
        f.runtime.runWorker()
        assertFailedOnce(f)
        assertEquals(0, reads)
    }

    @Test
    fun enqueueRefusalAndProviderOrDecoderExceptionsReleasePendingExactlyOnce() {
        val rejected = fixture()
        rejected.runtime.rejectEnqueue = true
        rejected.pending.enqueue(AndroidClipboardReader.Snapshot(text = "queued")) { true }
        assertFailedOnce(rejected)
        for (failure in listOf(IOException("provider revoked"), IllegalArgumentException("decoder failed"))) {
            val f = fixture()
            f.pending.enqueue(AndroidClipboardReader.Snapshot(text = "payload")) {
                f.pending.read("application/pdf") { throw failure }
            }
            f.runtime.runWorker()
            assertFailedOnce(f)
            assertEquals(1, f.runtime.exitedScopes.get())
        }
    }

    @Test
    fun cancellationAfterCommitBeforeMainPublicationAndJniFailureEmitNoSuccess() {
        for (failure in listOf("cancel", "refuse", "jni", "feedback")) {
            val f = fixture()
            f.pending.enqueue(AndroidClipboardReader.Snapshot(text = "saved")) { true }
            f.runtime.runWorker()
            when (failure) {
                "cancel" -> f.pending.cancel()
                "refuse" -> f.runtime.refuseScope = true
                "jni" -> f.runtime.throwCompletion = true
                "feedback" -> f.runtime.throwFeedback = true
            }
            assertFailedOnce(f)
        }
    }

    @Test
    fun successfulCallbackThatThrowsDoesNotPublishASecondFalseResult() {
        val host = AndroidClipboardReader.Host(11)
        val runtime = NativeRuntime()
        val results = mutableListOf<Boolean>()
        val pending = AndroidClipboardReader.Pending(host, { saved ->
            results.add(saved)
            throw IllegalStateException("UI callback failed")
        }, runtime)
        host.pending.add(pending)
        runtime.cancellation = pending::cancel
        pending.attach(12)
        pending.enqueue(AndroidClipboardReader.Snapshot(text = "saved")) { true }
        runtime.runWorker()
        runtime.runMain()
        assertEquals(listOf(true), results)
        assertEquals(1, runtime.feedback)
        assertEquals(1, runtime.hostCloses)
        assertEquals(listOf(12L), runtime.abandoned)
        assertEquals(0, runtime.scopes.get())
    }

    @Test
    fun blockedProviderCloseIsCancellationAttemptAndNotNativeDrainProof() {
        val f = fixture()
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val closeAttempts = AtomicInteger()
        val input = object : InputStream() {
            override fun read(): Int = error("Use bulk read")
            override fun read(bytes: ByteArray, offset: Int, length: Int): Int {
                entered.countDown()
                release.await()
                return -1
            }
            override fun close() { closeAttempts.incrementAndGet() }
        }
        f.pending.enqueue(AndroidClipboardReader.Snapshot(text = "provider")) {
            var payload: ByteArray? = null
            val read = f.pending.read("application/pdf") {
                f.pending.input(input)
                try { payload = AndroidClipboardReader.readBounded(input, 4096, f.pending) }
                finally { f.pending.releaseInput(input); input.close() }
            }
            read && payload != null
        }
        val worker = Thread { f.runtime.runWorker() }.also(Thread::start)
        entered.await()
        f.pending.cancel()
        f.runtime.closes.removeFirst().close()
        f.runtime.runMain()
        assertEquals(1, closeAttempts.get())
        assertEquals(1, f.runtime.scopes.get())
        assertEquals(0, f.runtime.drainAcknowledgements)
        assertEquals(listOf(false), f.results)
        release.countDown()
        worker.join()
        f.runtime.runMain()
        assertEquals(0, f.runtime.scopes.get())
        assertEquals(1, f.runtime.drainAcknowledgements)
        assertEquals(listOf(false), f.results)
        assertEquals(listOf(12L), f.runtime.abandoned)
        assertTrue(f.host.pending.isEmpty())
    }
}
