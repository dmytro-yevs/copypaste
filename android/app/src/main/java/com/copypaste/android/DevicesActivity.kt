package com.copypaste.android

import android.content.Intent
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeElevated
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdeSuccess
import com.copypaste.android.ui.theme.IdeText
import com.copypaste.android.ui.theme.SectionLabel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * "Online" recency threshold for the per-peer green dot.
 *
 * A peer that completed a successful P2P sync within the last [ONLINE_WINDOW_MS]
 * is rendered online (green dot); otherwise offline (grey). This mirrors the
 * macOS daemon's `ONLINE_THRESHOLD_SECS` (60 s) so both platforms agree on what
 * "online" means. The presence signal is [PairedPeer.lastSyncMs], stamped by
 * [FgsSyncLoop] (via [Settings.updatePeerLastSync]) on each successful dial —
 * NOT the old `lastSupabasePollWallTime` poll-cursor proxy.
 */
internal const val ONLINE_WINDOW_MS = 60_000L

/** True when [peer] synced within [ONLINE_WINDOW_MS] of [nowMs]. */
internal fun PairedPeer.isOnline(nowMs: Long = System.currentTimeMillis()): Boolean =
    lastSyncMs > 0L && (nowMs - lastSyncMs) <= ONLINE_WINDOW_MS

/**
 * Forget a single paired peer locally: remove its roster entry (fingerprint,
 * sync address, KEK-wrapped session key). The peer is NOT notified; it may keep
 * trying to contact us until it is also unpaired on its side.
 *
 * Does NOT touch this device's P2P identity (cert/key) — we keep our own
 * identity so our OTHER pairings keep working and re-pairing needs no new cert.
 */
fun unpairPeer(settings: Settings, fingerprint: String) {
    settings.removePeer(fingerprint)
}

// ─────────────────────────────────────────────────────────────────────────────
// Activity
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Devices screen — shows the full roster of paired P2P peers, each as a card
 * with a real-presence online dot, sync address, fingerprint, last-sync time,
 * and per-peer Unpair / Revoke actions. Parity with the macOS DevicesView.
 *
 * Navigation: launched from the DEVICES tab in [MainActivity] bottom nav, and
 * also accessible as a standalone activity from [SettingsActivity] (General tab
 * "Devices" row).
 */
class DevicesActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            CopyPasteTheme {
                DevicesScreen(showBackButton = true, onBack = { finish() })
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Composable screen (also embedded in MainShell's DEVICES tab)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
fun DevicesScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val deviceKeyStore = remember { DeviceKeyStore(ctx) }
    val scope = rememberCoroutineScope()

    // Refresh the roster every poll interval so the online dots and last-sync
    // labels update as FgsSyncLoop stamps presence.
    var peers by remember { mutableStateOf(settings.pairedPeers) }
    var ownIdentity by remember { mutableStateOf(settings.p2pIdentity) }

    // ── LAN discovery + SAS pairing state ─────────────────────────────────────
    // P2P must be enabled for discovery (parity with the daemon gating discovery
    // behind start_p2p). When disabled we neither advertise nor browse.
    val p2pEnabled = remember { settings.p2pSyncEnabled }
    // Non-paired, SAS-capable peers discovered on the LAN (refreshed by the poll
    // effect below). Paired peers are filtered out natively via `paired`.
    var discovered by remember { mutableStateOf<List<DiscoveredPeer>>(emptyList()) }
    // The peer a SAS pairing modal is currently open for, or null. Setting it
    // non-null opens the modal (which begins polling pair_get_sas).
    var pairingPeer by remember { mutableStateOf<DiscoveredPeer?>(null) }
    // True while pair_with_discovered is in flight (before the modal opens).
    var pairStarting by remember { mutableStateOf(false) }
    // Inline error shown beneath the discovered list (e.g. another pairing busy).
    var discoverError by remember { mutableStateOf<String?>(null) }

    fun refresh() {
        peers = settings.pairedPeers
        ownIdentity = settings.p2pIdentity
    }

    LaunchedEffect(Unit) {
        while (true) {
            delay(PEER_POLL_MS)
            refresh()
        }
    }

    // ── mDNS discovery lifecycle lives in ClipboardService (HB-2) ─────────────
    // Discovery (the mDNS advert + the standing SAS-pairing responder on
    // [SAS_BPORT]) is started/stopped by the always-on [ClipboardService] FGS,
    // NOT here. Hosting it on this screen meant the responder died the moment the
    // Devices screen closed, so a Mac→Android pair got "Connection refused". The
    // FGS keeps it alive for the lifetime of the service; this screen only
    // browses the resulting peer snapshot below.

    // ── Poll the discovered peer list every ~2 s ──────────────────────────────
    // HB-4: listDiscovered marks `paired` by IP-correlation now (the mDNS
    // device_id is a UUID, not a cert fingerprint, so the old fingerprint-compare
    // never matched). We pass the set of IP hosts we have paired with — each
    // peer's syncAddr host plus its peerLocalIp — and drop the matched entries.
    LaunchedEffect(p2pEnabled) {
        if (!p2pEnabled) {
            discovered = emptyList()
            return@LaunchedEffect
        }
        while (true) {
            try {
                val pairedIps = settings.pairedPeers.flatMap { peer ->
                    listOfNotNull(
                        // host part of "host:port" (substringBeforeLast tolerates a
                        // bare host with no port).
                        peer.syncAddr.takeIf { it.isNotEmpty() }?.substringBeforeLast(':'),
                        peer.peerLocalIp?.takeIf { it.isNotEmpty() },
                    )
                }.distinct()
                val list = withContext(Dispatchers.IO) { listDiscovered(pairedIps) }
                discovered = list.filterNot { it.paired }
            } catch (e: Exception) {
                // Discovery is best-effort — keep the previous snapshot, log only.
                Log.w(TAG, "listDiscovered failed: ${e.message}")
            }
            delay(DISCOVERED_POLL_MS)
        }
    }

    // Begin a discovery-initiated SAS pairing as initiator, then open the modal.
    fun startPairing(peer: DiscoveredPeer) {
        if (pairStarting || pairingPeer != null) return
        discoverError = null
        pairStarting = true
        scope.launch {
            try {
                val cert = withContext(Dispatchers.IO) {
                    deviceKeyStore.peek() ?: deviceKeyStore.getOrCreate()
                }
                withContext(Dispatchers.IO) {
                    pairWithDiscovered(
                        deviceId = peer.deviceId,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        // The peer (a configured Mac) provides provisioning; the
                        // phone advertises no sync address / carries no config.
                        syncAddr = "",
                        localProvisioning = null,
                        // HB-1a (ABI 14): advertise this device's own metadata.
                        deviceName = android.os.Build.MODEL ?: "Android",
                        deviceModel = android.os.Build.MODEL ?: "Android",
                        osVersion = "Android " + android.os.Build.VERSION.RELEASE,
                        appVersion = BuildConfig.VERSION_NAME,
                        localIp = lanIpv4Address(),
                    )
                }
                pairingPeer = peer
            } catch (e: Exception) {
                Log.w(TAG, "pairWithDiscovered failed: ${e.message}", e)
                discoverError = e.message ?: "Failed to start pairing."
                // HB-8: pairWithDiscovered may have claimed the native SM (via
                // try_begin) before failing — reset defensively so a retry is not
                // refused with "a pairing is already in flight".
                try {
                    withContext(Dispatchers.IO) { pairReset() }
                } catch (re: Exception) {
                    Log.w(TAG, "pairReset after failed start failed: ${re.message}")
                }
            } finally {
                pairStarting = false
            }
        }
    }

    // Per-peer dialog targets (null = no dialog showing).
    var unpairTarget by remember { mutableStateOf<PairedPeer?>(null) }
    var revokeTarget by remember { mutableStateOf<PairedPeer?>(null) }
    // Non-null when an async revokeDeviceAudit IO call failed — surfaced to the user.
    var revokeError by remember { mutableStateOf<String?>(null) }

    // ── Unpair confirmation ──────────────────────────────────────────────────
    unpairTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { unpairTarget = null },
            title = { Text("Forget paired device?") },
            text = {
                Text(
                    "This device will no longer sync with ${target.displayName()} over P2P. " +
                    "You can re-pair at any time by scanning a new QR code."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    unpairTarget = null
                    unpairPeer(settings, target.fingerprint)
                    refresh()
                }) { Text("Forget", color = IdeDanger) }
            },
            dismissButton = {
                TextButton(onClick = { unpairTarget = null }) { Text("Cancel") }
            },
        )
    }

    // ── Revoke confirmation ──────────────────────────────────────────────────
    revokeTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { revokeTarget = null },
            title = { Text("Revoke pairing?") },
            text = {
                Text(
                    "${target.displayName()} will no longer connect over P2P, and a " +
                    "revocation record is kept. But a revoked device that still holds " +
                    "the shared sync key can keep reading cloud and relay items until " +
                    "you rotate the sync key. To rotate it, change the Sync Passphrase " +
                    "in Settings — every device must then re-enter the new passphrase " +
                    "(or re-pair) to keep syncing."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    revokeTarget = null
                    // Forget the PEER locally (never our own p2pIdentity), then
                    // write a durable audit/revocation record on the IO dispatcher.
                    settings.removePeer(target.fingerprint)
                    refresh()
                    scope.launch {
                        val ok = withContext(Dispatchers.IO) {
                            runCatching {
                                revokeDeviceAudit(
                                    dbPath = settings.dbPath,
                                    key = settings.encryptionKey,
                                    fingerprint = target.fingerprint,
                                    name = target.displayName(),
                                )
                            }
                        }.fold(
                            onSuccess = { true },
                            onFailure = { e ->
                                Log.e(
                                    TAG,
                                    "revokeDeviceAudit failed for ${target.fingerprint.take(8)}: ${e.message}",
                                    e,
                                )
                                false
                            },
                        )
                        if (!ok) revokeError = "Failed to record revocation. The peer was unpaired locally."
                    }
                }) { Text("Revoke", color = IdeDanger) }
            },
            dismissButton = {
                TextButton(onClick = { revokeTarget = null }) { Text("Cancel") }
            },
        )
    }

    // ── Revoke failure surface ────────────────────────────────────────────────
    revokeError?.let { msg ->
        AlertDialog(
            onDismissRequest = { revokeError = null },
            title = { Text("Revocation incomplete") },
            text = { Text(msg) },
            confirmButton = {
                TextButton(onClick = { revokeError = null }) { Text("OK") }
            },
        )
    }

    // ── SAS pairing modal (port of macOS SasPairingModal) ─────────────────────
    pairingPeer?.let { peer ->
        SasPairingDialog(
            peer = peer,
            settings = settings,
            onClose = { pairingPeer = null },
            onPaired = { refresh() },
        )
    }

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_devices),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = "Back",
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {

            // ── Paired peers ────────────────────────────────────────────────
            SectionLabel("Paired Devices")

            if (peers.isNotEmpty()) {
                for (peer in peers) {
                    PeerCard(
                        peer = peer,
                        onUnpair = { unpairTarget = peer },
                        onRevoke = { revokeTarget = peer },
                    )
                }
            } else {
                NoPeerCard(
                    onPair = {
                        ctx.startActivity(Intent(ctx, PairActivity::class.java))
                    }
                )
            }

            // HB-6: scanning a device's QR lives HERE now (was on the Pair screen).
            // Launch PairActivity with mode=scan so it auto-opens its camera scan
            // flow; the Pair screen otherwise shows only THIS device's own QR.
            OutlinedButton(
                onClick = {
                    ctx.startActivity(
                        Intent(ctx, PairActivity::class.java)
                            .putExtra("mode", "scan")
                    )
                },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(stringResource(R.string.btn_scan_qr))
            }

            Spacer(Modifier.height(8.dp))

            // ── Discovered on your network ───────────────────────────────────
            // Parity with the macOS DevicesView "Devices on your network" list:
            // unpaired, SAS-capable LAN peers with a Pair button. Only shown when
            // P2P is enabled (discovery is gated on it).
            if (p2pEnabled) {
                SectionLabel("Discovered on Your Network")
                if (discovered.isNotEmpty()) {
                    for (peer in discovered) {
                        DiscoveredPeerCard(
                            peer = peer,
                            busy = pairStarting || pairingPeer != null,
                            onPair = { startPairing(peer) },
                        )
                    }
                } else {
                    Text(
                        text = "Searching for nearby devices…",
                        color = IdeFaint,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                discoverError?.let { msg ->
                    Text(
                        text = msg,
                        color = IdeDanger,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Spacer(Modifier.height(8.dp))
            }

            // ── This device ──────────────────────────────────────────────────
            ownIdentity?.let { identity ->
                SectionLabel("This Device")
                OwnDeviceCard(identity = identity)
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

/** Display label for a peer: its name when set, else a short fingerprint. */
private fun PairedPeer.displayName(): String =
    name.ifBlank { "device ${fingerprint.take(8)}" }

// ─────────────────────────────────────────────────────────────────────────────
// Peer card
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PeerCard(
    peer: PairedPeer,
    onUnpair: () -> Unit,
    onRevoke: () -> Unit,
) {
    val online = peer.isOnline()
    val dotColor = if (online) IdeSuccess else IdeFaint

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            // ── Header row: dot + name + status ─────────────────────────────
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .clip(CircleShape)
                        .background(dotColor),
                )
                Text(
                    text = peer.name.ifBlank { "Paired device" },
                    color = IdeText,
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = if (online) "Online" else "Offline",
                    color = dotColor,
                    style = MaterialTheme.typography.labelMedium,
                )
            }

            Spacer(Modifier.height(10.dp))

            // ── Fingerprint (short) ─────────────────────────────────────────
            DeviceField(label = "Fingerprint", value = peer.fingerprint.take(16))

            // ── Device info (HB-1c) ─────────────────────────────────────────
            // Peer metadata learned in-band during pairing (ABI 14, persisted on
            // PairedPeer.peer*). Each row is omitted when the field is absent — a
            // legacy / pre-ABI-14 roster entry simply shows none of them.
            peer.peerModel?.takeIf { it.isNotBlank() }?.let {
                Spacer(Modifier.height(6.dp))
                DeviceField(label = "Model", value = it)
            }
            peer.peerOs?.takeIf { it.isNotBlank() }?.let {
                Spacer(Modifier.height(6.dp))
                DeviceField(label = "OS", value = it)
            }
            peer.peerAppVersion?.takeIf { it.isNotBlank() }?.let {
                Spacer(Modifier.height(6.dp))
                DeviceField(label = "App version", value = it)
            }
            peer.peerLocalIp?.takeIf { it.isNotBlank() }?.let {
                Spacer(Modifier.height(6.dp))
                DeviceField(label = "Local IP", value = it)
            }
            peer.peerPublicIp?.takeIf { it.isNotBlank() }?.let {
                Spacer(Modifier.height(6.dp))
                DeviceField(label = "Public IP", value = it)
            }

            // ── Sync address ────────────────────────────────────────────────
            if (peer.syncAddr.isNotBlank()) {
                Spacer(Modifier.height(6.dp))
                DeviceField(label = "Sync address", value = peer.syncAddr)
            }

            // ── Last sync ───────────────────────────────────────────────────
            if (peer.lastSyncMs > 0L) {
                Spacer(Modifier.height(6.dp))
                val elapsed = (System.currentTimeMillis() - peer.lastSyncMs) / 1_000L
                val lastSyncText = when {
                    elapsed < 60 -> "${elapsed}s ago"
                    elapsed < 3600 -> "${elapsed / 60}m ago"
                    else -> "${elapsed / 3600}h ago"
                }
                DeviceField(label = "Last sync", value = lastSyncText)
            }

            HorizontalDivider(
                modifier = Modifier.padding(vertical = 12.dp),
                color = IdeBorder.copy(alpha = 0.5f),
                thickness = 0.5.dp,
            )

            // ── Actions ─────────────────────────────────────────────────────
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(
                    onClick = onUnpair,
                    modifier = Modifier.weight(1f),
                ) {
                    Text("Unpair", color = IdeDanger)
                }
                Button(
                    onClick = onRevoke,
                    modifier = Modifier.weight(1f),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = IdeDanger.copy(alpha = 0.15f),
                        contentColor = IdeDanger,
                    ),
                ) {
                    Text("Revoke")
                }
            }
        }
    }
}

