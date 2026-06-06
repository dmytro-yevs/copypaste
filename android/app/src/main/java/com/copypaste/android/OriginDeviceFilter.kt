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
 *  - Returns the [PairedPeer.name] from [peers] when a matching fingerprint exists
 *    and the name is non-blank.
 *  - Falls back to the first 8 characters of [deviceId] (always non-empty because
 *    device ids are UUIDs).
 */
fun deviceDisplayName(
    deviceId: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
): String {
    if (deviceId == ownDeviceId) return "This device"
    val peerName = peers.firstOrNull { it.fingerprint == deviceId }?.name?.takeIf { it.isNotBlank() }
    return peerName ?: deviceId.take(8)
}
