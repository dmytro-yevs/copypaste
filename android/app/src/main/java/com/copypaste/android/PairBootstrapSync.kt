package com.copypaste.android

import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.copypaste_android.BootstrapResult
import uniffi.copypaste_android.ScannedPairing

// CopyPaste-vp63.38: runBootstrap/finalizeSync moved out of PairController.kt
// into extension functions purely to keep every extracted file under the
// size target — together with PairController.kt and PairingApi.kt they form
// ONE logical state machine. Bodies are verbatim from the former PairScreen
// composable (bar `settings`/`deviceKeyStore`/`repository`/`context`/`scope`/
// `api` now being PairController's constructor properties instead of
// composable-local vals).

// Drive bootstrap PAKE pairing + a single P2P sync against the scanned peer
// (Android-as-initiator). Runs entirely off the main thread; result text is
// shown on completion. All FFI errors surface as a snackbar (no crash).
//
// NOTE (L4, RESOLVED on the macOS side): the daemon now advertises a real
// LAN-routable host:port (via copypaste_p2p::interfaces::advertise_sync_addr)
// in BOTH the QR addr_hint AND the in-band P2P sync-listener address, instead
// of 127.0.0.1. So `bootstrap.peerSyncAddr` persisted below in
// `settings.pairedPeerSyncAddr` is now dialable from a real phone over Wi-Fi
// (it is loopback only when the Mac has no LAN interface at all).
//
// ── CopyPaste-1jms.33: two-phase pairing flow ────────────────────────────
//
// Phase 1 — runBootstrap: runs the PAKE exchange and stores the result in
// [PairController.pendingBootstrap].  The peer's model/OS/appVersion are now
// visible in the UI so the user can verify them BEFORE confirming the sync.
//
// Phase 2 — finalizeSync: uses the cached [BootstrapResult] to apply
// provisioning, run the initial sync and commit the peer to the roster.
//
// The PAKE crypto flow is unchanged — only the UI confirmation step is split.
// ─────────────────────────────────────────────────────────────────────────

/**
 * Phase 1: run the PAKE bootstrap and surface the peer's device metadata.
 *
 * On success [PairController.pendingBootstrap] is set; the UI switches to the
 * review card which shows model/OS/appVersion and a "Confirm & sync" button.
 * On failure [PairController.errorMessage] is set (sanitized) and
 * [PairController.pendingBootstrap] stays null.
 */
internal fun PairController.runBootstrap(peer: ScannedPairing) {
    if (syncing) return
    // CopyPaste-1jms.13: resolve the human-readable device name BEFORE
    // entering the coroutine, where `context` (LocalContext.current) is only
    // accessible on the composition thread and ContentResolver is not
    // capturable via a qualified-this label inside a nested coroutine.
    val deviceNameForPairing: String = run {
        val settingsName = try {
            android.provider.Settings.Global.getString(
                context.contentResolver,
                "device_name",
            )
        } catch (_: Exception) { null }
        settingsName?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
            ?: "Android"
    }
    scope.launch {
        syncing = true
        syncResult = null
        try {
            val bootstrap = withContext(Dispatchers.IO) {
                // CopyPaste-44rq.55: getOrCreate() zeroes cert.keyDer; re-fetch
                // via peek() to obtain the KEK-unwrapped key from AndroidKeyStore.
                deviceKeyStore.getOrCreate()
                val cert = deviceKeyStore.peek()!!
                // Path A: advertise THIS device's inbound mTLS listener address
                // so the macOS peer persists it and can dial back (macOS→Android
                // direction). The listener is bound by [ClipboardService]; its
                // OS-assigned port is published in [ClipboardService.activeListenerPort].
                val listenerPort = ClipboardService.activeListenerPort
                val lanIp = lanIpv4Address()
                val ownSyncAddr = if (listenerPort > 0 && lanIp != null) {
                    "$lanIp:$listenerPort"
                } else {
                    android.util.Log.i(
                        "PairActivity",
                        "Not advertising listener sync_addr (port=$listenerPort, lanIp=${lanIp ?: "none"}) — " +
                            "falling back to Android→macOS dial only until the listener is up",
                    )
                    ""
                }
                // ABI 18 (PG-28): collect own WAN address via STUN so the
                // peer's device record shows a reachable external candidate.
                val ownPublicIp = StunUtils.queryPublicIp(settings.collectPublicIp)
                api.bootstrapPairInitiator(
                    addrHint = peer.addrHint,
                    certDer = cert.certDer,
                    keyDer = cert.keyDer,
                    pakePassword = peer.pakePassword,
                    syncAddr = ownSyncAddr,
                    localProvisioning = null,
                    // HB-1a (ABI 14): send THIS device's own metadata so the
                    // PC's device card shows real Android info.
                    deviceName = deviceNameForPairing,
                    deviceModel = Build.MODEL ?: "Android",
                    osVersion = "Android " + Build.VERSION.RELEASE,
                    appVersion = BuildConfig.VERSION_NAME,
                    localIp = lanIp,
                    publicIp = ownPublicIp,
                )
            }
            // PAKE succeeded — surface the peer metadata in the review card.
            pendingBootstrap = bootstrap
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception text to users.
            errorMessage = ErrorMessages.friendlyPairingError(e)
        } finally {
            syncing = false
        }
    }
}

