package com.copypaste.android

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CpBadgeChip
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.EmptyStateCard
import com.copypaste.android.ui.theme.LocalAccent
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons
import com.copypaste.android.ui.theme.relativeTimeAgoLabel

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
    /**
     * android-devices spec "Fingerprint tap-to-copy parity" — copies the FULL
     * 64-hex fingerprint. This card has no [androidx.compose.ui.platform.LocalClipboardManager]
     * dependency of its own; the toast/clipboard side effect lives in
     * [DevicesScreen], mirroring [com.copypaste.android.PairedPeerList]'s
     * `onCopyFingerprint` callback shape.
     */
    onCopyFingerprint: (String) -> Unit,
) {
    val cp = LocalCpColors.current
    val chip = transportChipFor(peer, cloudTransport)

    // Row content only — the enclosing CopyPasteCard provides the surface.
    Column {
        // ── Header row: pulse dot + name + status + transport chip ───────
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(CpSpacing.s4),
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            // §7 online pulse ring (replaces plain dot).
            PulseDot(online = online)
            Text(
                text = peer.name.ifBlank { stringResource(R.string.devices_peer_default_name) },
                style = CpTypography.bodyEmphasis,
                color = cp.text,
                modifier = Modifier.weight(1f, fill = false),
            )
            // Presence is never color-only: the dot above is paired with this label.
            Text(
                text = if (online) stringResource(R.string.devices_status_online) else stringResource(R.string.devices_status_offline),
                style = CpTypography.meta,
                color = if (online) cp.ok else cp.err,
            )
            // §7 / §9.4 transport pill: P2P (info) / Relay (warn) / Cloud (accent).
            TransportChipLabel(chip = chip)
        }

        // mgkr (NG-3): Verified trust badge (§9.4 hairline chip + dot) — all
        // persisted peers completed SAS confirmation before roster insertion.
        // Parity with the web DeviceCard trust badge.
        Row(modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
            val verified = peer.sasVerified
            CpBadgeChip(
                text = trustLabel(peer),
                color = if (verified) cp.ok else cp.warn,
                pill = false,
                showDot = true,
            )
        }

        // ── §9.7 field grid ─────────────────────────────────────────────
        // Baseline-aligned label/value rows ([MetaRow]). Only rows with a
        // genuinely-absent value are omitted — legacy pre-ABI-14 roster
        // entries simply show fewer rows. RTT is the one exception: it always
        // renders, with an em-dash placeholder when no live P2P link has ever
        // measured a round trip (android-devices spec "RTT shows a placeholder
        // without a live P2P link").
        val lastSyncElapsedMs = nowMs - peer.lastSyncMs
        val lastSyncText: String? = if (peer.lastSyncMs > 0L) {
            if (lastSyncElapsedMs >= 86_400_000L) formatEpochMs(peer.lastSyncMs)
            else relativeTimeAgoLabel(lastSyncElapsedMs)
        } else null

        Column(modifier = Modifier.padding(horizontal = 16.dp)) {
            peer.peerModel?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = stringResource(R.string.meta_label_model), value = it)
            }
            peer.peerOs?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = stringResource(R.string.meta_label_os), value = it)
            }
            peer.peerAppVersion?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = stringResource(R.string.meta_label_version), value = it)
            }
            // PG-39: show peerLocalIp when present, else fall back to the host
            // portion of syncAddr — mirrors macOS DeviceCard.tsx:215
            //   `peer.local_ip ?? extractIp(peer.address)`.
            // syncAddrToIp() strips the port (handles IPv4 and [IPv6]:port).
            val localIpDisplay = peer.peerLocalIp?.takeIf { it.isNotBlank() }
                ?: syncAddrToIp(peer.syncAddr)
            localIpDisplay?.let {
                MetaRow(label = stringResource(R.string.meta_label_local_ip), value = it)
            }
            peer.peerPublicIp?.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = stringResource(R.string.meta_label_public_ip), value = it)
            }
            if (peer.pairedAtMs > 0L) {
                MetaRow(label = stringResource(R.string.meta_label_paired), value = formatEpochMs(peer.pairedAtMs))
            }
            lastSyncText?.let {
                MetaRow(label = stringResource(R.string.meta_label_last_sync), value = it)
            }
            // RTT: always rendered — "—" until FgsSyncLoop measures a live
            // round-trip time (Ping/Pong over mTLS, CopyPaste-8dd).
            MetaRow(
                label = stringResource(R.string.meta_label_rtt),
                value = peer.latencyMs?.let { stringResource(R.string.meta_label_rtt_value, it) } ?: EM_DASH,
            )
            // PG-45 / CopyPaste-crh3.45: Android-specific superset field — macOS
            // shows the fingerprint only in the SAS pairing modal, not the device
            // card; Android also surfaces it here so a user can re-verify a
            // peer's identity at any time. Tap-to-copy (android-devices spec):
            // truncated first16…last8 display, full 64-hex value on the
            // clipboard.
            peer.fingerprint.takeIf { it.isNotBlank() }?.let { fp ->
                MetaRow(
                    label = stringResource(R.string.meta_label_fingerprint),
                    value = formatPeerFingerprint(fp),
                    onClick = { onCopyFingerprint(fp) },
                    onClickLabel = stringResource(R.string.cd_copy_fingerprint),
                )
            }
        }

        HorizontalDivider(modifier = Modifier.padding(top = 12.dp))

        // ── §9.9 danger footer — equal-width Unpair / Revoke ───────────────
        // CopyPaste-jkbo: replaced raw M3 Button/ButtonDefaults with shared
        // CopyPasteButton(DANGER) which applies the styleguide bg=danger@9%,
        // fg=danger recipe automatically (matching web spec §9.1).
        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            CopyPasteButton(
                onClick = onUnpair,
                variant = ButtonVariant.DANGER,
                modifier = Modifier.weight(1f),
            ) {
                Text(stringResource(R.string.btn_unpair))
            }
            CopyPasteButton(
                onClick = onRevoke,
                variant = ButtonVariant.DANGER,
                modifier = Modifier.weight(1f),
            ) {
                Text(stringResource(R.string.btn_revoke))
            }
        }
    }
}

