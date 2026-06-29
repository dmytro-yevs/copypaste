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

// ─────────────────────────────────────────────────────────────────────────────
// §7 Liquid Glass Devices parity — pure logic helpers (testable without SDK)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Transport chip variants shown on each peer card.
 *
 * P2P   = direct local network. Relay = the encrypted-blob relay (amber).
 * Cloud = Supabase (or an unknown secondary transport).
 *
 * CopyPaste-crh3.30: a 3-way chip at parity with macOS DeviceCard.tsx, which
 * distinguishes Relay from Supabase ("Cloud") because they have different
 * latency/privacy/cost. The old enum collapsed both into "Cloud".
 */
internal enum class TransportChip { P2P, Relay, Cloud }

/**
 * The active secondary (non-P2P) transport on THIS device, derived from local
 * sync configuration.
 *
 * On macOS the daemon owns this and surfaces a per-peer `transport` string over
 * IPC (handlers_peers.rs: live P2P sink > relay active > supabase active). Android
 * has no separate daemon — the app IS the sync engine — so the authoritative
 * source is the device's own sync config, and `transport` is NOT a persisted
 * field on [PairedPeer]. We compute it the same way macOS does: relay, then
 * Supabase, then none.
 */
internal enum class CloudTransport { RELAY, SUPABASE, NONE }

/**
 * Pick the active secondary transport, mirroring the macOS daemon priority
 * (relay is checked before Supabase). Pure so it is unit-testable on a plain JVM.
 *
 * @param relayActive    relay enabled AND configured ([Settings.relayEnabled] &&
 *                       [Settings.isRelayConfigured]).
 * @param supabaseActive supabase enabled AND configured ([Settings.supabaseEnabled]
 *                       && [Settings.isSupabaseConfigured]).
 */
internal fun activeCloudTransport(
    relayActive: Boolean,
    supabaseActive: Boolean,
): CloudTransport = when {
    relayActive -> CloudTransport.RELAY
    supabaseActive -> CloudTransport.SUPABASE
    else -> CloudTransport.NONE
}

/**
 * Derive the transport chip for [peer], at parity with the macOS authoritative
 * priority (live P2P > relay > supabase):
 * - P2P when [PairedPeer.syncAddr] or [PairedPeer.peerLocalIp] is non-blank,
 *   meaning we have a local-network address for this peer (takes precedence).
 * - Otherwise the device's [cloudTransport]: RELAY → Relay (amber), and
 *   SUPABASE / NONE → Cloud (Supabase or unknown secondary route).
 *
 * [cloudTransport] defaults to [CloudTransport.NONE], preserving the old
 * P2P-vs-Cloud heuristic for callers that do not yet thread the active transport.
 *
 * Defensive: never throws on null/blank fields.
 */
internal fun transportChipFor(
    peer: PairedPeer,
    cloudTransport: CloudTransport = CloudTransport.NONE,
): TransportChip = when {
    peer.syncAddr.isNotBlank() || peer.peerLocalIp?.isNotBlank() == true ->
        TransportChip.P2P
    cloudTransport == CloudTransport.RELAY -> TransportChip.Relay
    else -> TransportChip.Cloud
}

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
 * CopyPaste-crh3.33: regenerate the pairing QR this many seconds BEFORE the token
 * actually expires, so a slow scan never reads an already-expired code. Mirrors
 * the macOS `QR_REFRESH_MARGIN_SECS` (useQrCode.ts).
 */
internal const val QR_REFRESH_MARGIN_SECONDS = 15

/**
 * True when the QR should be pre-emptively regenerated: within the
 * [QR_REFRESH_MARGIN_SECONDS] margin of expiry. Extracted so the margin logic is
 * unit-testable without driving the Compose countdown effect.
 */
internal fun shouldRegenerateQr(remainingSeconds: Int): Boolean =
    remainingSeconds <= QR_REFRESH_MARGIN_SECONDS

