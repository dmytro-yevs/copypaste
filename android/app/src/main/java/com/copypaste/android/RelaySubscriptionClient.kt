package com.copypaste.android

import android.util.Base64
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.security.SecureRandom
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

/**
 * Relay SSE subscription client — the THIRD independent sync receive transport
 * on Android, alongside P2P (mTLS) and Supabase Realtime (WebSocket).
 *
 * Mirrors [SupabaseRealtimeClient]'s structure for consistency:
 *   - one reconnect loop launched on the FGS scope ([start] / [close])
 *   - exponential backoff 1 s → 60 s with ±20% jitter ([reconnectDelayMs])
 *   - cursor-based at-least-once resume; dedup via the shared LWW item_id path
 *
 * ## Wire contract (issue #26, relay server side shipped)
 *   - `GET /devices/{deviceId}/subscribe?since=<wall>&since_id=<id>`
 *   - `Authorization: Bearer <relayToken>` (server-issued; 401 → re-register)
 *   - On connect: backfills items past the `(wall_time, id)` cursor, then streams.
 *   - `event: item` / `id: <inbox id>` /
 *     `data: {"id","content_type","content_b64","wall_time"}`; 25 s keepalive.
 *
 * ## Independence (3-path architecture)
 * Gated ONLY on a configured `relayUrl` + sync being enabled — NOT on Supabase
 * being configured. The relay can be the sole transport. (Decryption still needs
 * a cross-device sync key, which today comes from the Supabase/QR provisioning;
 * absent a key, items simply fail to decrypt and are skipped — the stream itself
 * still runs independently.)
 *
 * ## Security
 * - Never logs the relay token, the registration key, ciphertext, or plaintext.
 */