@Composable
private fun NoPeerCard(onPair: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = "No device paired",
                color = IdeDim,
                style = MaterialTheme.typography.bodyLarge,
            )
            Text(
                text = "Pair with a Mac running CopyPaste to enable P2P clipboard sync over your local network.",
                color = IdeFaint,
                style = MaterialTheme.typography.bodySmall,
            )
            Button(onClick = onPair) {
                Text("Pair a device")
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Own-device card
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun OwnDeviceCard(identity: P2pIdentity) {
    // HB-1c: render THIS device's info at parity with the macOS "This Mac" card.
    // ABI 14 sends these same fields to peers (own gather in PairActivity /
    // DevicesActivity startPairing); we surface them locally too. Gathered live —
    // P2pIdentity only carries the id/fingerprint, the rest comes from the
    // platform (Build/BuildConfig) and a LAN-IPv4 enumeration. No synchronous
    // public-IP source on-device, so that row is omitted (matches the bootstrap
    // path, which sends public_ip = None for this device).
    val model = android.os.Build.MODEL ?: "Android"
    val osVersion = "Android " + android.os.Build.VERSION.RELEASE
    val appVersion = BuildConfig.VERSION_NAME
    val localIp = remember { lanIpv4Address() }

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            // Header: online dot (this device is by definition online) + model.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .clip(CircleShape)
                        .background(IdeSuccess),
                )
                Text(
                    text = model,
                    color = IdeText,
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = "Online",
                    color = IdeSuccess,
                    style = MaterialTheme.typography.labelMedium,
                )
            }

            Spacer(Modifier.height(10.dp))
            DeviceField(label = "Model", value = model)
            Spacer(Modifier.height(6.dp))
            DeviceField(label = "OS", value = osVersion)
            Spacer(Modifier.height(6.dp))
            DeviceField(label = "App version", value = appVersion)
            localIp?.let {
                Spacer(Modifier.height(6.dp))
                DeviceField(label = "Local IP", value = it)
            }
            Spacer(Modifier.height(6.dp))
            DeviceField(label = "Device ID", value = identity.deviceId)
            Spacer(Modifier.height(6.dp))
            DeviceField(label = "My fingerprint", value = identity.fingerprint)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Discovered-peer card (LAN, unpaired)
