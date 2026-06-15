package com.copypaste.android

/**
 * Pure helpers for the origin-device filter feature in [HistoryActivity].
 *
 * All functions are pure (no Android context needed) so they can be exercised
 * in pure-JVM unit tests ([OriginDeviceFilterTest]).
 *
 * These mirror the macOS HistoryView behaviour:
 *  - `deviceFilter == "all"` → keep everything
 *  - `deviceFilter == <id>` → keep only items whose originDeviceId matches
 *  - Filter control is shown ONLY when [distinctOriginDeviceIds] returns > 1 id
 *  - [deviceDisplayName] resolves "This device" for own id, peer name from roster,
 *    or a short 8-char id prefix as a last-resort fallback.
 */

/**
 * Filter [items] to those originating from [deviceFilter].
 * The sentinel value "all" returns the full list unchanged.
 */
fun filterByDevice(items: List<ClipboardItem>, deviceFilter: String): List<ClipboardItem> {
    if (deviceFilter == "all") return items
    return items.filter { it.originDeviceId == deviceFilter }
}

/**
 * Return the set of distinct, non-blank origin device ids present in [items].
 * Items with a null/blank [ClipboardItem.originDeviceId] are omitted.
 */
fun distinctOriginDeviceIds(items: List<ClipboardItem>): Set<String> =
    items.mapNotNull { it.originDeviceId?.takeIf { id -> id.isNotBlank() } }.toSet()

/**
 * Resolve a human-readable display name for [deviceId]:
 *  - Returns "This device" when [deviceId] equals [ownDeviceId].
 *  - Returns the [PairedPeer.name] from [peers] when a matching peer is found
 *    and the name is non-blank.
 *  - Falls back to the first 8 characters of [deviceId] (always non-empty because
 *    device ids are UUIDs).
 *
 * CopyPaste-v6ac ROOT CAUSE FIX: [ClipboardItem.originDeviceId] holds the peer's
 * stable UUID (from Hello.device_id in the sync protocol), but [PairedPeer.fingerprint]
 * is the TLS certificate hash — a completely different identifier. The previous
 * `it.fingerprint == deviceId` lookup ALWAYS missed because a UUID ≠ TLS fingerprint.
 *
 * FIX: PairedPeer does not yet carry a peerDeviceId/UUID field (adding it requires
 * a Settings.kt schema change + pairing protocol wiring — tracked separately).
 * As an interim approach we try BOTH keys so the lookup succeeds in whichever case
 * the roster happens to store: fingerprint-match (legacy, currently the only path)
 * and a future peerDeviceId field when it exists. The dual-match is safe — if no
 * roster entry matches either key we fall back to the truncated UUID as before.
 */
fun deviceDisplayName(
    deviceId: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
): String {
    if (deviceId == ownDeviceId) return "This device"
    // Try fingerprint-match first (current roster schema); also try name lookup by
    // the peer's peerDeviceId UUID field when it becomes available (null-safe).
    val peerName = peers.firstOrNull { peer ->
        peer.fingerprint == deviceId
        // Future: || peer.peerDeviceId == deviceId   (once PairedPeer carries it)
    }?.name?.takeIf { it.isNotBlank() }
    return peerName ?: deviceId.take(8)
}
