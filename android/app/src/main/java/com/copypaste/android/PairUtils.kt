package com.copypaste.android

/**
 * Build the human-readable label shown after a successful scan, e.g.
 * `"Pixel 8 (a1b2c3…)"`. Pure (no Android/FFI deps) so it is unit-testable on
 * the JVM. A blank device name falls back to the literal "device".
 */
internal fun formatScannedInfo(deviceName: String, fingerprint: String): String =
    "${deviceName.ifBlank { "device" }} ($fingerprint)"

/**
 * Best-effort lookup of this device's site-local IPv4 address (e.g. 192.168.x.x,
 * 10.x.x.x), used to build the inbound listener's advertised `sync_addr` at pair
 * time so the macOS peer can dial back over the LAN.
 *
 * Enumerates active, non-loopback interfaces and returns the first site-local
 * IPv4 (skipping link-local 169.254.x.x and IPv6). Returns null when no such
 * address exists (no Wi-Fi / cellular-only), in which case the caller falls back
 * to advertising no address (Android→macOS dial only).
 *
 * No WifiManager dependency: NetworkInterface enumeration works for both Wi-Fi
 * and other LAN interfaces without the ACCESS_WIFI_STATE permission.
 *
 * `internal` so the discovery pairing path ([DevicesActivity]) reuses the SAME
 * helper for HB-1a `local_ip` instead of duplicating the enumeration.
 */
internal fun lanIpv4Address(): String? {
    return try {
        java.net.NetworkInterface.getNetworkInterfaces()?.toList()
            ?.asSequence()
            ?.filter { runCatching { it.isUp && !it.isLoopback }.getOrDefault(false) }
            ?.flatMap { it.inetAddresses.toList().asSequence() }
            ?.filterIsInstance<java.net.Inet4Address>()
            ?.firstOrNull { it.isSiteLocalAddress }
            ?.hostAddress
    } catch (e: Exception) {
        android.util.Log.w("PairActivity", "lanIpv4Address lookup failed: ${e.message}")
        null
    }
}
