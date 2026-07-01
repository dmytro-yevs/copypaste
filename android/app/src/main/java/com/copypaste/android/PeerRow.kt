package com.copypaste.android

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard

// ─────────────────────────────────────────────────────────────────────────────
// Grouped inset list rows (PARITY-SPEC §8)
// ─────────────────────────────────────────────────────────────────────────────

/** Display label for a peer: its name when set, else a short fingerprint. */
internal fun PairedPeer.displayName(): String =
    name.ifBlank { "device ${fingerprint.take(8)}" }

/** Short label for a discovered peer: name when set, else a short device id. */
internal fun DiscoveredPeer.displayName(): String =
    deviceName.ifBlank { "Device ${deviceId.take(8)}" }

// CopyPaste-jkbo: promoted from private to internal so future screens can reuse.
@Composable
internal fun PeerRow(
    peer: PairedPeer,
    /**
     * Pre-computed online flag from [DevicesScreen] — the SINGLE source of truth
     * for this peer's online/offline state. Replaces the former per-card call to
     * [PairedPeer.isOnline] which diverged from the footer badge computation.
     */
    online: Boolean,
    /** Current epoch millis from the 1-second ticker in [DevicesScreen]. */
    nowMs: Long,
    /**
     * CopyPaste-crh3.30: the device's active secondary (non-P2P) transport, used
     * to label a cloud-only peer Relay vs Cloud. Derived once in [DevicesScreen]
     * from [Settings] via [activeCloudTransport]. Defaults to
     * [CloudTransport.NONE] (preserves the old P2P-vs-Cloud heuristic).
     */
    cloudTransport: CloudTransport = CloudTransport.NONE,
    onUnpair: () -> Unit,
    onRevoke: () -> Unit,
) {
    // PG-37 parity: offline status dot uses danger (red) to match the macOS
    // DeviceCard offline indicator (was c.faint/grey, which diverged).
    val dotColor = if (online) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error
    val chip = transportChipFor(peer, cloudTransport)

    // Row content only — the enclosing CopyPasteCard provides the surface.
    Column {
        // ── Header row: pulse dot + name + status + transport chip ───────
        Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // §7 online pulse ring (replaces plain dot).
            PulseDot(online = online)
            Text(
                text = peer.name.ifBlank { "Paired device" },
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(
                text = if (online) "Online" else "Offline",
                color = dotColor,
            )
            // §7 transport chip: P2P (info) or Cloud (accent).
            TransportChipLabel(chip = chip)
        }

        // mgkr (NG-3): Verified trust badge — all persisted peers completed SAS
        // confirmation before roster insertion. Surface this explicitly via a
        // "Verified" chip. Parity with the web DeviceCard trust badge.
        Text(text = trustLabel(peer))

        // ── Two-column aligned table ─────────────────────────────────────
        // Label column is [META_LABEL_WIDTH] wide; value column takes the
        // rest. Each row uses verticalAlignment = CenterVertically so
        // multi-line values don't cause the label to sit misaligned.
        // Only rows with non-blank values rendered — legacy pre-ABI-14
        // roster entries simply show fewer rows.
        val lastSyncText: String? = if (peer.lastSyncMs > 0L) {
            val elapsed = (nowMs - peer.lastSyncMs) / 1_000L
            when {
                elapsed < 60 -> "${elapsed}s ago"
                elapsed < 3600 -> "${elapsed / 60}m ago"
                elapsed < 86400 -> "${elapsed / 3600}h ago"
                else -> formatEpochMs(peer.lastSyncMs)
            }
        } else null

        Column {
            peer.peerModel?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Model", value = it)
            }
            peer.peerOs?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "OS", value = it)
            }
            peer.peerAppVersion?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Version", value = it)
            }
            // PG-39: show peerLocalIp when present, else fall back to the host
            // portion of syncAddr — mirrors macOS DeviceCard.tsx:215
            //   `peer.local_ip ?? extractIp(peer.address)`.
            // syncAddrToIp() strips the port (handles IPv4 and [IPv6]:port).
            val localIpDisplay = peer.peerLocalIp?.takeIf { it.isNotBlank() }
                ?: syncAddrToIp(peer.syncAddr)
            localIpDisplay?.let {
                MetaRow(label = "Local IP", value = it)
            }
            peer.peerPublicIp?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Public IP", value = it)
            }
            if (peer.pairedAtMs > 0L) {
                MetaRow(label = "Paired", value = formatEpochMs(peer.pairedAtMs))
            }
            lastSyncText?.let {
                MetaRow(label = "Last sync", value = it)
            }
            // RTT: shown when FgsSyncLoop has measured a live round-trip time.
            // FgsSyncLoop instrumentation (Ping/Pong over mTLS) deferred to CopyPaste-8dd.
            peer.latencyMs?.let {
                MetaRow(label = "RTT", value = "$it ms")
            }
            // PG-45 / CopyPaste-crh3.45: show the truncated peer fingerprint so the
            // user can verify a peer's identity inline at ANY time. NOTE: this is an
            // Android-specific superset — macOS shows the fingerprint only in the SAS
            // pairing modal (SasPairingModal.tsx), NOT in its device card. Android
            // surfaces it in BOTH the SAS dialog (CopyPaste-crh3.29) and here, so a
            // user can re-verify after pairing. (Earlier comment falsely claimed this
            // mirrors macOS DeviceCard — it does not.) Format: first16…last8 via the
            // shared formatPeerFingerprint() helper at the top of this file.
            peer.fingerprint.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Fingerprint", value = formatPeerFingerprint(it))
            }
        }

        HorizontalDivider()

        // ── Actions ─────────────────────────────────────────────────────
        // CopyPaste-jkbo: replaced raw M3 Button/ButtonDefaults with shared
        // CopyPasteButton(DANGER) which applies the styleguide bg=danger@15%,
        // fg=danger recipe automatically (matching web spec §7).
        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            CopyPasteButton(
                onClick = onUnpair,
                variant = ButtonVariant.DANGER,
                modifier = Modifier.weight(1f),
            ) {
                Text("Unpair")
            }
            CopyPasteButton(
                onClick = onRevoke,
                variant = ButtonVariant.DANGER,
                modifier = Modifier.weight(1f),
            ) {
                Text("Revoke")
            }
        }
    }
}

