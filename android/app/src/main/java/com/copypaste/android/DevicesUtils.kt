package com.copypaste.android

import android.util.Log
import java.text.DateFormat
import java.util.Date
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.SkinBackground

// ─────────────────────────────────────────────────────────────────────────────
// §7 Liquid Glass Devices parity — pure logic helpers (testable without SDK)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Transport chip variants shown on each peer card.
 * P2P = direct local network; Cloud = relay/Supabase.
 */
internal enum class TransportChip { P2P, Cloud }

/**
 * Derive the transport chip for [peer]:
 * - P2P when [PairedPeer.syncAddr] or [PairedPeer.peerLocalIp] is non-blank,
 *   meaning we have a local-network address for this peer.
 * - Cloud otherwise (relay or Supabase-only peer).
 *
 * Defensive: never throws on null/blank fields.
 */
internal fun transportChipFor(peer: PairedPeer): TransportChip =
    if (peer.syncAddr.isNotBlank() || peer.peerLocalIp?.isNotBlank() == true)
        TransportChip.P2P
    else
        TransportChip.Cloud

/**
 * Format the own-device fingerprint: always shown in full (no truncation).
 * Mirrors §7 "full fingerprint+copy on own".
 */
internal fun formatOwnFingerprint(fp: String): String = fp

/**
 * Format a peer fingerprint: take(16)+"…"+takeLast(8).
 * Mirrors §7 "16…8 truncated+hover-copy on peers".
 */
internal fun formatPeerFingerprint(fp: String): String =
    fp.take(16) + "…" + fp.takeLast(8)

/**
 * QR countdown drain-bar progress in [0f, 1f].
 * [remainingSeconds] / [totalSeconds], clamped to [0f, 1f].
 */
internal fun qrCountdownProgress(remainingSeconds: Int, totalSeconds: Int): Float =
    (remainingSeconds.toFloat() / totalSeconds.toFloat()).coerceIn(0f, 1f)

/**
 * QR countdown urgency threshold. PARITY-SPEC §10 / audit #26: the bar + label
 * switch from accent → warning at ≤20 s remaining (was 15 s, faint→warning).
 */
internal const val DEVICES_QR_URGENT_THRESHOLD_SECONDS = 20

/**
 * True when the QR is in the warning zone (≤20 s remaining).
 * Matches [DEVICES_QR_URGENT_THRESHOLD_SECONDS] (PARITY-SPEC §10 / audit #26).
 */
internal fun isQrWarning(remainingSeconds: Int): Boolean =
    remainingSeconds <= DEVICES_QR_URGENT_THRESHOLD_SECONDS

/**
 * True when the PulseDot should animate: [online] && ![ reducedMotion].
 * Extracted so unit tests can verify the gate without Compose.
 */
internal fun shouldPulse(online: Boolean, reducedMotion: Boolean): Boolean =
    online && !reducedMotion

/**
 * True when the aurora animated canvas should be painted as the screen backdrop.
 *
 * Gating rules (A-C2):
 *  - [background] must be [SkinBackground.AURORA] (Classic only).
 *  - [translucent] must be true (user pref; same gate as before).
 *  - [paintCanvasBackdrop] must be true (standalone vs. embedded gate; same as before).
 *
 * Classic keeps the SAME condition as before — byte-identical output.
 * Quiet (FLAT) and Vapor (TINT_BLOB) return false here.
 *
 * Extracted so it can be unit-tested without the Compose runtime.
 */
internal fun shouldPaintAurora(
    background: SkinBackground,
    translucent: Boolean,
    paintCanvasBackdrop: Boolean,
): Boolean = background == SkinBackground.AURORA && translucent && paintCanvasBackdrop

/**
 * True when a static tinted blob should be painted as the screen backdrop.
 *
 * Gating rules (A-C2):
 *  - [background] must be [SkinBackground.TINT_BLOB] (Vapor only).
 *  - [translucent] must be true (same pref gate as aurora).
 *  - [paintCanvasBackdrop] must be true (standalone vs. embedded gate).
 *
 * Extracted so it can be unit-tested without the Compose runtime.
 */
internal fun shouldPaintTintBlob(
    background: SkinBackground,
    translucent: Boolean,
    paintCanvasBackdrop: Boolean,
): Boolean = background == SkinBackground.TINT_BLOB && translucent && paintCanvasBackdrop

/**
 * CopyPaste-mgkr / CopyPaste-1jms.4 (NG-3): trust label for a paired peer.
 *
 * Returns "Verified" only when [PairedPeer.sasVerified] is true — meaning the
 * peer was admitted through the SAS (Short Authentication String) flow that
 * proves absence of a man-in-the-middle. All historical roster entries default
 * to sasVerified=true for backward-compatibility.
 *
 * Peers admitted by any other mechanism (cloud-import, admin provisioning, etc.)
 * have sasVerified=false and receive "Unverified" so users can distinguish them.
 *
 * Extracted as a pure function for unit-testability without the Compose runtime.
 */
internal fun trustLabel(peer: PairedPeer): String =
    if (peer.sasVerified) "Verified" else "Unverified"

