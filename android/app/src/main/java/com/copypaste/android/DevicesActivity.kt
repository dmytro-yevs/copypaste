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
import androidx.compose.ui.text.font.FontFamily
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
    val scope = rememberCoroutineScope()

    // Refresh the roster every poll interval so the online dots and last-sync
    // labels update as FgsSyncLoop stamps presence.
    var peers by remember { mutableStateOf(settings.pairedPeers) }
    var ownIdentity by remember { mutableStateOf(settings.p2pIdentity) }

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
                    "This device will no longer connect to ${target.displayName()}. " +
                    "A revocation record is kept."
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

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = "Devices",
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

            Spacer(Modifier.height(8.dp))

            // ── This device ──────────────────────────────────────────────────
            ownIdentity?.let { identity ->
                SectionLabel("This Device")
                OwnDeviceCard(identity = identity)
            }
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
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            DeviceField(label = "Device ID", value = identity.deviceId)
            Spacer(Modifier.height(6.dp))
            DeviceField(label = "My fingerprint", value = identity.fingerprint)
        }
    }
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
