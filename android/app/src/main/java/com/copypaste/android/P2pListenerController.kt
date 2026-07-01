package com.copypaste.android

import android.app.NotificationManager
import android.content.Context
import android.os.Build
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * CopyPaste-vp63.32: inbound mTLS P2P listener + FGS mDNS discovery +
 * pair-responder poller + mDNS peer watcher — extracted VERBATIM from
 * [ClipboardService]'s instance methods.
 *
 * Owns the LIVE port-poll pure decisions in [FgsDiscoveryPortPoll] and the
 * mutable listener/job state that was previously spread across
 * [ClipboardService] instance fields. One instance is constructed and owned
 * by [ClipboardService] in `onCreate`; [ClipboardService.onStartCommand]
 * forwards start calls and [ClipboardService.onDestroy] calls [shutdown].
 *
 * @param context application context — used only for [postIncomingPairNotification]
 *   and cancelling the pair-request notification. Never held beyond the call.
 * @param scope the service-owned [CoroutineScope] (SupervisorJob) so a failing
 *   child coroutine here does not cancel the capture pipeline or vice versa.
 * @param settings shared [Settings] instance.
 * @param deviceKeyStore device mTLS identity store.
 * @param repository local item store, used to seed the listener's initial
 *   sync-item set.
 * @param fgsSyncLoop used for [FgsSyncLoop.storeSyncedItem] (inbound item
 *   store mapping) and [FgsSyncLoop.signalP2pWake] (opportunistic dial).
 */