/**
 * Forget a single paired peer locally: remove its roster entry (fingerprint,
 * sync address, KEK-wrapped session key).
 *
 * CopyPaste-1jms.8: Android cannot send a mutual unpair signal to the peer
 * because the Android app has no live mTLS channel management equivalent to the
 * macOS daemon's `send_unpair_signal_if_connected()` / `queue_unpair_for_offline_delivery()`
 * (crates/copypaste-daemon/src/ipc.rs:998-1052). A `ControlMsg::Unpair` would
 * need:
 *   (a) a persistent mTLS connection handle to the peer, OR
 *   (b) a durable pending-unpair queue flushed on next P2P dial.
 * Neither exists on Android yet — the P2P dialer (syncWithPeer FFI) is
 * one-shot pull, not a live connection.
 *
 * As a result the revoked peer continues trying to sync until it is also
 * unpaired on its side (or times out). This is tracked as a known limitation.
 * Backend support required: expose a "queue_unpair" IPC call or durable
 * pending-action table that FgsSyncLoop can flush on next dial.
 *
 * Does NOT touch this device's P2P identity (cert/key) — we keep our own
 * identity so our OTHER pairings keep working and re-pairing needs no new cert.
 */
fun unpairPeer(settings: Settings, fingerprint: String) {
    settings.removePeer(fingerprint)
    // CopyPaste-1jms.8: local removal is all we can do on Android today.
    // Log the limitation so it is visible in diagnostics; the revoked peer
    // will continue dialling until it is also unpaired on its own side.
    Log.w(
        "DevicesActivity",
        "unpairPeer: peer ${fingerprint.take(16)}… removed locally. " +
            "No unpair signal sent — Android lacks a durable pending-unpair queue " +
            "(backend support needed: see CopyPaste-1jms.8).",
    )
}

/**
 * Fixed width of the label column in the two-column metadata table.
 * Sized to fit the longest label ("Local IP" / "Public IP") at 11 sp so
 * values in all three row types (Own, Peer, Discovered) start at the same
 * horizontal position regardless of which row they appear in.
 */
internal val META_LABEL_WIDTH: Dp = 72.dp

/**
 * Single 1dp hairline between rows in the grouped inset device list
 * (PARITY-SPEC §4 / §8 — kills the former 0.5dp mix). Inset on the leading edge
 * to read as an Apple grouped-list separator.
 */
@Composable
internal fun RowDivider() {
    val c = LocalIdeColors.current
    HorizontalDivider(
        modifier = Modifier.padding(start = 16.dp),
        color = c.divider,
        thickness = 1.dp,
    )
}

/**
 * Two-column aligned table row used in device rows.
 *
 * The label column is [META_LABEL_WIDTH] wide (fixed) so all labels in
 * OwnDeviceRow, PeerRow, and DiscoveredPeerRow start at the same horizontal
 * offset. Both text nodes are vertically centred within the row
 * (verticalAlignment = Alignment.CenterVertically) so multi-line values don't
 * cause the label to sit misaligned — fixing the former "Mac" misalignment in
 * the Model row.
 */
// CopyPaste-jkbo: promoted from private to internal so future screens can reuse.
@Composable
internal fun MetaRow(label: String, value: String) {
    val c = LocalIdeColors.current
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = c.dim,
            fontSize = 11.sp,
            modifier = Modifier.width(META_LABEL_WIDTH),
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = MonoFontFamily),
            color = c.text,
            fontSize = 11.sp,
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * Format a Unix epoch-millisecond timestamp as a short locale date+time string
 * for device-info fields. Returns "—" for zero / negative values (unknown).
 * Mirrors macOS formatEpochSecs (which uses toLocaleString()).
 */
internal fun formatEpochMs(ms: Long): String {
    if (ms <= 0L) return "—"
    return DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT)
        .format(Date(ms))
}

/** Extract the host part from a "host:port" sync address, or return the full string. */
internal fun syncAddrToIp(syncAddr: String): String? {
    if (syncAddr.isBlank()) return null
    // IPv6: [::1]:4242 → ::1; IPv4: 192.168.1.2:4242 → 192.168.1.2
    val v6 = Regex("""^\[(.+)]:\d+$""").find(syncAddr)
    if (v6 != null) return v6.groupValues[1]
    val colon = syncAddr.lastIndexOf(':')
    return if (colon > 0) syncAddr.substring(0, colon) else syncAddr
}

/** Poll cadence for refreshing peer state on the Devices screen. */
internal const val PEER_POLL_MS = 10_000L

/** Poll cadence for refreshing the LAN-discovered peer list (~2 s). */
internal const val DISCOVERED_POLL_MS = 2_000L

/** Poll cadence for the SAS pairing state machine (~500 ms). */
internal const val SAS_POLL_MS = 500L

/**
 * Fixed bootstrap (SAS-pairing) listener port this device advertises in its mDNS
 * TXT record so peers can dial back to pair. A non-zero bport marks this device
 * SAS-pairing-capable (v2); the native discovery service binds/owns this port.
 */
// `internal` so the always-on [ClipboardService] FGS owns the discovery
// lifecycle with the SAME well-known bport (HB-2).
internal const val SAS_BPORT = 47_654
