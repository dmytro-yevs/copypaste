package com.copypaste.android

import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.RadiusChip

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
    onUnpair: () -> Unit,
    onRevoke: () -> Unit,
) {
    val c = LocalIdeColors.current
    // PG-37 parity: offline status dot uses danger (red) to match the macOS
    // DeviceCard offline indicator (was c.faint/grey, which diverged).
    val dotColor = if (online) c.success else c.danger
    val chip = transportChipFor(peer)

    // Row content only — the enclosing CopyPasteCard provides the glass surface,
    // 12dp radius, and 1dp hairline border (PARITY-SPEC §8 grouped inset list).
    Column(modifier = Modifier.padding(16.dp)) {
        // ── Header row: pulse dot + name + status + transport chip ───────
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // §7 online pulse ring (replaces plain dot).
            PulseDot(online = online, modifier = Modifier.size(10.dp))
            Text(
                text = peer.name.ifBlank { "Paired device" },
                color = c.text,
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(
                text = if (online) "Online" else "Offline",
                color = dotColor,
                style = MaterialTheme.typography.labelMedium,
            )
            // §7 transport chip: P2P (info) or Cloud (accent).
            TransportChipLabel(chip = chip)
        }

        Spacer(Modifier.height(6.dp))

        // mgkr (NG-3): Verified trust badge — all persisted peers completed SAS
        // confirmation before roster insertion. Surface this explicitly via a
        // green "Verified" chip using success token colours + RadiusChip shape
        // (4 dp — PARITY-SPEC §4 chip radius) so it adapts across skins without
        // hard-coding a value. Parity with the web DeviceCard trust badge.
        Text(
            text = trustLabel(peer),
            color = c.success,
            fontSize = 10.sp,
            letterSpacing = 0.4.sp,
            style = MaterialTheme.typography.labelSmall,
            modifier = Modifier
                .background(c.success.copy(alpha = 0.14f), RadiusChip)
                .border(
                    width = 1.dp,
                    color = c.success.copy(alpha = 0.30f),
                    shape = RadiusChip,
                )
                .padding(horizontal = 6.dp, vertical = 2.dp),
        )

        Spacer(Modifier.height(8.dp))

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

        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
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
            // PG-45: show truncated peer fingerprint so the user can verify the
            // peer's identity inline — mirrors macOS DeviceCard which shows a
            // truncated fingerprint in the MetaGrid. Format: first16…last8.
            // formatPeerFingerprint() is the shared helper at the top of this file.
            peer.fingerprint.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Fingerprint", value = formatPeerFingerprint(it))
            }
        }

        // CopyPaste-g4ze: reduce divider gap (vertical 12dp → top 10 / bottom 8) to avoid
        // disproportionate spacing between the metadata table and the action buttons.
        HorizontalDivider(
            modifier = Modifier.padding(top = 10.dp, bottom = 8.dp),
            color = c.divider,
            thickness = 1.dp,
        )

        // ── Actions ─────────────────────────────────────────────────────
        // CopyPaste-jkbo: replaced raw M3 Button/ButtonDefaults with shared
        // CopyPasteButton(DANGER) which applies the styleguide bg=danger@15%,
        // fg=danger recipe automatically (matching web spec §7).
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
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
    val c = LocalIdeColors.current
    CopyPasteCard(accent = c.border) {
        Row(
            modifier = Modifier.padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Discovery rings icon — concentric ping rings around the network symbol.
            // Mirrors styleguide `networkRing` keyframe: scale .78→1.35, opacity .5→0,
            // 2.7 s × motionScale loop; second ring delayed by 1.1 s × motionScale.
            DiscoveryRingsIcon(size = 52.dp)

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = "No device paired",
                    color = c.dim,
                    style = MaterialTheme.typography.bodyLarge,
                )
                Text(
                    text = "Pair with a Mac running CopyPaste to enable P2P clipboard sync over your local network.",
                    color = c.faint,
                    style = MaterialTheme.typography.bodySmall,
                )
                // CopyPaste-jkbo: replaced raw M3 Button with CopyPasteButton(PRIMARY).
                CopyPasteButton(onClick = onPair, variant = ButtonVariant.PRIMARY) {
                    Text("Pair a device")
                }
            }
        }
    }
}

/**
 * Network icon with two concentric discovery-ping rings animated outward.
 * Mirrors the styleguide `.empty-icon::before/::after` + `networkRing` keyframe:
 *   scale 0.78 → 1.35, opacity 0.5 → 0, ease-out, 2.7 s × motionScale loop.
 * The second ring is delayed by 1.1 s × motionScale to stagger the pulses.
 * Both rings are tinted [accent2] (styleguide `.empty-icon::before` uses accent-2).
 * Gated on system reduced-motion.
 */
@Composable
internal fun DiscoveryRingsIcon(size: Dp = 58.dp) {
    val c = LocalIdeColors.current
    // Discovery rings removed — static icon is calmer (no idle loop animation).
    Box(
        modifier = Modifier.size(size),
        contentAlignment = Alignment.Center,
    ) {
        // Icon surface — glass-tinted rounded square with network symbol (text).
        Box(
            modifier = Modifier
                .size(size)
                .clip(RoundedCornerShape(size / 3.5f))
                .background(c.accentDim),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = "⊕",
                color = c.accent,
                fontSize = (size.value * 0.45f).sp,
            )
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
    val c = LocalIdeColors.current
    val model = Build.MODEL.orEmpty().ifBlank { "Android" }
    val osVersion = "Android " + Build.VERSION.RELEASE
    val appVersion = BuildConfig.VERSION_NAME

    // Live local IP — re-read every ~5 s (keyed on nowMs / 5000) so a network
    // change (Wi-Fi handoff, VPN connect) is reflected promptly.
    // The bare `remember { lanIpv4Address() }` snapshot was stale on network
    // change because it was only evaluated once at first composition.
    val localIp = remember(nowMs / 5_000L) { lanIpv4Address() }

    // Badge float removed — static badge is calmer and more professional.

    // Row content only — the enclosing CopyPasteCard provides the glass surface
    // (PARITY-SPEC §8 grouped inset list).
    Column(modifier = Modifier.padding(16.dp)) {
        // Header: §7 pulse dot (always online) + model name + "Online"
        // + §7 "This Device" accent badge (parity with macOS "This Mac").
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // Own device is always online — pulse ring always animates (unless
            // reduced motion is enabled).
            PulseDot(online = true, modifier = Modifier.size(10.dp))
            Text(
                text = model,
                color = c.text,
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(
                text = "Online",
                color = c.success,
                style = MaterialTheme.typography.labelMedium,
            )
            // §7 "This Device" accent badge — static (float animation removed).
            // CopyPaste-5917.44: was RoundedCornerShape(4.dp); canonical chip token is RadiusChip (7dp).
            Text(
                text = "This Device",
                color = c.accent,
                fontSize = 10.sp,
                letterSpacing = 0.4.sp,
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier
                    .background(c.accentDim, RadiusChip)
                    .padding(horizontal = 6.dp, vertical = 2.dp),
            )
        }

        Spacer(Modifier.height(10.dp))

        // Two-column aligned table — same [META_LABEL_WIDTH] as PeerRow.
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
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
    val c = LocalIdeColors.current
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
                    color = c.text,
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
                color = c.faint,
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
        }
    }
}