class P2pListenerController(
    private val context: Context,
    private val scope: CoroutineScope,
    private val settings: Settings,
    private val deviceKeyStore: DeviceKeyStore,
    private val repository: ClipboardRepository,
    private val fgsSyncLoop: FgsSyncLoop,
) {
    /**
     * Inbound mTLS P2P listener handle (macOS→Android direction). Bound in
     * [startInboundP2pListener] when `syncEnabled && p2pSyncEnabled`, drained by
     * [p2pListenerJob], released in [stopInboundP2pListener]. Null while not running.
     */
    private var p2pListener: P2pListenerHandleInfo? = null

    /** Coroutine draining the listener on the dial cadence. Cancelled in [stopInboundP2pListener]. */
    private var p2pListenerJob: Job? = null

    /**
     * CopyPaste-mip2: coroutine that watches [listDiscovered] and fires
     * [FgsSyncLoop.signalP2pWake] the moment a paired peer is discovered on the LAN.
     * Polls every [MDNS_PEER_WATCH_INTERVAL_MS]. Cancelled in [shutdown].
     * Null when P2P is disabled or the native library is absent.
     */
    private var mdnsPeerWatchJob: Job? = null

    /**
     * Coroutine that polls [pairGetSas] every ~1 s and posts a HIGH-priority
     * notification when this device transitions to `awaiting_sas` with
     * role="responder" (it received an incoming pairing request). Cancelled in
     * [shutdown]. Null when P2P is disabled or the native library is absent.
     */
    private var pairResponderPollJob: Job? = null

    /**
     * HB-2: start LAN discovery for the lifetime of the foreground service.
     *
     * Advertises this device over mDNS (sync port = the live inbound listener
     * port; bootstrap port = the fixed [SAS_BPORT]) and runs the standing
     * SAS-pairing responder so a macOS peer can dial back to pair AT ANY TIME —
     * not only while the Devices screen is open. Uses the persisted device cert
     * (peek, else generate). Idempotent on the native side (a second start while
     * already running is a no-op). All failures are logged and non-fatal: the FGS
     * must never crash because discovery could not start.
     */
    fun startFgsDiscovery() {
        // ydhw: early-exit and diagnosable log when the native library is absent.
        // startDiscovery() is a no-op in stub mode, but the caller cannot
        // distinguish "not started" from "started but found nothing" without this.
        if (!isNativeLibraryLoaded) {
            Log.w(TAG, "startFgsDiscovery: skipped — native library not loaded (isNativeLibraryLoaded=false); discovery will be empty")
            return
        }
        scope.launch {
            try {
                // CopyPaste-44rq.55: getOrCreate() zeroes cert.keyDer before returning;
                // peek() re-fetches the KEK-unwrapped identity (including keyDer) from
                // AndroidKeyStore so the keyDer is available for mTLS. The identity was
                // just stored by getOrCreate(), so peek() is guaranteed non-null here.
                val cert = withContext(Dispatchers.IO) {
                    deviceKeyStore.peek() ?: deviceKeyStore.getOrCreate().let { deviceKeyStore.peek()!! }
                }
                // ydhw: reactive wait for the inbound listener port rather than a
                // hard 3s deadline. startInboundP2pListener binds asynchronously on
                // Dispatchers.IO; advertising syncPort=0 causes macOS peers to see
                // "device unavailable" and fail to dial back. We poll with short
                // backoff until the port is non-zero, with a 10s safety timeout so a
                // listener that failed silently does not block discovery indefinitely.
                val syncPort = withContext(Dispatchers.IO) {
                    val deadlineMs = System.currentTimeMillis() + 10_000L
                    var backoffMs = 20L
                    while (activeListenerPort == 0 && System.currentTimeMillis() < deadlineMs) {
                        delay(backoffMs)
                        backoffMs = (backoffMs * 2).coerceAtMost(250L)
                    }
                    val port = activeListenerPort
                    if (port == 0) {
                        // Listener did not bind within the safety window. Log at WARN
                        // so this is diagnosable: look for P2P listener errors above.
                        Log.w(TAG, "startFgsDiscovery: listener port still 0 after 10 s wait — NOT advertising (would publish syncPort=0 which Mac cannot dial). Discovery deferred until listener binds.")
                    }
                    port
                }
                // ydhw: do not advertise if port is still 0 — a syncPort=0
                // advertisement is worse than no advertisement (Mac dials :0 and fails).
                // The caller will retry startFgsDiscovery if the listener binds later.
                if (syncPort == 0) return@launch
                withContext(Dispatchers.IO) {
                    // ABI 18 (PG-28): collect own WAN address via STUN so the
                    // macOS peer learns a reachable external candidate. Gated
                    // behind the user's collect_public_ip setting (parity with
                    // the macOS daemon). Best-effort: null on failure or opt-out.
                    val ownPublicIp = StunUtils.queryPublicIp(settings.collectPublicIp)
                    startDiscovery(
                        deviceId = cert.deviceId,
                        deviceName = Build.MODEL ?: "Android",
                        syncPort = syncPort,
                        bport = SAS_BPORT,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        // HB-1a (ABI 14): own metadata for the standing responder.
                        deviceModel = Build.MODEL ?: "Android",
                        osVersion = "Android " + Build.VERSION.RELEASE,
                        appVersion = BuildConfig.VERSION_NAME,
                        localIp = lanIpv4Address(),
                        // ABI 18 (PG-28): STUN-derived WAN address.
                        publicIp = ownPublicIp,
                    )
                }
                Log.i(TAG, "FGS discovery started (syncPort=$syncPort, bport=$SAS_BPORT)")
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.w(TAG, "FGS discovery failed to start (${e.javaClass.simpleName}: ${e.message})")
            }
        }
    }

    /**
     * Bind the inbound mTLS P2P listener and launch a coroutine that drains it on
     * the dial cadence, storing each received item via the SAME store-mapping the
     * dialer uses ([FgsSyncLoop.storeSyncedItem]) — LWW dedup on item_id makes a
     * re-receipt (already delivered by the dialer or a previous tick) a no-op.
     *
     * Idempotent: a no-op while [p2pListener] is already bound (sticky restart).
     * All failures are logged and non-fatal — a listener that cannot bind must
     * never crash the foreground service; the Android→macOS dialer still runs.
     */
    fun startInboundP2pListener() {
        if (p2pListener != null) return // already running — idempotent

        // mTLS identity is mandatory; if pairing never generated one there is
        // nothing to listen with (a peer could not authenticate us anyway).
        val cert = deviceKeyStore.peek() ?: run {
            Log.i(TAG, "P2P listener not started: no device cert (never paired?)")
            return
        }

        val key = settings.encryptionKey
        val peers = settings.pairedPeers
        val allowed = peers.map { it.fingerprint }
        val revoked = try {
            listRevokedFingerprints(settings.dbPath, key)
        } catch (e: Exception) {
            Log.w(TAG, "P2P listener: listRevokedFingerprints failed, using empty denylist: ${e.message}")
            emptyList()
        }
        val sessionKeys = peers.map {
            PeerSessionKeyInfo(it.fingerprint, settings.sessionKeyFor(it.fingerprint))
        }

        // C5: localItemsForSync can block for tens–hundreds ms on large history;
        // running it on the FGS main thread risks ANR. Move the fetch and all
        // downstream work onto Dispatchers.IO via the service-owned scope.
        scope.launch(Dispatchers.IO) {
        val localItems = repository.localItemsForSync(key)

        val handle = try {
            startP2pListener(
                listenPort = 0, // OS-assigned free port; read back from actualPort
                certDer = cert.certDer,
                keyDer = cert.keyDer,
                allowedFingerprints = allowed,
                revokedFingerprints = revoked,
                sessionKeys = sessionKeys,
                localItems = localItems,
                deviceId = settings.deviceId,
            )
        } catch (e: Exception) {
            Log.w(TAG, "P2P listener failed to start (${e.javaClass.simpleName}: ${e.message}) — macOS→Android dial-in disabled this session")
            return@launch
        }

        p2pListener = handle
        activeListenerPort = handle.actualPort
        Log.i(TAG, "P2P listener bound on port ${handle.actualPort} (id=${handle.listenerId})")
        // ydhw: reactive discovery retry — if startFgsDiscovery() ran before
        // the port was known (and returned early to avoid advertising port 0),
        // re-trigger it now that activeListenerPort is non-zero. startDiscovery()
        // is idempotent on the native side, so a second call when discovery is
        // already running is a no-op — safe to call unconditionally here.
        startFgsDiscovery()

        p2pListenerJob = scope.launch {
            val listenerId = handle.listenerId
            while (isActive) {
                // Refresh the roster/denylist/session-keys each tick so a pairing
                // change or revocation is honoured without restarting the listener.
                try {
                    val freshPeers = settings.pairedPeers
                    val freshRevoked = try {
                        listRevokedFingerprints(settings.dbPath, settings.encryptionKey)
                    } catch (e: Exception) {
                        Log.w(TAG, "P2P listener: denylist refresh failed: ${e.message}")
                        emptyList()
                    }
                    updateP2pListenerPeers(
                        listenerId = listenerId,
                        allowed = freshPeers.map { it.fingerprint },
                        revoked = freshRevoked,
                        sessionKeys = freshPeers.map {
                            PeerSessionKeyInfo(it.fingerprint, settings.sessionKeyFor(it.fingerprint))
                        },
                    )
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    Log.w(TAG, "P2P listener: updateP2pListenerPeers failed: ${e.message}")
                }

                // Drain received items; store each via the shared mapping. Per-item
                // try/catch so one malformed item does not stall the rest.
                try {
                    val items = pollP2pListener(listenerId)
                    var stored = 0
                    for (item in items) {
                        try {
                            if (fgsSyncLoop.storeSyncedItem(item)) stored += 1
                        } catch (e: CancellationException) {
                            throw e
                        } catch (e: Exception) {
                            Log.w(TAG, "P2P listener: failed to store item ${item.itemId.take(8)}: ${e.message}")
                        }
                    }
                    if (stored > 0) {
                        Log.i(TAG, "P2P listener: stored $stored inbound item(s)")
                    }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    Log.w(TAG, "P2P listener: poll failed: ${e.message}")
                }

                delay(FgsSyncLoop.P2P_DIAL_INTERVAL_MS)
            }
        }
        } // end scope.launch(Dispatchers.IO)
    }

    /**
     * Stop the inbound listener and cancel its drain coroutine. Idempotent and
     * safe to call when the listener was never started. Errors are logged, not
     * thrown — [shutdown] must complete regardless.
     */
    fun stopInboundP2pListener() {
        // HB-2: tear down LAN discovery (mDNS advert + standing SAS responder)
        // alongside the inbound listener. stopDiscovery() is idempotent and
        // tolerates a stop without a completed start. Called here so both the
        // P2P-toggle-off path and shutdown stop advertising.
        try {
            stopDiscovery()
        } catch (e: Exception) {
            Log.w(TAG, "FGS discovery: stop failed (${e.javaClass.simpleName}: ${e.message})")
        }
        p2pListenerJob?.cancel()
        p2pListenerJob = null
        val handle = p2pListener ?: return
        p2pListener = null
        activeListenerPort = 0
        try {
            stopP2pListener(handle.listenerId)
            Log.i(TAG, "P2P listener stopped (id=${handle.listenerId})")
        } catch (e: Exception) {
            Log.w(TAG, "P2P listener: stop failed (${e.javaClass.simpleName}: ${e.message})")
        }
    }

    /**
     * Deliverable 1 — Incoming-pairing notification.
     *
     * Polls [pairGetSas] every ~1 s. When the state machine enters `awaiting_sas`
     * with role="responder" (a peer dialed US), posts a HIGH-priority notification
     * whose tap opens [DevicesActivity]. De-duped: only one notification is posted
     * per pairing session (tracked by a local flag). Clears the notification
     * when the state returns to a non-awaiting state.
     *
     * Idempotent: a no-op when the native library is absent (stub mode). Started
     * once by [ClipboardService.onStartCommand]; cancelled in [shutdown].
     */
    fun startPairResponderPoller() {
        if (pairResponderPollJob?.isActive == true) return
        if (!isNativeLibraryLoaded) return

        pairResponderPollJob = scope.launch {
            var notifPosted = false
            while (isActive) {
                try {
                    val st = withContext(Dispatchers.IO) { pairGetSas() }
                    if (st.state == "awaiting_sas" && st.role == "responder" && !notifPosted) {
                        ServiceNotifications.postIncomingPairNotification(
                            context = context,
                            peerName = st.sas?.let { "" } ?: "", // sas field != peer name; use empty
                        )
                        notifPosted = true
                    } else if (st.state != "awaiting_sas") {
                        if (notifPosted) {
                            // Clear the notification once the pairing is no longer pending.
                            val nm = context.getSystemService(NotificationManager::class.java)
                            nm?.cancel(ServiceNotifications.NOTIF_ID_PAIR_REQUEST)
                            notifPosted = false
                        }
                    }
                } catch (_: CancellationException) {
                    throw CancellationException()
                } catch (e: Exception) {
                    // pairGetSas not available yet (discovery not started) — suppress.
                    Log.v(TAG, "pairResponderPoll: pairGetSas unavailable: ${e.message}")
                }
                delay(PAIR_RESPONDER_POLL_MS)
            }
        }
    }

    /**
     * CopyPaste-mip2: watch for newly-discovered mDNS peers and signal an
     * opportunistic P2P dial the moment one appears on the LAN.
     *
     * Polls [listDiscovered] every [MDNS_PEER_WATCH_INTERVAL_MS].  Fires
     * [FgsSyncLoop.signalP2pWake] on the FIRST tick where the discovered
     * count transitions from zero to non-zero, and again on every subsequent
     * increase (a new peer joined while another was already present).
     *
     * Rationale: [listDiscovered] is cheap (in-memory mDNS table read, no
     * network I/O), so polling at 2 s does not drain the battery.  The CONFLATED
     * channel in [FgsSyncLoop] absorbs any signal burst — at most one early dial
     * fires per burst of discoveries.
     *
     * Idempotent: a no-op while [mdnsPeerWatchJob] is already running.
     * All exceptions from [listDiscovered] are caught and logged — the watcher
     * must never crash the FGS.
     */
    fun startMdnsPeerWatcher() {
        if (mdnsPeerWatchJob?.isActive == true) return
        if (!isNativeLibraryLoaded) return

        mdnsPeerWatchJob = scope.launch(Dispatchers.IO) {
            var lastKnownCount = 0
            while (isActive) {
                try {
                    val peers = settings.pairedPeers
                    if (peers.isNotEmpty()) {
                        val discovered = listDiscovered(peers.map { it.fingerprint })
                        val count = discovered.size
                        if (count > lastKnownCount) {
                            // New peer(s) arrived — signal an immediate P2P dial.
                            Log.d(TAG, "mDNS peer watcher: $count peer(s) discovered (was $lastKnownCount) — signalling P2P wake")
                            fgsSyncLoop.signalP2pWake()
                        }
                        lastKnownCount = count
                    }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    // listDiscovered may throw if native side is not yet started.
                    Log.v(TAG, "mDNS peer watcher: listDiscovered unavailable: ${e.message}")
                }
                delay(MDNS_PEER_WATCH_INTERVAL_MS)
            }
        }
    }

    /**
     * Cancel the pair-responder poller job. Called from [ClipboardService.onDestroy]
     * — kept as a separate step (not bundled into [stopInboundP2pListener]) so
     * [ClipboardService.onDestroy] can preserve the ORIGINAL monolith's exact
     * teardown interleaving (stopInboundP2pListener, then fgsSyncLoop.stop(),
     * then realtimeClient/relayClient close, THEN this).
     */
    fun cancelPairResponderPoller() {
        pairResponderPollJob?.cancel()
        pairResponderPollJob = null
    }

    /**
     * Cancel the mDNS peer-watcher job. Called from [ClipboardService.onDestroy]
     * — see [cancelPairResponderPoller] for why this is a separate step.
     */
    fun cancelMdnsPeerWatcher() {
        mdnsPeerWatchJob?.cancel()
        mdnsPeerWatchJob = null
    }

    companion object {
        private const val TAG = "ClipboardService"

        /**
         * Port the inbound mTLS P2P listener is currently bound to (OS-assigned),
         * or 0 when no listener is running. Published by [startInboundP2pListener]
         * / cleared by [stopInboundP2pListener] so [PairActivity] can advertise
         * `"<lan-ip>:<port>"` to a peer at pair time (Path A: the peer persists it
         * and dials back). @Volatile — read cross-thread from the pairing flow.
         */
        @Volatile
        var activeListenerPort: Int = 0
            private set

        /**
         * CopyPaste-mip2: how often [startMdnsPeerWatcher] polls [listDiscovered] to
         * detect newly-arrived LAN peers.  2 s is short enough that a freshly-booted
         * macOS peer triggers an opportunistic dial within ~2 s of being advertised,
         * while being light enough to run continuously without draining the battery.
         */
        private const val MDNS_PEER_WATCH_INTERVAL_MS = 2_000L

        /** Poll cadence for the responder-role SAS watcher in [startPairResponderPoller]. */
        private const val PAIR_RESPONDER_POLL_MS = 1_000L
    }
}
