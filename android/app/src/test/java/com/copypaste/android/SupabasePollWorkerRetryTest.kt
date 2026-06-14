package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.IOException
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import javax.net.ssl.SSLException

/**
 * Unit tests for [SupabasePollWorker.shouldRetry] — the retry classification that
 * decides whether a failed Supabase poll re-runs via WorkManager backoff or is
 * dropped until the 15-min periodic cadence (CopyPaste-z934).
 */
class SupabasePollWorkerRetryTest {

    // ── Retryable (transient network) — all IOException subtypes ──

    @Test fun unknownHostRetries() =
        assertTrue(SupabasePollWorker.shouldRetry(UnknownHostException("no dns")))

    @Test fun socketTimeoutRetries() =
        assertTrue(SupabasePollWorker.shouldRetry(SocketTimeoutException("timed out")))

    /** The regression case: a plain IOException used to be swallowed as success. */
    @Test fun plainIoExceptionRetries() =
        assertTrue(SupabasePollWorker.shouldRetry(IOException("connection reset")))

    @Test fun sslExceptionRetries() =
        assertTrue(SupabasePollWorker.shouldRetry(SSLException("handshake_failure")))

    @Test fun eofExceptionRetries() =
        assertTrue(SupabasePollWorker.shouldRetry(java.io.EOFException("premature eof")))

    // ── Non-retryable (logic / config / auth) ──

    @Test fun illegalStateDoesNotRetry() =
        assertFalse(SupabasePollWorker.shouldRetry(IllegalStateException("bad config")))

    @Test fun illegalArgumentDoesNotRetry() =
        assertFalse(SupabasePollWorker.shouldRetry(IllegalArgumentException("bad cursor")))

    @Test fun runtimeExceptionDoesNotRetry() =
        assertFalse(SupabasePollWorker.shouldRetry(RuntimeException("auth 401")))
}
