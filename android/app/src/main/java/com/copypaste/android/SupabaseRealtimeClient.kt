package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

/**
 * Supabase Realtime WebSocket client for Android.
 *
 * Mirrors the macOS daemon's `copypaste-supabase/src/realtime.rs` wire
 * protocol byte-for-byte:
 *   - WS URL: `wss://{project}.supabase.co/realtime/v1/websocket?apikey={ANON}&vsn=1.0.0`
 *   - Frame format: 5-element JSON array `[join_ref, ref, topic, event, payload]`
 *   - Topic: `realtime:clipboard_items`
 *   - Join: `phx_join` with `{config:{access_token:JWT,
 *       postgres_changes:[{event:"*",schema:"public",table:"clipboard_items",
 *       filter:"user_id=eq.<UUID>"}]}}`
 *   - Heartbeat every 30s: `[null,"<ref>","phoenix","heartbeat",{}]`
 *   - Reconnect: exponential backoff 1s→60s + jitter on phx_error/phx_close/disconnect
 *
 * ## Usage (FGS lifetime)
 * ```
 * val ws = SupabaseRealtimeClient(settings, syncManager, repository, scope)
 * ws.start()          // in onStartCommand
 * ws.isConnected      // poll-interval gate
 * ws.close()          // in onDestroy
 * ```
 *
 * ## Security
 * - Never logs the access_token or payload content.
 * - Redacts frame payloads in debug logs (length + prefix only).
 */