/**
 * True when the PulseDot should animate: [online] && ![ reducedMotion].
 * Extracted so unit tests can verify the gate without Compose.
 */
internal fun shouldPulse(online: Boolean, reducedMotion: Boolean): Boolean =
    online && !reducedMotion

/**
 * True when the PulseDot should START a one-shot pulse (§MO-5).
 *
 * The pulse fires exactly once on the LEADING EDGE of the offline→online
 * transition. Subsequent frames where [isNowOnline] stays true (already-online
 * steady state) return false so the animation does not re-trigger.
 *
 * Gate: reduced-motion suppresses the pulse entirely per §8.
 */
internal fun shouldStartOneShotPulse(
    wasOnline: Boolean,
    isNowOnline: Boolean,
    reducedMotion: Boolean,
): Boolean = !wasOnline && isNowOnline && !reducedMotion

/**
 * CopyPaste-bdac.102: the semantic colour role for the [PulseDot] glyph (dot and ring).
 *
 * The ring must always use the SAME colour as the solid dot — encoding this as an
 * enum rather than a CompositionLocal value makes the invariant unit-testable on a
 * plain JVM without a Compose runtime.
 *
 *  ONLINE  → success green  (c.success)
 *  OFFLINE → danger red     (c.danger)
 *
 * Both the ring [Modifier.background] and the dot [Modifier.background] in
 * [PulseDot] derive their colour from [dotColor], which maps to [PulseDotColorRole]
 * via [pulseDotColorRole].
 */
internal enum class PulseDotColorRole { ONLINE, OFFLINE }

/**
 * Return the semantic colour role for the PulseDot based on [online].
 *
 * Extracted as a pure function so unit tests can confirm:
 *   - online  → ONLINE  (ring + dot use success/green)
 *   - offline → OFFLINE (ring + dot use danger/red)
 *
 * The callers ([PulseDot] ring and dot backgrounds) derive their colour from
 * [PulseDotColorRole] via the `dotColor` variable — ensuring both are identical.
 */
internal fun pulseDotColorRole(online: Boolean): PulseDotColorRole =
    if (online) PulseDotColorRole.ONLINE else PulseDotColorRole.OFFLINE

/**
 * True when the calm screen-canvas backdrop should be painted (STYLEGUIDE §6).
 * A frosted backdrop is painted only when translucency is on and the screen
 * owns its backdrop.
 */
internal fun shouldPaintCanvas(
    translucent: Boolean,
    paintCanvasBackdrop: Boolean,
): Boolean = translucent && paintCanvasBackdrop

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

/**
 * CopyPaste-pkd0: which UI rows to render for the discovered-peers (LAN) section.
 *
 * Pure tristate — the decision on what to show in the discovered-peers section is
 * expressed here without any Compose dependency so it can be unit-tested on a plain
 * JVM.  [DevicesActivity.DevicesScreen] maps this to its concrete row list.
 *
 *  HIDDEN      — P2P is disabled; the section is completely suppressed.
 *  EMPTY_STATE — P2P is enabled but no peers are visible yet; show the label +
 *                "Searching for nearby devices…" empty-state row.
 *  SHOW_PEERS  — P2P is enabled and at least one unmatched peer is present; show
 *                the label + one row per peer.
 */
internal enum class DiscoveredSectionPresence { HIDDEN, EMPTY_STATE, SHOW_PEERS }

/**
 * Decide which LAN-discovery section rows to emit.
 *
 * @param p2pEnabled     mirrors [Settings.p2pSyncEnabled]; false → HIDDEN.
 * @param discoveredCount number of discovered (unpaired) peers from [listDiscovered].
 *
 * Extracted as a pure function so the pkd0 regression (section silently dropped
 * from DevicesScreen by the redesign) has a lightweight, always-runnable guard.
 */
internal fun discoveredSectionPresence(
    p2pEnabled: Boolean,
    discoveredCount: Int,
): DiscoveredSectionPresence = when {
    !p2pEnabled -> DiscoveredSectionPresence.HIDDEN
    discoveredCount == 0 -> DiscoveredSectionPresence.EMPTY_STATE
    else -> DiscoveredSectionPresence.SHOW_PEERS
}