// ─────────────────────────────────────────────────────────────────────────────

/** Short label for a discovered peer: name when set, else a short device id. */
private fun DiscoveredPeer.displayName(): String =
    deviceName.ifBlank { "Device ${deviceId.take(8)}" }

/**
 * One discovered (unpaired) LAN device row with a Pair button. Mirrors the macOS
 * DiscoveredRow: the Pair button is DISABLED when the peer advertises no
 * bootstrap port ([DiscoveredPeer.bport] == null) — a v1 peer that cannot do SAS
 * pairing — or while another pairing is in flight ([busy]).
 */
@Composable
private fun DiscoveredPeerCard(
    peer: DiscoveredPeer,
    busy: Boolean,
    onPair: () -> Unit,
) {
    // v1 peers (no bootstrap port) cannot do SAS pairing → disable Pair.
    val pairable = peer.bport != null
    val ip = peer.ipAddrs.firstOrNull()

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = peer.displayName(),
                    color = IdeText,
                    style = MaterialTheme.typography.titleSmall,
                )
                Spacer(Modifier.height(4.dp))
                DeviceField(
                    label = "Fingerprint",
                    value = peer.deviceId.take(16),
                )
                if (ip != null) {
                    Spacer(Modifier.height(4.dp))
                    DeviceField(label = "Local IP", value = ip)
                }
            }
            Button(
                onClick = onPair,
                enabled = pairable && !busy,
            ) {
                Text("Pair")
            }
        }
        if (!pairable) {
            Text(
                text = "This device does not support secure pairing.",
                color = IdeFaint,
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SAS pairing modal (port of macOS DevicesView SasPairingModal)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Modal that drives a discovery-initiated SAS pairing to completion.
 *
 * Behaviour mirrors the macOS [SasPairingModal]:
 *  - polls [pairGetSas] every [SAS_POLL_MS];
 *  - `initiating` → spinner ("Connecting…");
 *  - `awaiting_sas` with a code → shows the 6-digit SAS + Match / Doesn't match;
 *  - `awaiting_sas` without a code → "Waiting for the other device…";
 *  - `confirmed` → persists the peer (KEK-wrapped session key + fill-missing
 *    provisioning) and shows success;
 *  - `rejected` / `aborted` / `timed_out` → error;
 *  - a TRAILING `idle` observed AFTER an active state is itself terminal
 *    ("pairing ended"): if the user already accepted locally, treat as success,
 *    else show a neutral "ended" close state — never loop on idle.
 *
 * Closing before a terminal state calls [pairAbort] exactly once; after any
 * terminal state [pairReset] is called to clear the native state machine.
 *
 * SECURITY: the SAS code is shown on screen but NEVER logged; the session-key
 * bytes are wrapped + zeroized and never logged.
 */
@Composable
private fun SasPairingDialog(
    peer: DiscoveredPeer,
    settings: Settings,
    onClose: () -> Unit,
    onPaired: () -> Unit,
) {
    val scope = rememberCoroutineScope()

    // Current pairing status; starts optimistically at "initiating".
    var status by remember {
        mutableStateOf(
            PairStatus(
                state = "initiating",
                sas = null,
                role = null,
                peerFingerprint = null,
                peerSyncAddr = null,
                sessionKey = null,
                peerProvisioning = null,
                // ABI 14 (HB-1b): peer metadata, populated by the native side on confirm.
                peerModel = null,
                peerOs = null,
                peerAppVersion = null,
                peerLocalIp = null,
                peerPublicIp = null,
            )
        )
    }
    // Transient (non-terminal) poll/confirm error.
    var error by remember { mutableStateOf<String?>(null) }
    // True while a pairConfirmSas call is in flight (disables the buttons).
    var confirmPending by remember { mutableStateOf(false) }
    // Neutral terminal close state — handshake ended on a trailing idle without a
    // local confirm. Distinct from the wire `aborted` state.
    var ended by remember { mutableStateOf(false) }
    // True once a terminal Confirmed has been observed — closing then must NOT
    // call pairAbort (the pairing already succeeded).
    val confirmedRef = remember { mutableStateOf(false) }
    // True once the user locally accepted (clicked Match): disambiguates a
    // trailing idle (local-accepted + idle ⇒ success).
    val localAcceptedRef = remember { mutableStateOf(false) }

    val terminal = ended ||
        status.state == "confirmed" ||
        status.state == "rejected" ||
        status.state == "aborted" ||
        status.state == "timed_out"

    // Persist a confirmed pairing: KEK-wrap the session key, upsert the peer, and
    // apply peer provisioning fill-missing (copied from PairActivity). Runs on IO.
    suspend fun persistConfirmed(st: PairStatus) {
        val fingerprint = st.peerFingerprint ?: return
        val keyUBytes = st.sessionKey ?: return
        withContext(Dispatchers.IO) {
            val rawSessionKey = ByteArray(keyUBytes.size) { keyUBytes[it].toByte() }
            try {
                val (wrappedB64, ivB64) = settings.wrapSessionKey(rawSessionKey)
                settings.upsertPeer(
                    PairedPeer(
                        fingerprint = fingerprint,
                        syncAddr = st.peerSyncAddr ?: "",
                        name = peer.deviceName,
                        sessionKeyWrappedB64 = wrappedB64,
                        sessionKeyIvB64 = ivB64,
                        lastSyncMs = System.currentTimeMillis(),
                        // HB-1b (ABI 14): persist the peer's device metadata received
                        // over the discovery/SAS pairing for the Wave-3 device card.
                        peerModel = st.peerModel,
                        peerOs = st.peerOs,
                        peerAppVersion = st.peerAppVersion,
                        peerLocalIp = st.peerLocalIp,
                        peerPublicIp = st.peerPublicIp,
                    )
                )

                // Apply peer provisioning fill-missing — NEVER overwrite a value
                // this device already configured (mirror the daemon's rule and the
                // PairActivity QR block). Never log the derived key bytes.
                st.peerProvisioning?.let { prov ->
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
                    prov.derivedSyncKey?.takeIf { it.isNotEmpty() }?.let { keyU ->
                        if (settings.cloudSyncKeyDirect == null) {
                            val keyBytes = ByteArray(keyU.size) { keyU[it].toByte() }
                            settings.cloudSyncKeyDirect = keyBytes
                            applied += "derivedSyncKey"
                        }
                    }
                    if (applied.isNotEmpty()) {
                        Log.i(TAG, "SAS provisioning applied (fill-missing): ${applied.joinToString(", ")}")
                    }
                }
            } finally {
                // Zero the raw session key copy once it has been wrapped.
                rawSessionKey.fill(0)
            }
        }
    }

    // Poll pair_get_sas until a terminal state. The native state machine resets to
    // idle after a terminal outcome, so a trailing idle (after an active state) is
    // itself terminal — never re-poll on it.
    LaunchedEffect(peer.deviceId) {
        var sawActive = false
        while (true) {
            val next = try {
                withContext(Dispatchers.IO) { pairGetSas() }
            } catch (e: Exception) {
                error = e.message ?: "Pairing status unavailable"
                return@LaunchedEffect
            }

            when (next.state) {
                "initiating", "awaiting_sas" -> {
                    sawActive = true
                    status = next
                    delay(SAS_POLL_MS)
                }
                "confirmed" -> {
                    confirmedRef.value = true
                    status = next
                    persistConfirmed(next)
                    onPaired()
                    pairReset()
                    return@LaunchedEffect
                }
                "rejected", "aborted", "timed_out" -> {
                    status = next
                    pairReset()
                    return@LaunchedEffect
                }
                else -> {
                    // state == "idle"
                    if (sawActive) {
                        if (confirmedRef.value || localAcceptedRef.value) {
                            confirmedRef.value = true
                            // Persist from the last status we held the keys on.
                            persistConfirmed(status)
                            status = PairStatus(
                                state = "confirmed",
                                sas = null,
                                role = null,
                                peerFingerprint = status.peerFingerprint,
                                peerSyncAddr = status.peerSyncAddr,
                                sessionKey = null,
                                peerProvisioning = null,
                                // HB-1b: carry forward the peer metadata we last held.
                                peerModel = status.peerModel,
                                peerOs = status.peerOs,
                                peerAppVersion = status.peerAppVersion,
                                peerLocalIp = status.peerLocalIp,
                                peerPublicIp = status.peerPublicIp,
                            )
                            onPaired()
                        } else {
                            ended = true
                        }
                        pairReset()
                        return@LaunchedEffect
                    }
                    // Idle before any active state — keep waiting.
                    status = next
                    delay(SAS_POLL_MS)
                }
            }
        }
    }

    // Close: abort the pairing unless it already succeeded (exactly once), then
    // ALWAYS reset the native pairing state machine.
    //
    // HB-8: pairAbort() moves the SM to the terminal `Aborted` state but leaves
    // `try_begin` claimed, so without a follow-up pairReset() every later pairing
    // attempt failed with "a pairing is already in flight". pairReset() returns
    // the SM to Idle. It is idempotent and safe whether we aborted, already hit a
    // terminal state, or the pairing succeeded.
    fun handleClose() {
        if (!confirmedRef.value && !terminal) {
            // Abort branch: abort, then reset, on the same IO dispatcher so the
            // reset is ordered AFTER the abort.
            scope.launch(Dispatchers.IO) {
                pairAbort()
                pairReset()
            }
        } else {
            // Already-terminal / confirmed branch: nothing to abort, but still
            // clear the SM so the next pairing can claim it.
            scope.launch(Dispatchers.IO) { pairReset() }
        }
        onClose()
    }

    fun handleConfirm(accept: Boolean) {
        confirmPending = true
        error = null
        // Record the local accept up-front so a trailing idle is read as success.
        if (accept) localAcceptedRef.value = true
        scope.launch {
            try {
                withContext(Dispatchers.IO) { pairConfirmSas(accept) }
                if (!accept) {
                    // User said it doesn't match — abort path already handled by
                    // the native side; close immediately.
                    onClose()
                    return@launch
                }
                // On accept keep polling; the next tick reflects confirmed/rejected.
            } catch (e: Exception) {
                // The decision never reached the native side — undo the optimistic
                // accept flag so a later trailing idle isn't misread as success.
                localAcceptedRef.value = false
                error = e.message ?: "Failed to send decision"
            } finally {
                confirmPending = false
            }
        }
    }

    val title = peer.displayName()

    AlertDialog(
        onDismissRequest = { handleClose() },
        title = { Text("Pair “$title”") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                when {
                    ended -> {
                        Text(
                            "Pairing ended — check the other device.",
                            color = IdeDim,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "confirmed" -> {
                        Text(
                            "Paired ✓",
                            color = IdeSuccess,
                            style = MaterialTheme.typography.titleSmall,
                        )
                    }
                    status.state == "rejected" || status.state == "aborted" || status.state == "timed_out" -> {
                        Text(
                            when (status.state) {
                                "timed_out" -> "Pairing timed out."
                                "rejected" -> "Pairing was rejected."
                                else -> "Pairing was cancelled."
                            },
                            color = IdeDanger,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "awaiting_sas" && status.sas != null -> {
                        Text(
                            "Confirm this code matches the one shown on the other device.",
                            color = IdeDim,
                            style = MaterialTheme.typography.bodySmall,
                        )
                        Text(
                            text = status.sas ?: "",
                            color = IdeText,
                            textAlign = TextAlign.Center,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 32.sp,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 8.dp),
                        )
                    }
                    status.state == "awaiting_sas" -> {
                        // Accepted locally; waiting for the peer to also accept.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(modifier = Modifier.size(18.dp))
                            Text(
                                "Waiting for the other device…",
                                color = IdeDim,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                    else -> {
                        // initiating / idle-before-active → connecting spinner.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(modifier = Modifier.size(18.dp))
                            Text(
                                "Connecting…",
                                color = IdeDim,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                }
                error?.let { msg ->
                    if (!terminal) {
                        Text(msg, color = IdeDanger, style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        },
        confirmButton = {
            when {
                terminal -> {
                    TextButton(onClick = { onClose() }) { Text("Close") }
                }
                status.state == "awaiting_sas" && status.sas != null -> {
                    TextButton(
                        enabled = !confirmPending,
                        onClick = { handleConfirm(true) },
                    ) { Text(if (confirmPending) "…" else "Match") }
                }
                else -> {}
            }
        },
        dismissButton = {
            when {
                terminal -> {}
                status.state == "awaiting_sas" && status.sas != null -> {
                    TextButton(
                        enabled = !confirmPending,
                        onClick = { handleConfirm(false) },
                    ) { Text("Doesn't match", color = IdeDim) }
                }
                else -> {
                    TextButton(onClick = { handleClose() }) { Text("Cancel", color = IdeFaint) }
                }
            }
        },
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun DeviceField(label: String, value: String) {
    Column {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = IdeDim,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            color = IdeText,
            fontSize = 11.sp,
        )
    }
}

private const val TAG = "DevicesActivity"

/** Poll cadence for refreshing peer state on the Devices screen. */
private const val PEER_POLL_MS = 10_000L

/** Poll cadence for refreshing the LAN-discovered peer list (~2 s). */
private const val DISCOVERED_POLL_MS = 2_000L

/** Poll cadence for the SAS pairing state machine (~500 ms). */
private const val SAS_POLL_MS = 500L

/**
 * Fixed bootstrap (SAS-pairing) listener port this device advertises in its mDNS
 * TXT record so peers can dial back to pair. A non-zero bport marks this device
 * SAS-pairing-capable (v2); the native discovery service binds/owns this port.
 */
// `internal` so the always-on [ClipboardService] FGS owns the discovery
// lifecycle with the SAME well-known bport (HB-2).
internal const val SAS_BPORT = 47_654
