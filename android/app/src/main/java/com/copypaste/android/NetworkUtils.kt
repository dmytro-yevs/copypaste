package com.copypaste.android

/**
 * CopyPaste-agde: returns true when the device has an active Wi-Fi (or Ethernet)
 * network connection, false when on cellular or when connectivity is unavailable.
 *
 * Used by [FgsSyncLoop.start] to enforce the [Settings.syncOnWifiOnly] preference:
 * Supabase poll and P2P dials are skipped while on a metered (cellular) connection.
 *
 * Null [context] → returns true (no skip) so unit tests and stub mode are unaffected.
 *
 * Implementation uses [android.net.ConnectivityManager.getNetworkCapabilities] on
 * API 23+ (required by our minSdk), which is the only reliable way to query transport
 * type. The legacy [android.net.ConnectivityManager.activeNetworkInfo] path is
 * deprecated since API 29 and omitted.
 *
 * Extracted verbatim from `FgsSyncLoop.kt` (CopyPaste-vp63.35) so it can be reused
 * by other collaborators without depending on the sync-loop class itself.
 */
internal fun isOnWifi(context: android.content.Context?): Boolean {
    context ?: return true // no context → don't block (unit-test safe)
    val cm = context.getSystemService(android.content.Context.CONNECTIVITY_SERVICE)
        as? android.net.ConnectivityManager ?: return false
    val network = cm.activeNetwork ?: return false
    val caps = cm.getNetworkCapabilities(network) ?: return false
    // TRANSPORT_WIFI covers both station and tethered Wi-Fi.
    // TRANSPORT_ETHERNET covers wired Ethernet (also unmetered — honour it).
    return caps.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI) ||
        caps.hasTransport(android.net.NetworkCapabilities.TRANSPORT_ETHERNET)
}