@Composable
internal fun NoPeerCard(onPair: () -> Unit) {
    CopyPasteCard {
        Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Discovery icon removed (de-style pass) — text-only empty state.
            Column {
                Text(text = "No device paired")
                Text(text = "Pair with a Mac running CopyPaste to enable P2P clipboard sync over your local network.")
                // CopyPaste-jkbo: replaced raw M3 Button with CopyPasteButton(PRIMARY).
                CopyPasteButton(onClick = onPair, variant = ButtonVariant.PRIMARY) {
                    Text("Pair a device")
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Own-device row
// ─────────────────────────────────────────────────────────────────────────────

// CopyPaste-jkbo: promoted from private to internal so future screens can reuse.
@Composable
internal fun OwnDeviceRow(
    identity: P2pIdentity,
    /** Current epoch millis from the 1-second ticker — drives live IP refresh. */
    nowMs: Long,
    /** Public IP from a STUN lookup (CopyPaste-6qq1), null when not yet resolved. */
    ownPublicIp: String? = null,
) {
    // HB-1c: render THIS device's info at parity with the macOS "This Mac" card.
    // ABI 14 sends these same fields to peers (own gather in PairActivity /
    // DevicesActivity startPairing); we surface them locally too. Gathered live —
    // P2pIdentity only carries the id/fingerprint, the rest comes from the
    // platform (Build/BuildConfig) and a LAN-IPv4 enumeration. No synchronous
    // public-IP source on-device, so that row is omitted (matches the bootstrap
    // path, which sends public_ip = None for this device).
    val model = Build.MODEL.orEmpty().ifBlank { "Android" }
    val osVersion = "Android " + Build.VERSION.RELEASE
    val appVersion = BuildConfig.VERSION_NAME

    // Live local IP — re-read every ~5 s (keyed on nowMs / 5000) so a network
    // change (Wi-Fi handoff, VPN connect) is reflected promptly.
    // The bare `remember { lanIpv4Address() }` snapshot was stale on network
    // change because it was only evaluated once at first composition.
    val localIp = remember(nowMs / 5_000L) { lanIpv4Address() }

    // Badge float removed — static badge is calmer and more professional.

    // Row content only — the enclosing CopyPasteCard provides the surface.
    Column {
        // Header: §7 pulse dot (always online) + model name + "Online"
        // + §7 "This Device" accent badge (parity with macOS "This Mac").
        Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Own device is always online — pulse ring always animates (unless
            // reduced motion is enabled).
            PulseDot(online = true)
            Text(
                text = model,
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(text = "Online")
            // §7 "This Device" accent badge — static (float animation removed).
            Text(text = "This Device")
        }

        // Two-column-ish table — same rows as PeerRow.
        Column {
            MetaRow(label = "Model", value = model)
            MetaRow(label = "OS", value = osVersion)
            MetaRow(label = "Version", value = appVersion)
            localIp?.let { MetaRow(label = "Local IP", value = it) }
            // CopyPaste-6qq1: show Public IP from async STUN lookup when available.
            ownPublicIp?.let { MetaRow(label = "Public IP", value = it) }
            // CopyPaste-0tb0: show own fingerprint — mirrors macOS ThisDeviceCard.
            // Full fingerprint displayed (no truncation) so the user can verify identity.
            identity.fingerprint.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Fingerprint", value = it)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Discovered-peer row (LAN, unpaired)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * One discovered (unpaired) LAN device row with a Pair button. Mirrors the macOS
 * DiscoveredRow: the Pair button is DISABLED when the peer advertises no
 * bootstrap port ([DiscoveredPeer.bport] == null) — a v1 peer that cannot do SAS
 * pairing — or while another pairing is in flight ([busy]).
 */
@Composable
internal fun DiscoveredPeerRow(
    peer: DiscoveredPeer,
    busy: Boolean,
    onPair: () -> Unit,
) {
    // v1 peers (no bootstrap port) cannot do SAS pairing → disable Pair.
    val pairable = peer.bport != null
    // CopyPaste-cnmw: show ALL discovered IPs (macOS merges/shows all) instead of
    // only firstOrNull(). When multiple interfaces advertise the peer we join them
    // with ", " so the user can see every reachable address.
    val ips = peer.ipAddrs

    // Row content only — the enclosing CopyPasteCard provides the glass surface
    // (PARITY-SPEC §8 grouped inset list).
    Column {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Discovery icon with concentric rings — signals "device nearby, tap to pair".
            DiscoveryRingsIcon(size = 40.dp)

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = peer.displayName(),
                    style = MaterialTheme.typography.titleSmall,
                )
                Spacer(Modifier.height(4.dp))
                // CopyPaste-cnmw: show all IPs joined, matching macOS parity.
                // Each IP shown on its own MetaRow so long multi-IP lists wrap cleanly.
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (ips.isNotEmpty()) {
                        MetaRow(
                            label = stringResource(R.string.meta_label_local_ip),
                            value = ips.joinToString(", "),
                        )
                    }
                }
            }
            // CopyPaste-jkbo: replaced raw M3 Button with CopyPasteButton(PRIMARY).
            CopyPasteButton(
                onClick = onPair,
                enabled = pairable && !busy,
                variant = ButtonVariant.PRIMARY,
            ) {
                Text("Pair")
            }
        }
        if (!pairable) {
            Text(
                text = "This device does not support secure pairing.",
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
        }
    }
}