class SupabaseRealtimeClient(
    private val settings: Settings,
    private val syncManager: SyncManager,
    private val repository: ClipboardRepository,
    private val scope: CoroutineScope,
    /**
     * Called with the text of a NEWLY stored text clip delivered via WS push.
     *
     * WS push delivers ONE item at a time (live single-item delta), so we
     * apply it immediately — there is no batch to reduce here. The same
     * callback type is reused for symmetry with [FgsSyncLoop.onSyncedTextClip].
     *
     * Null (default) means "no auto-apply".
     */
    private val onSyncedTextClip: ((text: String) -> Unit)? = null,
) {
    companion object {
        private const val TAG = "SupabaseRealtimeClient"

        /** Supabase Realtime WS URL template — vsn=1.0.0 is required. */
        private const val WS_PATH = "/realtime/v1/websocket?apikey=%s&vsn=1.0.0"

        /** Channel topic that matches the `clipboard_items` table. */
        private const val TOPIC = "realtime:clipboard_items"

        /** Heartbeat every 30 s (mirrors realtime.rs). */
        private const val HEARTBEAT_INTERVAL_MS = 30_000L

        /** OkHttp ping interval: keeps the socket alive through idle periods. */
        private const val PING_INTERVAL_S = 25L

        /** Minimum reconnect delay (1 s, mirrors realtime.rs initial_backoff). */
        private const val BACKOFF_BASE_MS = 1_000L

        /** Maximum reconnect delay (60 s, mirrors realtime.rs max_backoff). */
        private const val BACKOFF_MAX_MS = 60_000L

        // ── Wire-protocol helpers (public for unit tests) ─────────────────────

        /**
         * Build a Phoenix `phx_join` frame for `realtime:clipboard_items`.
         *
         * Wire format: `[join_ref, ref, topic, "phx_join", payload]`
         * Payload: `{config:{access_token:<JWT>,
         *   postgres_changes:[{event:"*",schema:"public",
         *   table:"clipboard_items",filter:"user_id=eq.<UUID>"}]}}`
         *
         * When [userUuid] is blank the filter is omitted (anon access).
         */
        fun buildJoinFrame(
            joinRef: String,
            ref: String,
            topic: String,
            accessToken: String,
            userUuid: String,
        ): String {
            val pgChanges = JSONArray().apply {
                put(JSONObject().apply {
                    put("event", "*")
                    put("schema", "public")
                    put("table", "clipboard_items")
                    if (userUuid.isNotBlank()) {
                        put("filter", "user_id=eq.$userUuid")
                    }
                })
            }
            val config = JSONObject().apply {
                put("access_token", accessToken)
                put("postgres_changes", pgChanges)
            }
            val payload = JSONObject().apply {
                put("config", config)
            }
            return JSONArray().apply {
                put(joinRef)
                put(ref)
                put(topic)
                put("phx_join")
                put(payload)
            }.toString()
        }

        /**
         * Build a Phoenix heartbeat frame: `[null,"<ref>","phoenix","heartbeat",{}]`.
         */
        fun buildHeartbeatFrame(ref: String): String =
            JSONArray().apply {
                put(JSONObject.NULL)
                put(ref)
                put("phoenix")
                put("heartbeat")
                put(JSONObject())
            }.toString()

        /**
         * Build a `phx_leave` frame to gracefully close the channel.
         */
        fun buildLeaveFrame(joinRef: String, ref: String, topic: String): String =
            JSONArray().apply {
                put(joinRef)
                put(ref)
                put(topic)
                put("phx_leave")
                put(JSONObject())
            }.toString()

        /**
         * Parse a Phoenix wire frame string into a [PhoenixFrame].
         * Returns null for malformed frames (wrong type, wrong element count).
         */
        fun parseFrame(raw: String): PhoenixFrame? {
            return try {
                val arr = JSONArray(raw)
                if (arr.length() < 5) return null
                PhoenixFrame(
                    joinRef = if (arr.isNull(0)) null else arr.optString(0),
                    ref = if (arr.isNull(1)) null else arr.optString(1),
                    topic = arr.getString(2),
                    event = arr.getString(3),
                    payload = arr.optJSONObject(4) ?: JSONObject(),
                )
            } catch (_: JSONException) {
                null
            }
        }

        /**
         * Extract the `record` object from a `postgres_changes` frame payload.
         * Layout: `payload.data.record`.
         * Returns null if the path is absent or the event is not `postgres_changes`.
         */
        fun extractRecord(frame: PhoenixFrame): JSONObject? {
            if (frame.event != "postgres_changes") return null
            return frame.payload
                .optJSONObject("data")
                ?.optJSONObject("record")
        }

        /**
         * Extract the change type string (e.g. "INSERT") from a `postgres_changes`
         * frame. Returns null if absent.
         */
        fun extractChangeType(frame: PhoenixFrame): String? =
            frame.payload.optJSONObject("data")?.optString("type")

        /**
         * Decode the JWT `sub` claim (user UUID) from a GoTrue access token.
         *
         * JWTs are three base64url segments separated by `.`. The middle segment
         * is the claims JSON; we decode it and read `sub`.
         *
         * Returns null for malformed tokens or tokens without a `sub` claim.
         * Never throws.
         */
        fun extractJwtSub(token: String): String? {
            return try {
                val parts = token.split(".")
                if (parts.size < 2) return null
                val decoded = java.util.Base64.getUrlDecoder()
                    .decode(parts[1].padEnd((parts[1].length + 3) / 4 * 4, '='))
                JSONObject(String(decoded, Charsets.UTF_8)).optString("sub")
                    .takeIf { it.isNotBlank() }
            } catch (_: Exception) {
                null
            }
        }

        /**
         * Exponential backoff for WS reconnects: `1s * 2^(attempt-1)` with ±20%
         * random jitter applied, then clamped to 60s. [attempt] >= 1. Mirror of
         * realtime.rs initial_backoff→max_backoff doubling.
         *
         * E6: the jitter (random factor in 0.8..1.2) de-synchronizes reconnects
         * across multiple devices so they do not retry in lockstep (thundering
         * herd) after a shared outage. The 60s clamp is applied AFTER jitter so
         * the delay never meaningfully exceeds BACKOFF_MAX_MS.
         */
        fun reconnectDelayMs(attempt: Int): Long {
            if (attempt <= 0) return BACKOFF_BASE_MS
            val exp = (attempt - 1).coerceAtMost(30)
            val base = BACKOFF_BASE_MS.toDouble() * (1L shl exp).toDouble()
            val jitterFactor = 0.8 + kotlin.random.Random.nextDouble() * 0.4 // 0.8..1.2
            val jittered = base * jitterFactor
            return if (jittered >= BACKOFF_MAX_MS.toDouble()) BACKOFF_MAX_MS else jittered.toLong()
        }

        /** Redact a payload for safe logging: emit only byte-length + 16-byte hex prefix. */
        private fun redact(payload: JSONObject): String {
            val s = payload.toString()
            val take = s.length.coerceAtMost(8)
            val prefix = s.toByteArray(Charsets.UTF_8).take(take)
                .joinToString("") { "%02x".format(it) }
            return "len=${s.length}, prefix=$prefix"
        }
    }

    // ── Parsed Phoenix frame ─────────────────────────────────────────────────

    /** Parsed representation of a Phoenix wire frame. */
    data class PhoenixFrame(
        val joinRef: String?,
        val ref: String?,
        val topic: String,
        val event: String,
        val payload: JSONObject,
    )

    // ── State ────────────────────────────────────────────────────────────────

    private val _connected = AtomicBoolean(false)

    /** True after a successful `phx_reply` join-confirmed event. */
    val isConnected: Boolean get() = _connected.get()

    private val refCounter = AtomicInteger(0)
    private val reconnectAttempt = AtomicInteger(0)

    @Volatile private var activeSocket: WebSocket? = null
    @Volatile private var joinRef: String = "1"

    private var connectJob: Job? = null
    private var heartbeatJob: Job? = null

    // ── OkHttp client — one instance, reused across reconnects ───────────────

    private val httpClient: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .pingInterval(PING_INTERVAL_S, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.MILLISECONDS) // no read timeout on WS
            .connectTimeout(10, TimeUnit.SECONDS)
            .build()
    }

    // ── Lifecycle ────────────────────────────────────────────────────────────

    /**
     * Start the WS connection loop. Idempotent — safe to call when already running.
     */
    fun start() {
        if (connectJob?.isActive == true) return
        connectJob = scope.launch(Dispatchers.IO) {
            Log.i(TAG, "SupabaseRealtimeClient starting")
            while (isActive) {
                if (!settings.syncEnabled ||
                    settings.syncBackend != SyncBackend.SUPABASE ||
                    !settings.isSupabaseConfigured
                ) {
                    delay(10_000L)
                    continue
                }
                connectOnce()
                if (!isActive) break
                val delay = reconnectDelayMs(reconnectAttempt.incrementAndGet())
                Log.i(TAG, "WS reconnect in ${delay}ms (attempt ${reconnectAttempt.get()})")
                delay(delay)
            }
            Log.i(TAG, "SupabaseRealtimeClient stopped")
        }
    }

    /**
     * Close the WebSocket gracefully (send `phx_leave`) and cancel the loops.
     * Called by the FGS from `onDestroy`.
     */
    fun close() {
        heartbeatJob?.cancel()
        heartbeatJob = null
        connectJob?.cancel()
        connectJob = null
        val sock = activeSocket
        if (sock != null) {
            // Send phx_leave before closing for a clean channel exit.
            runCatching {
                sock.send(buildLeaveFrame(joinRef, nextRef(), TOPIC))
            }
            sock.close(1000, "service stopping")
            activeSocket = null
        }
        _connected.set(false)
    }

    // ── Connect + session ────────────────────────────────────────────────────

    private suspend fun connectOnce() {
        val anonKey = settings.supabaseAnonKey
        val baseUrl = settings.supabaseUrl
            .replace("https://", "wss://")
            .replace("http://", "ws://")
        val wsUrl = "$baseUrl${WS_PATH.format(anonKey)}"

        // Resolve the bearer token and user UUID for the join payload.
        val client = SupabaseClient(settings.supabaseUrl, anonKey)
        val bearer = if (settings.hasSupabaseCredentials) {
            SyncManager.cachedOrFreshBearer(
                client, settings.supabaseUrl,
                settings.supabaseEmail, settings.supabasePassword,
            ) ?: run {
                Log.w(TAG, "WS: sign-in failed — cannot join channel")
                return
            }
        } else {
            anonKey
        }
        val userUuid = extractJwtSub(bearer) ?: ""

        Log.i(TAG, "WS: connecting to Supabase Realtime (user=${userUuid.take(8)}…)")

        joinRef = nextRef()
        val request = Request.Builder().url(wsUrl).build()
        val latch = java.util.concurrent.CountDownLatch(1)

        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                Log.i(TAG, "WS: socket opened — sending phx_join")
                activeSocket = webSocket
                val ref = nextRef()
                webSocket.send(
                    buildJoinFrame(
                        joinRef = joinRef,
                        ref = ref,
                        topic = TOPIC,
                        accessToken = bearer,
                        userUuid = userUuid,
                    )
                )
                startHeartbeat(webSocket)
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                val frame = parseFrame(text) ?: return
                handleFrame(frame, webSocket)
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                Log.w(TAG, "WS: failure — ${t.message}")
                // E4: only tear down if this callback belongs to the CURRENT
                // socket. A delayed callback from a previous socket must not
                // null out a newer healthy connection.
                if (activeSocket === webSocket) {
                    _connected.set(false)
                    activeSocket = null
                }
                heartbeatJob?.cancel()
                latch.countDown()
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                Log.i(TAG, "WS: closed (code=$code reason=$reason)")
                // E4: identity-guard — see onFailure above.
                if (activeSocket === webSocket) {
                    _connected.set(false)
                    activeSocket = null
                }
                heartbeatJob?.cancel()
                latch.countDown()
            }
        }

        httpClient.newWebSocket(request, listener)

        // Wait for the socket to close (failure or server close). The latch is
        // counted down from onFailure/onClosed. Teardown is driven by [close],
        // which explicitly closes the socket → triggers onClosed → counts the
        // latch down, unblocking this wait. Coroutine cancellation does NOT
        // interrupt CountDownLatch.await (no InterruptedException is thrown
        // here), so the scope.isActive check is the cancellation escape hatch.
        while (!latch.await(1, TimeUnit.SECONDS)) {
            if (!scope.isActive) break
        }
    }

    // ── Frame dispatch ───────────────────────────────────────────────────────

    private fun handleFrame(frame: PhoenixFrame, webSocket: WebSocket) {
        when (frame.event) {
            "phx_reply" -> {
                val status = frame.payload.optString("status")
                if (status == "ok" && frame.joinRef == joinRef) {
                    Log.i(TAG, "WS: channel joined (topic=${frame.topic})")
                    _connected.set(true)
                    reconnectAttempt.set(0)
                    // Trigger an immediate catch-up poll on join so no rows are
                    // missed in the window before WS events start flowing.
                    scope.launch(Dispatchers.IO) { triggerCatchUpPoll() }
                } else if (status != "ok") {
                    Log.w(TAG, "WS: phx_reply non-ok: ${redact(frame.payload)}")
                }
            }

            "postgres_changes" -> {
                val record = extractRecord(frame) ?: return
                val changeType = extractChangeType(frame)
                if (changeType == "INSERT" || changeType == "UPDATE") {
                    scope.launch(Dispatchers.IO) { ingestWsRow(record) }
                }
                // DELETE rows are ignored — local history is append-only on Android.
            }

            "phx_error" -> {
                Log.w(TAG, "WS: phx_error received — will reconnect")
                _connected.set(false)
                webSocket.close(1011, "phx_error")
            }

            "phx_close" -> {
                Log.i(TAG, "WS: phx_close received")
                _connected.set(false)
            }

            else -> {
                // heartbeat replies, system messages — ignore silently.
            }
        }
    }

    // ── Row ingest from WS push ──────────────────────────────────────────────

    /**
     * Ingest a single row received via WS push.
     *
     * Reuses [SupabaseClient.decodePayloadCt] and [SupabaseClient.decryptRow]
     * — no crypto is duplicated here. The ingest path is deliberately identical
     * to the catch-up poll path in [FgsSyncLoop] (LWW via storeItemWithLww,
     * cursor advancement, lamport observe) so WS and poll are always consistent.
     *
     * The cursor is advanced only for rows strictly newer than the current
     * watermark, matching the poll drain logic.
     */
    private suspend fun ingestWsRow(record: JSONObject) {
        // E3: the row to ingest arrives inline in the WS frame, so we only need
        // the decryption context (client + sync key) — NOT a REST history GET.
        // resolveSyncContext() reuses the same cached JWT / Argon2id sync key /
        // client construction as pollFromSupabase but performs no network poll.
        val ctx = syncManager.resolveSyncContext() ?: return

        // Build a CloudRow from the WS record JSON.
        val id = record.optString("id").takeIf { it.isNotBlank() } ?: return
        val itemId = record.optString("item_id").takeIf { it.isNotBlank() } ?: return
        val payloadCtWire = record.optString("payload_ct").takeIf { it.isNotBlank() } ?: return
        val deviceId = record.optString("device_id", "")
        val lamportTs = record.optLong("lamport_ts", 0L)
        val wallTime = record.optLong("wall_time", 0L)
        val contentType = record.optString("content_type", "text")
        val expiresAt = if (record.isNull("expires_at")) null else record.optLong("expires_at")
        val appBundleId = if (record.isNull("app_bundle_id")) null else record.optString("app_bundle_id")

        // Skip own-device echoes.
        if (deviceId == settings.deviceId) return

        val row = SupabaseClient.CloudRow(
            id = id,
            itemId = itemId,
            contentType = contentType,
            payloadCtWire = payloadCtWire,
            lamportTs = lamportTs,
            wallTime = wallTime,
            expiresAt = expiresAt,
            appBundleId = appBundleId,
            deviceId = deviceId,
        )

        // Advance the Lamport clock for this row (mirrors FgsSyncLoop poll path).
        settings.lamportClock.observe(lamportTs)

        val item = ctx.client.decryptRow(row, ctx.syncKey) ?: run {
            Log.w(TAG, "WS: decryptRow failed for id=$id")
            return
        }

        // Advance the cursor if this row is strictly newer than the watermark.
        // advanceSupabaseCursor holds supabaseCursorLock so this WS push path
        // and the concurrent poll paths (FgsSyncLoop, SupabasePollWorker) all
        // serialise on the same monitor — no advance is lost.
        settings.advanceSupabaseCursor(wallTime, id)

        val isImage = item.contentType == "image" || item.contentType.startsWith("image/")
        val isFile = item.contentType == "file"
        val stored = if (isImage) {
            if (item.plaintext.isEmpty()) {
                false
            } else {
                val storedId = repository.storeItem(
                    plaintext = "[image]",
                    key = settings.encryptionKey,
                    overrideId = item.itemId,
                    contentType = item.contentType,
                    lamportTs = item.lamportTs,
                    wallTimeMs = item.wallTime,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeImageBytes(storedId, item.plaintext)
                    // Generate thumbnail after full-res storage; non-fatal on failure.
                    SyncThumbnailHelper.generateAndStore(item.plaintext) { thumbBytes ->
                        repository.storeThumbnailBytes(storedId, thumbBytes)
                    }
                    true
                } else {
                    false
                }
            }
        } else if (isFile) {
            // File row: store actual bytes so the user can save/copy them.
            // WS path uses DecryptedItem (no file_name/mime columns in the Supabase
            // SELECT); nulls are correct here — file metadata lives in the payload.
            if (item.plaintext.isEmpty()) {
                false
            } else {
                val label = SyncFileHelper.buildFileLabel(null)
                val storedId = repository.storeItem(
                    plaintext = label,
                    key = settings.encryptionKey,
                    overrideId = item.itemId,
                    contentType = item.contentType,
                    lamportTs = item.lamportTs,
                    wallTimeMs = item.wallTime,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeFileBytes(storedId, item.plaintext)
                    repository.storeFileMeta(storedId, null, null)
                    true
                } else {
                    false
                }
            }
        } else {
            val text = item.plaintext.toString(Charsets.UTF_8)
            if (text.isBlank()) false
            else {
                val didStore = repository.storeItemWithLww(
                    plaintext = text,
                    key = settings.encryptionKey,
                    itemId = item.itemId,
                    incomingLamportTs = item.lamportTs,
                    wallTimeMs = item.wallTime,
                    originDeviceId = deviceId,
                )
                // WS delivers one item at a time (live delta, not a bulk batch),
                // so apply it immediately — no reduction needed.
                if (didStore) onSyncedTextClip?.invoke(text)
                didStore
            }
        }

        if (stored) {
            Log.d(TAG, "WS: stored item itemId=${item.itemId.take(8)}… contentType=${item.contentType}")
        }
    }

    /**
     * Trigger a one-shot catch-up poll on WS (re)connect.
     *
     * Fetches any rows missed while the WS was down and stores them using the
     * same LWW / cursor logic as [ingestWsRow] and [FgsSyncLoop].  Without this
     * the poll batch was fetched and then silently discarded — the "catch-up"
     * healed nothing (H2).
     *
     * Cursor advancement mirrors [FgsSyncLoop.poll]: every row (including
     * self-echo and decrypt-failures) advances the watermark before any
     * `continue`, so a batch of own-device rows still moves the cursor forward.
     */
    private suspend fun triggerCatchUpPoll() {
        try {
            val batch = syncManager.pollFromSupabase(
                sinceWallTime = settings.lastSupabasePollWallTime,
                sinceId = settings.lastSupabasePollId,
            ) ?: return
            if (batch.rows.isEmpty()) return
            Log.d(TAG, "WS catch-up: ${batch.rows.size} row(s) — storing")

            var newCount = 0
            var cursorWallTime = settings.lastSupabasePollWallTime
            var cursorId = settings.lastSupabasePollId

            for (row in batch.rows) {
                // Advance cursor for EVERY row before any continue (mirrors FgsSyncLoop).
                if (row.wallTime > cursorWallTime ||
                    (row.wallTime == cursorWallTime && row.id > cursorId)
                ) {
                    cursorWallTime = row.wallTime
                    cursorId = row.id
                }

                // Skip own-device echoes.
                if (row.deviceId == settings.deviceId) continue

                // Advance Lamport clock.
                settings.lamportClock.observe(row.lamportTs)

                val item = batch.client.decryptRow(row, batch.syncKey)
                if (item == null) {
                    Log.w(TAG, "WS catch-up: decryptRow failed for id=${row.id}")
                    continue
                }

                val isImage = item.contentType == "image" || item.contentType.startsWith("image/")
                val isFile = item.contentType == "file"

                val stored = if (isImage) {
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val storedId = repository.storeItem(
                            plaintext = "[image]",
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, item.plaintext)
                            SyncThumbnailHelper.generateAndStore(item.plaintext) { thumbBytes ->
                                repository.storeThumbnailBytes(storedId, thumbBytes)
                            }
                            true
                        } else {
                            false
                        }
                    }
                } else if (isFile) {
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val label = SyncFileHelper.buildFileLabel(null)
                        val storedId = repository.storeItem(
                            plaintext = label,
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeFileBytes(storedId, item.plaintext)
                            repository.storeFileMeta(storedId, null, null)
                            true
                        } else {
                            false
                        }
                    }
                } else {
                    val text = item.plaintext.toString(Charsets.UTF_8)
                    if (text.isBlank()) false
                    else repository.storeItemWithLww(
                        plaintext = text,
                        key = settings.encryptionKey,
                        itemId = item.itemId,
                        incomingLamportTs = item.lamportTs,
                    )
                }
                if (stored) newCount++
            }

            // Persist the advanced cursor after processing the batch.
            // Serialises with FgsSyncLoop and SupabasePollWorker via supabaseCursorLock.
            settings.advanceSupabaseCursor(cursorWallTime, cursorId)

            if (newCount > 0) {
                Log.i(TAG, "WS catch-up: stored $newCount of ${batch.rows.size} row(s)")
            }
        } catch (e: Exception) {
            Log.w(TAG, "WS catch-up poll failed: ${e.message}")
        }
    }

    // ── Heartbeat ────────────────────────────────────────────────────────────

    private fun startHeartbeat(webSocket: WebSocket) {
        heartbeatJob?.cancel()
        heartbeatJob = scope.launch(Dispatchers.IO) {
            while (isActive) {
                delay(HEARTBEAT_INTERVAL_MS)
                if (!isActive) break
                val ref = nextRef()
                val ok = webSocket.send(buildHeartbeatFrame(ref))
                if (!ok) {
                    Log.w(TAG, "WS: heartbeat send failed — socket may be dead")
                    break
                }
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    private fun nextRef(): String = refCounter.incrementAndGet().toString()
}