/**
 * §9.10 empty state (icon + headline + hint) shown instead of a paired-peer
 * list when the roster is empty — distinct from the LAN "scanning" state
 * rendered separately by [DevicesScreen]'s discovered-peers section (see
 * android-devices spec "Empty state when no peers are paired").
 */
@Composable
internal fun NoPeerCard(onPair: () -> Unit) {
    val cp = LocalCpColors.current
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        EmptyStateCard(
            icon = {
                Icon(
                    imageVector = LucideIcons.NavDevices,
                    contentDescription = null,
                    tint = cp.faint,
                    modifier = Modifier.size(28.dp),
                )
            },
            title = stringResource(R.string.devices_no_peer_title),
            subtitle = stringResource(R.string.devices_no_peer_body),
            padding = PaddingValues(0.dp),
            reducedMotion = rememberReducedMotion(),
        )
        // CopyPaste-jkbo: replaced raw M3 Button with CopyPasteButton(PRIMARY).
        CopyPasteButton(onClick = onPair, variant = ButtonVariant.PRIMARY, modifier = Modifier.fillMaxWidth()) {
            Text(stringResource(R.string.devices_btn_pair_device))
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
    /** android-devices spec "Fingerprint tap-to-copy parity" — see [PeerRow]. */
    onCopyFingerprint: (String) -> Unit = {},
) {
    // HB-1c: render THIS device's info at parity with the macOS "This Mac" card.
    // ABI 14 sends these same fields to peers (own gather in PairActivity /
    // DevicesActivity startPairing); we surface them locally too. Gathered live —
    // P2pIdentity only carries the id/fingerprint, the rest comes from the
    // platform (Build/BuildConfig) and a LAN-IPv4 enumeration. No synchronous
    // public-IP source on-device, so that row is omitted (matches the bootstrap
    // path, which sends public_ip = None for this device).
    val cp = LocalCpColors.current
    val model = Build.MODEL.orEmpty().ifBlank { "Android" }
    val osVersion = "Android " + Build.VERSION.RELEASE
    val appVersion = BuildConfig.VERSION_NAME

    // Live local IP — re-read every ~5 s (keyed on nowMs / 5000) so a network
    // change (Wi-Fi handoff, VPN connect) is reflected promptly.
    // The bare `remember { lanIpv4Address() }` snapshot was stale on network
    // change because it was only evaluated once at first composition.
    val localIp = remember(nowMs / 5_000L) { lanIpv4Address() }

    // Row content only — the enclosing CopyPasteCard provides the surface.
    // android-devices spec "Own-device card field grid": no Unpair/Revoke
    // footer — this row represents the current device.
    Column {
        // Header: §7 pulse dot (always online) + model name + "Online"
        // + §9.4 "This device" accent-2 pill (parity with macOS "This Mac").
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(CpSpacing.s4),
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            // Own device is always online — pulse ring always animates (unless
            // reduced motion is enabled).
            PulseDot(online = true)
            Text(
                text = model,
                style = CpTypography.bodyEmphasis,
                color = cp.text,
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(
                text = stringResource(R.string.devices_status_online),
                style = CpTypography.meta,
                color = cp.ok,
            )
            // "This device" — accent-2 tint (STYLEGUIDE §9.4), not plain text.
            CpBadgeChip(
                text = stringResource(R.string.devices_badge_this_device),
                color = LocalAccent.current.variant,
                pill = true,
            )
        }

        // §9.7 six-field grid — same MetaRow mechanics as PeerRow.
        Column(modifier = Modifier.padding(horizontal = 16.dp)) {
            MetaRow(label = stringResource(R.string.meta_label_model), value = model)
            MetaRow(label = stringResource(R.string.meta_label_os), value = osVersion)
            MetaRow(label = stringResource(R.string.meta_label_version), value = appVersion)
            localIp?.let { MetaRow(label = stringResource(R.string.meta_label_local_ip), value = it) }
            // CopyPaste-6qq1: show Public IP from async STUN lookup when available.
            ownPublicIp?.let { MetaRow(label = stringResource(R.string.meta_label_public_ip), value = it) }
            // CopyPaste-0tb0: own fingerprint — mirrors macOS ThisDeviceCard, now
            // truncated + tap-to-copy for parity with the peer/roster surfaces
            // (android-devices spec "Fingerprint tap-to-copy parity" — this
            // supersedes the prior always-full-value display).
            identity.fingerprint.takeIf { it.isNotBlank() }?.let { fp ->
                MetaRow(
                    label = stringResource(R.string.meta_label_fingerprint),
                    value = formatPeerFingerprint(fp),
                    onClick = { onCopyFingerprint(fp) },
                    onClickLabel = stringResource(R.string.cd_copy_fingerprint),
                )
            }
        }
        Spacer(Modifier.height(4.dp))
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
    val cp = LocalCpColors.current
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
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = peer.displayName(),
                    style = CpTypography.bodyEmphasis,
                    color = cp.text,
                )
                Spacer(Modifier.height(4.dp))
                // CopyPaste-cnmw: show all IPs joined, matching macOS parity.
                // Each IP shown on its own MetaRow so long multi-IP lists wrap cleanly.
                Column(verticalArrangement = Arrangement.spacedBy(CpSpacing.s2)) {
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
                Text(stringResource(R.string.btn_pair))
            }
        }
        if (!pairable) {
            Text(
                text = stringResource(R.string.devices_no_secure_pairing),
                style = CpTypography.micro,
                color = cp.faint,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
        }
    }
}