/** Poll cadence for the SAS pairing state machine (~500 ms). */
internal const val SAS_POLL_MS = 500L

/**
 * CopyPaste-crh3.27: UI-side watchdog for the SAS pairing dialog. If no terminal
 * state is reached within this window the dialog stops waiting and surfaces a
 * timeout instead of hanging forever. Mirrors the macOS `SAS_WATCHDOG_MS`
 * (30 s) in `SasPairingModal.tsx`.
 */
internal const val SAS_WATCHDOG_MS = 30_000L

/**
 * CopyPaste-1jms.33: format the peer's device metadata returned by the PAKE bootstrap
 * (BootstrapResult.peerModel/peerOs/peerAppVersion) into a list of label→value pairs
 * for display on the post-PAKE peer-review card.
 *
 * Rules:
 *  - Only non-null, non-blank values are included.
 *  - Order: Model → OS → Version (mirrors the macOS pairing-confirmation modal and
 *    the SAS dialog [SasPeerMetadataCard] / the post-pair success popup [PairedSuccessPopup]).
 *  - An empty list means the peer sent no metadata (pre-ABI-14 daemon or no metadata
 *    available) — callers must handle this gracefully (no crash, no empty section).
 *
 * Extracted as a pure function so unit tests can verify the include/exclude logic and
 * ordering without any Compose runtime or Android SDK.
 *
 * @param peerModel     peer's hardware model (e.g. "MacBook Air (M3)")
 * @param peerOs        peer's OS string (e.g. "macOS 15.3")
 * @param peerAppVersion peer's app version string (e.g. "0.5.3")
 * @return ordered list of (label-key, value) where label-key is a strings.xml key
 *         name (e.g. "meta_label_model") — callers resolve it via [stringResource].
 */
internal fun peerMetaReviewRows(
    peerModel: String?,
    peerOs: String?,
    peerAppVersion: String?,
): List<Pair<String, String>> = buildList {
    peerModel?.takeIf { it.isNotBlank() }?.let { add("meta_label_model" to it) }
    peerOs?.takeIf { it.isNotBlank() }?.let { add("meta_label_os" to it) }
    peerAppVersion?.takeIf { it.isNotBlank() }?.let { add("meta_label_version" to it) }
}

/**
 * Fixed bootstrap (SAS-pairing) listener port this device advertises in its mDNS
 * TXT record so peers can dial back to pair. A non-zero bport marks this device
 * SAS-pairing-capable (v2); the native discovery service binds/owns this port.
 */
// `internal` so the always-on [ClipboardService] FGS owns the discovery
// lifecycle with the SAME well-known bport (HB-2).
internal const val SAS_BPORT = 47_654

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-crh3.34: "Revoke all" parity with macOS DevicesView
// ─────────────────────────────────────────────────────────────────────────────

/**
 * CopyPaste-crh3.34: "Revoke all" button is enabled only when there is at least
 * one paired peer to revoke. Mirrors macOS `disabled={peers.length === 0}` in
 * DevicesView/index.tsx (line 172).
 *
 * Extracted as a pure function so the enabled-state gate is unit-testable on a
 * plain JVM without Compose or Android SDK.
 */
internal fun revokeAllEnabled(peerCount: Int): Boolean = peerCount > 0

/**
 * CopyPaste-crh3.34: Confirmation body text for the "Revoke all" dialog.
 * Mirrors the macOS confirmation modal body in DevicesView/index.tsx (lines 183-185):
 *   "This will immediately break trust with all paired devices."
 *   "All devices will need to re-pair before syncing can resume."
 *
 * Extracted as a pure function so the copy is unit-testable without Compose.
 */
internal fun revokeAllConfirmBody(): String =
    "This will immediately break trust with all paired devices. " +
        "All devices will need to re-pair before syncing can resume."