class RelaySubscriptionClient(
    private val settings: Settings,
    private val syncManager: SyncManager,
    private val repository: ClipboardRepository,
    private val scope: CoroutineScope,
) {
    companion object {
        private const val TAG = "RelaySubscriptionClient"

        /** Minimum reconnect delay (1 s — mirrors SupabaseRealtimeClient). */
        private const val BACKOFF_BASE_MS = 1_000L

        /** Maximum reconnect delay (60 s). */
        private const val BACKOFF_MAX_MS = 60_000L

        /**
         * Per-read socket timeout for the SSE stream. The relay emits a keepalive
         * comment every 25 s, so a read gap longer than this means the connection
         * is dead and we reconnect. 60 s leaves comfortable margin over 25 s.
         */
        private const val SSE_READ_TIMEOUT_MS = 60_000L

        /**
         * Catch-up poll backstop interval. SSE is the primary push path; this
         * cheap periodic poll heals any item missed while the stream was down
         * (Doze, OEM kill, proxy drop). Dedup via the shared item_id LWW path
         * makes a doubly-delivered item a silent no-op.
         */
        private const val CATCHUP_POLL_INTERVAL_MS = 300_000L // 5 min

        /** Idle wait when relay is unconfigured/disabled before re-checking. */
        private const val DISABLED_RECHECK_MS = 10_000L

        /**
         * Exponential backoff `1s * 2^(attempt-1)` with ±20% jitter, clamped to
         * 60 s. Jitter de-synchronizes reconnects across devices (no thundering
         * herd). Identical policy to [SupabaseRealtimeClient.reconnectDelayMs].
         */
        fun reconnectDelayMs(attempt: Int): Long {
            if (attempt <= 0) return BACKOFF_BASE_MS
            val exp = (attempt - 1).coerceAtMost(30)
            val base = BACKOFF_BASE_MS.toDouble() * (1L shl exp).toDouble()
            val jitterFactor = 0.8 + kotlin.random.Random.nextDouble() * 0.4 // 0.8..1.2
            val jittered = base * jitterFactor
            return if (jittered >= BACKOFF_MAX_MS.toDouble()) BACKOFF_MAX_MS else jittered.toLong()
        }
    }

    private val _connected = AtomicBoolean(false)

    /** True while an SSE stream is actively open (for diagnostics). */
    val isConnected: Boolean get() = _connected.get()

    private val reconnectAttempt = AtomicInteger(0)

    private var connectJob: Job? = null
    private var catchUpJob: Job? = null

    /**
     * Start the relay SSE loop. Idempotent — safe to call when already running.
     * Owns its own reconnect loop inside [scope]; cancellation via [close].
     */
    fun start() {
        if (connectJob?.isActive == true) return
        connectJob = scope.launch(Dispatchers.IO) {
            Log.i(TAG, "RelaySubscriptionClient starting")
            while (isActive) {
                if (!settings.syncEnabled || !settings.isRelayConfigured) {
                    _connected.set(false)
                    delay(DISABLED_RECHECK_MS)
                    continue
                }
                subscribeOnce()
                if (!isActive) break
                val backoff = reconnectDelayMs(reconnectAttempt.incrementAndGet())
                Log.i(TAG, "relay SSE reconnect in ${backoff}ms (attempt ${reconnectAttempt.get()})")
                delay(backoff)
            }
            Log.i(TAG, "RelaySubscriptionClient stopped")
        }
        startCatchUpBackstop()
    }

    /** Cancel the loops and mark disconnected. Called by the FGS from onDestroy. */
    fun close() {
        catchUpJob?.cancel()
        catchUpJob = null
        connectJob?.cancel()
        connectJob = null
        _connected.set(false)
    }

    /**
     * Open one SSE stream and ingest items until it ends or drops. Lazily
     * registers this device with the relay (and persists the server-issued token)
     * when no valid token is cached. Returns when the stream closes; the caller
     * applies backoff before the next attempt.
     */
    private suspend fun subscribeOnce() {
        val relayUrl = settings.relayUrl
        val client = RelayClient(relayUrl)

        val token = ensureRelayToken(client, relayUrl) ?: run {
            Log.w(TAG, "relay SSE: no token (registration failed) — will retry")
            return
        }

        Log.i(TAG, "relay SSE: connecting (device=${settings.deviceId.take(8)}…)")
        val status = client.subscribe(
            deviceId = settings.deviceId,
            token = token,
            sinceWallTime = settings.lastRelaySubscribeWallTime,
            sinceId = settings.lastRelaySubscribeId,
            readTimeoutMs = SSE_READ_TIMEOUT_MS,
            shouldContinue = { scope.isActive },
            onItem = { item -> ingestAndAdvance(item) },
        )

        when (status) {
            200 -> {
                // Clean stream that ended (server close / EOF): reset backoff so
                // a healthy reconnect is immediate-ish.
                reconnectAttempt.set(0)
            }
            401 -> {
                // Token rejected (expired/revoked) — drop it so we re-register.
                Log.w(TAG, "relay SSE: 401 — clearing cached token to re-register")
                settings.relayToken = ""
                settings.relayTokenUrl = ""
            }
            else -> {
                // Transport error or non-2xx — leave backoff to the caller.
            }
        }
        _connected.set(false)
    }

    /**
     * Ingest one SSE item via the shared relay decrypt + LWW path, then advance
     * the relay cursor only when the row is strictly forward of the watermark.
     *
     * Runs INLINE on the subscribe reader coroutine (a `suspend` callback), so
     * the reader does not pull the next frame until this row is durably stored —
     * the cursor only moves forward after storage, giving at-least-once delivery.
     * Never throws — [SyncManager.ingestRelaySseItem] handles its own errors.
     */
    private suspend fun ingestAndAdvance(item: RelayClient.SseItem) {
        val ok = try {
            syncManager.ingestRelaySseItem(item, repository)
        } catch (e: Exception) {
            Log.w(TAG, "relay SSE ingest threw: ${e.message}")
            false
        }
        // Advance the cursor for EVERY processed row (even a decrypt-skip or a
        // dup) so the stream does not redeliver it forever. Forward-only — never
        // backward — matching the poll drain logic.
        advanceCursor(item.wallTime, item.id)
        if (ok) reconnectAttempt.set(0)
        _connected.set(true)
    }

    /** Advance the persisted `(wall_time, id)` relay cursor monotonically. */
    @Synchronized
    private fun advanceCursor(wallTime: Long, id: Long) {
        val curW = settings.lastRelaySubscribeWallTime
        val curId = settings.lastRelaySubscribeId
        if (wallTime > curW || (wallTime == curW && id > curId)) {
            settings.lastRelaySubscribeWallTime = wallTime
            settings.lastRelaySubscribeId = id
        }
    }

    /**
     * Return a valid relay bearer token, registering this device with the relay
     * (and persisting the server-issued token) on a miss or when the cached token
     * was issued for a different relay URL. Returns null if registration fails.
     *
     * The relay issues a random token (not derivable), so we register once and
     * cache it. The 32-byte registration public key is a stable per-install random
     * value (see [Settings.relayRegistrationKeyB64]) — the relay only stores it.
     */
    private suspend fun ensureRelayToken(client: RelayClient, relayUrl: String): String? {
        val cached = settings.relayToken
        if (cached.isNotBlank() && settings.relayTokenUrl == relayUrl) return cached

        val pubKeyB64 = ensureRegistrationKey()
        val device = client.registerDevice(settings.deviceId, pubKeyB64) ?: return null
        settings.relayToken = device.token
        settings.relayTokenUrl = relayUrl
        Log.i(TAG, "relay SSE: registered device, token cached")
        return device.token
    }

    /** Mint (once) and return the base64 32-byte relay registration key. */
    private fun ensureRegistrationKey(): String {
        settings.relayRegistrationKeyB64.takeIf { it.isNotBlank() }?.let { return it }
        val bytes = ByteArray(32).also { SecureRandom().nextBytes(it) }
        // NO_WRAP: the relay decodes standard base64; a trailing newline would
        // corrupt the value on some servers.
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        settings.relayRegistrationKeyB64 = b64
        return b64
    }

    /**
     * Periodic catch-up poll backstop. Heals items missed while the SSE stream
     * was down. Reuses the same SSE ingest + cursor path, so a doubly-delivered
     * item is a silent LWW no-op. Cheap: one short-lived GET every 5 minutes.
     */
    private fun startCatchUpBackstop() {
        if (catchUpJob?.isActive == true) return
        catchUpJob = scope.launch(Dispatchers.IO) {
            while (isActive) {
                delay(CATCHUP_POLL_INTERVAL_MS)
                if (!isActive) break
                if (!settings.syncEnabled || !settings.isRelayConfigured) continue
                runCatchUpPoll()
            }
        }
    }

    /**
     * One catch-up poll: pull relay items past the cursor and ingest them via the
     * same path as the SSE stream. Best-effort — failures are logged, never fatal.
     */
    private suspend fun runCatchUpPoll() {
        val relayUrl = settings.relayUrl
        val client = RelayClient(relayUrl)
        val token = ensureRelayToken(client, relayUrl) ?: return
        try {
            val items = client.pollSseBacklog(
                deviceId = settings.deviceId,
                token = token,
                sinceWallTime = settings.lastRelaySubscribeWallTime,
                sinceId = settings.lastRelaySubscribeId,
            )
            if (items.isEmpty()) return
            var stored = 0
            for (item in items) {
                val ok = syncManager.ingestRelaySseItem(item, repository)
                advanceCursor(item.wallTime, item.id)
                if (ok) stored++
            }
            if (stored > 0) Log.d(TAG, "relay catch-up poll: stored $stored item(s)")
        } catch (e: Exception) {
            Log.w(TAG, "relay catch-up poll failed: ${e.message}")
        }
    }
}