/**
 * Phase 2: apply provisioning, run the initial P2P sync, and commit the peer
 * to the roster. Called after the user reviews the peer metadata and clicks
 * "Confirm & sync".
 *
 * [bootstrap] is the result from [runBootstrap] — already validated by PAKE.
 * Does NOT re-run the PAKE exchange.
 */
internal fun PairController.finalizeSync(peer: ScannedPairing, bootstrap: BootstrapResult) {
    if (syncing) return
    // CopyPaste-tqt0: snapshot the retained QR payload now; its 6th-field
    // provisioning is applied below ONLY after the PAKE bootstrap succeeds.
    val provisioningRaw = pendingProvisioningRaw
    scope.launch {
        syncing = true
        try {
            val key = settings.encryptionKey
            var pairedFingerprint: String? = null
            val message = withContext(Dispatchers.IO) {
                val cert = deviceKeyStore.peek()!!
                // QR full-provisioning: if the paired PC carried its sync
                // config in the pairing payload, fill any field this device
                // has not already configured. NEVER overwrite an existing
                // local value (mirror the daemon's fill-missing rule).
                bootstrap.peerProvisioning?.let { prov ->
                    val applied = mutableListOf<String>()
                    prov.supabaseUrl?.takeIf { it.isNotBlank() }?.let { url ->
                        if (settings.supabaseUrl.isBlank()) {
                            settings.supabaseUrl = url
                            applied += "supabaseUrl"
                        }
                    }
                    prov.supabaseAnonKey?.takeIf { it.isNotBlank() }?.let { anon ->
                        if (settings.supabaseAnonKey.isBlank()) {
                            settings.supabaseAnonKey = anon
                            applied += "supabaseAnonKey"
                        }
                    }
                    prov.relayUrl?.takeIf { it.isNotBlank() }?.let { relay ->
                        if (settings.relayUrl.isBlank()) {
                            settings.relayUrl = relay
                            applied += "relayUrl"
                        }
                    }
                    // The derived 32-byte cloud sync key: store via the direct
                    // key path (KEK-wrapped) so the phone can decrypt cloud
                    // rows without the passphrase. Only when none is set yet.
                    prov.derivedSyncKey?.takeIf { it.isNotEmpty() }?.let { keyUBytes ->
                        if (settings.cloudSyncKeyDirect == null) {
                            val keyBytes = ByteArray(keyUBytes.size) { keyUBytes[it].toByte() }
                            settings.cloudSyncKeyDirect = keyBytes
                            applied += "derivedSyncKey"
                        }
                    }
                    if (applied.isNotEmpty()) {
                        android.util.Log.i(
                            "PairActivity",
                            "QR provisioning applied (fill-missing): ${applied.joinToString(", ")}",
                        )
                    } else {
                        android.util.Log.i(
                            "PairActivity",
                            "QR provisioning carried by peer but all fields already configured locally — nothing applied",
                        )
                    }
                }
                // CopyPaste-tqt0: NOW (post-PAKE = the user confirmed pairing)
                // apply the QR's 6th-field relay/Supabase provisioning.
                provisioningRaw?.let { raw ->
                    val prov = extractQrProvisioning(raw)
                    val applied = prov?.let { applyQrProvisioning(it, settings) } ?: emptyList()
                    if (applied.isNotEmpty()) {
                        android.util.Log.i(
                            "PairActivity",
                            "QR provisioning (6th field) applied after pair confirmation: ${applied.joinToString(", ")}",
                        )
                    }
                }
                val localItems = repository.localItemsForSync(key)
                // Denylist: never ingest items from a peer this device revoked.
                val revoked = runCatching { api.listRevokedFingerprints(settings.dbPath, key) }
                    .getOrElse { e ->
                        android.util.Log.w("PairActivity", "listRevokedFingerprints failed: ${e.message}")
                        emptyList()
                    }
                val result = api.syncWithPeer(
                    peerAddr = bootstrap.peerSyncAddr,
                    peerFingerprint = bootstrap.peerFingerprint,
                    sessionKey = ByteArray(bootstrap.sessionKey.size) { bootstrap.sessionKey[it].toByte() },
                    certDer = cert.certDer,
                    keyDer = cert.keyDer,
                    localItems = localItems,
                    revokedFingerprints = revoked,
                    deviceId = settings.deviceId,
                )
                // HB-7b: route each received item BY CONTENT TYPE.
                var stored = 0
                for (item in result.items) {
                    val plaintextBytes =
                        ByteArray(item.plaintext.size) { item.plaintext[it].toByte() }
                    val isImage = item.contentType == "image" ||
                        item.contentType.startsWith("image/")
                    val isFile = item.contentType == "file"
                    val didStore = when {
                        isImage -> {
                            if (plaintextBytes.isEmpty()) {
                                false
                            } else {
                                val storedId = repository.storeItem(
                                    plaintext = "[image]",
                                    key = key,
                                    overrideId = item.itemId,
                                    contentType = item.contentType,
                                )
                                if (storedId.isNotEmpty()) {
                                    repository.storeImageBytes(storedId, plaintextBytes)
                                    SyncThumbnailHelper.generateAndStore(plaintextBytes) { thumb ->
                                        repository.storeThumbnailBytes(storedId, thumb)
                                    }
                                    true
                                } else {
                                    false
                                }
                            }
                        }
                        isFile -> {
                            if (plaintextBytes.isEmpty()) {
                                false
                            } else {
                                val label = SyncFileHelper.buildFileLabel(item.fileName)
                                val storedId = repository.storeItem(
                                    plaintext = label,
                                    key = key,
                                    overrideId = item.itemId,
                                    contentType = item.contentType,
                                )
                                if (storedId.isNotEmpty()) {
                                    repository.storeFileBytes(storedId, plaintextBytes)
                                    repository.storeFileMeta(storedId, item.fileName, item.mime)
                                    true
                                } else {
                                    false
                                }
                            }
                        }
                        else -> {
                            val plaintext = String(plaintextBytes, Charsets.UTF_8)
                            repository.storeItem(plaintext, key, overrideId = item.itemId)
                                .isNotEmpty()
                        }
                    }
                    if (didStore) stored += 1
                }
                // Persist (APPEND) the peer into the multi-peer roster.
                val rawSessionKey =
                    ByteArray(bootstrap.sessionKey.size) { bootstrap.sessionKey[it].toByte() }
                val (wrappedB64, ivB64) = settings.wrapSessionKey(rawSessionKey)
                val nowMs = System.currentTimeMillis()
                settings.upsertPeer(
                    PairedPeer(
                        fingerprint = bootstrap.peerFingerprint,
                        syncAddr = bootstrap.peerSyncAddr,
                        name = peer.deviceName,
                        sessionKeyWrappedB64 = wrappedB64,
                        sessionKeyIvB64 = ivB64,
                        lastSyncMs = nowMs,
                        pairedAtMs = nowMs,
                        // HB-1b (ABI 14): persist the peer's device metadata
                        // received over the authenticated tunnel.
                        peerModel = bootstrap.peerModel,
                        peerOs = bootstrap.peerOs,
                        peerAppVersion = bootstrap.peerAppVersion,
                        peerLocalIp = bootstrap.peerLocalIp,
                        peerPublicIp = bootstrap.peerPublicIp,
                        // CopyPaste-3k6m (ABI 17): persist the peer's stable device UUID.
                        peerDeviceId = bootstrap.peerDeviceId?.takeIf { it.isNotBlank() }
                            ?: peer.deviceId.takeIf { it.isNotBlank() },
                        // CopyPaste-6udn (ABI 19): persist the peer's linked Supabase
                        // account id. No fallback source exists — DiscoveredPeer/
                        // ScannedPairing carry no supabase-account field — so this is
                        // the bootstrap value straight through.
                        // Normalize blank to null for parity with peerDeviceId above —
                        // putOpt would otherwise persist an empty-string key.
                        peerSupabaseAccountId = bootstrap.peerSupabaseAccountId?.takeIf { it.isNotBlank() },
                    )
                )
                pairedFingerprint = bootstrap.peerFingerprint
                val peerCount = settings.pairedPeers.size
                val skipped = "skipped: legacy ${result.itemsSkippedLegacy} / " +
                    "decrypt ${result.itemsSkippedDecryptFail} / " +
                    "type ${result.itemsSkippedUnknownType} / " +
                    "blob ${result.itemsSkippedMissingBlob}"
                "Paired with ${peer.deviceName.ifBlank { "device" }} — received ${result.itemsReceived} item(s), stored $stored ($skipped), sent ${result.itemsSent}. ($peerCount paired device(s))"
            }
            // Surface the just-persisted peer for the compact success popup.
            pairedPeerForPopup = settings.pairedPeers
                .firstOrNull { it.fingerprint == pairedFingerprint }
            syncResult = message
            scannedPeer = null
            pendingBootstrap = null
            // CopyPaste-tqt0: provisioning has been applied; drop the retained payload.
            pendingProvisioningRaw = null
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception text to users.
            errorMessage = ErrorMessages.friendlyPairingError(e)
        } finally {
            syncing = false
        }
    }
}
