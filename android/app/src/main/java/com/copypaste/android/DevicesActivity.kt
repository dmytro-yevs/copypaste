package com.copypaste.android

import android.content.Intent
import android.os.Bundle
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
import kotlinx.coroutines.delay

/**
 * View-model data class for a paired P2P peer.
 *
 * Populated entirely from [Settings] — no new FFI bindings required. The three
 * fields persisted by [PairActivity] after a successful pairing are:
 *   • [Settings.pairedPeerFingerprint] — SHA-256 hex fingerprint of peer's cert
 *   • [Settings.pairedPeerSyncAddr]    — host:port for background P2P dialer
 *   • [Settings.lastSupabasePollWallTime] — used as a coarse "last-sync" proxy
 *
 * Multi-peer note: [Settings] stores only ONE peer (the most recently paired).
 * Supporting a roster of peers requires a new SharedPreferences list or an FFI
 * call — deferred as a follow-up.
 */
data class PairedPeerInfo(
    val fingerprint: String,
    val syncAddr: String,
    /** Wall-clock ms of the most recent known sync event, or 0 if unknown. */
    val lastSyncMs: Long = 0L,
) {
    /**
     * True when [lastSyncMs] is within [ONLINE_WINDOW_MS] of [nowMs].
     * Mirrors the macOS daemon's "green dot" recency check (last_sync_ms > now - 60s).
     */
    fun isOnline(nowMs: Long = System.currentTimeMillis()): Boolean =
        lastSyncMs > 0L && (nowMs - lastSyncMs) <= ONLINE_WINDOW_MS

    companion object {
        /** 60-second window: a peer that synced within the last minute is "online". */
        const val ONLINE_WINDOW_MS = 60_000L

        /**
         * Build a [PairedPeerInfo] from raw values (used in tests and from [fromSettings]).
         * Returns null when [fingerprint] is blank — indicating no peer is paired.
         */
        fun fromRaw(fingerprint: String, syncAddr: String, lastSyncMs: Long = 0L): PairedPeerInfo? {
            if (fingerprint.isBlank()) return null
            return PairedPeerInfo(fingerprint = fingerprint, syncAddr = syncAddr, lastSyncMs = lastSyncMs)
        }

        /** Load the currently-paired peer from [settings], or null if not paired. */
        fun fromSettings(settings: Settings): PairedPeerInfo? = fromRaw(
            fingerprint = settings.pairedPeerFingerprint,
            syncAddr = settings.pairedPeerSyncAddr,
            // lastSupabasePollWallTime is the best available coarse "last seen" signal
            // on Android: it advances whenever a Supabase row is processed. It is NOT
            // a poll-completion clock, so a non-zero value only means "synced recently
            // enough to have received at least one row". Zero means never synced.
            lastSyncMs = settings.lastSupabasePollWallTime,
        )
    }
}

/**
 * Forget this device locally: clear the paired peer fingerprint, sync address,
 * and session key from SharedPreferences. The peer is NOT notified; it will
 * reject future connections from us because we no longer have a valid session
 * key, but it may still try to contact us until it is also unpaired.
 *
 * Does NOT touch the P2P identity (cert/key) — we keep our own identity so
 * re-pairing with a new peer works without regenerating the cert.
 */
fun unpairPeer(settings: Settings) {
    settings.pairedPeerFingerprint = ""
    settings.pairedPeerSyncAddr = ""
    settings.pairedPeerSessionKey = ByteArray(0)
}

/**
 * Revoke the pairing: same as [unpairPeer] plus clear our own P2P mTLS identity
 * so the peer can no longer authenticate us even if it retains our old fingerprint.
 * The next pairing will generate a fresh cert.
 *
 * This is the "nuclear" option — use when the user suspects the peer device is
 * compromised or lost. After revocation the user must re-pair with a fresh QR scan.
 */
fun revokePeer(settings: Settings) {
    unpairPeer(settings)
    // Clearing p2pIdentity forces a fresh cert on next pairing. The old cert's
    // fingerprint was pinned on the peer side; a new cert means a new fingerprint,
    // so even if the peer retains our old entry it will reject future connections.
    settings.p2pIdentity = null
}

// ─────────────────────────────────────────────────────────────────────────────
// Activity
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Devices screen — shows the currently-paired P2P peer (if any) as a card with
 * an online status dot, sync address, fingerprint, and Unpair / Revoke actions.
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

    // Refresh the peer state every poll interval so the online dot updates.
    var peer by remember { mutableStateOf(PairedPeerInfo.fromSettings(settings)) }
    var ownIdentity by remember { mutableStateOf(settings.p2pIdentity) }

    LaunchedEffect(Unit) {
        while (true) {
            delay(PEER_POLL_MS)
            peer = PairedPeerInfo.fromSettings(settings)
            ownIdentity = settings.p2pIdentity
        }
    }

    var showUnpairDialog by remember { mutableStateOf(false) }
    var showRevokeDialog by remember { mutableStateOf(false) }

    // ── Unpair confirmation ──────────────────────────────────────────────────
    if (showUnpairDialog) {
        AlertDialog(
            onDismissRequest = { showUnpairDialog = false },
            title = { Text("Forget paired device?") },
            text = {
                Text(
                    "This device will no longer sync with the paired Mac over P2P. " +
                    "You can re-pair at any time by scanning a new QR code."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    showUnpairDialog = false
                    unpairPeer(settings)
                    peer = PairedPeerInfo.fromSettings(settings)
                }) { Text("Forget", color = IdeDanger) }
            },
            dismissButton = {
                TextButton(onClick = { showUnpairDialog = false }) { Text("Cancel") }
            },
        )
    }

    // ── Revoke confirmation ──────────────────────────────────────────────────
    if (showRevokeDialog) {
        AlertDialog(
            onDismissRequest = { showRevokeDialog = false },
            title = { Text("Revoke pairing?") },
            text = {
                Text(
                    "This will forget the paired device AND regenerate your P2P identity. " +
                    "The peer will no longer be able to connect to you. " +
                    "You will need to re-pair with a fresh QR scan."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    showRevokeDialog = false
                    revokePeer(settings)
                    peer = PairedPeerInfo.fromSettings(settings)
                    ownIdentity = settings.p2pIdentity
                }) { Text("Revoke", color = IdeDanger) }
            },
            dismissButton = {
                TextButton(onClick = { showRevokeDialog = false }) { Text("Cancel") }
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

            // ── Paired peer ─────────────────────────────────────────────────
            SectionLabel("Paired Device")

            if (peer != null) {
                PeerCard(
                    peer = peer!!,
                    onUnpair = { showUnpairDialog = true },
                    onRevoke = { showRevokeDialog = true },
                )
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

// ─────────────────────────────────────────────────────────────────────────────
// Peer card
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun PeerCard(
    peer: PairedPeerInfo,
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
            // ── Header row: dot + status ────────────────────────────────────
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
                    text = if (online) "Online" else "Offline",
                    color = dotColor,
                    style = MaterialTheme.typography.labelMedium,
                )
            }

            Spacer(Modifier.height(10.dp))

            // ── Fingerprint ─────────────────────────────────────────────────
            DeviceField(label = "Fingerprint", value = peer.fingerprint)

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

/** Poll cadence for refreshing peer state on the Devices screen. */
private const val PEER_POLL_MS = 10_000L
