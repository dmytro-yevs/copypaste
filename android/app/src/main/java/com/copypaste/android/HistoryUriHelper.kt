package com.copypaste.android

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri

// ─────────────────────────────────────────────────────────────────────────────
// AB-12 — broad copy-back URI grant
// ─────────────────────────────────────────────────────────────────────────────

/**
 * CopyPaste-5917.73 (security hardening): Grant FLAG_GRANT_READ_URI_PERMISSION for
 * [uri] only to apps that can plausibly receive a paste, not to every installed package.
 *
 * The previous implementation called `getInstalledPackages(0)` and granted to every
 * app on the device (~100-400 packages), which is far broader than needed and
 * constitutes a URI permission leak. This version narrows to:
 *   1. Apps returned by queryIntentActivities for ACTION_PASTE (standard clipboard consumers)
 *   2. Apps returned by queryIntentActivities for ACTION_SEND with [mime] (share targets)
 *   3. A known-OEM hardlist: AOSP SystemUI, MIUI Home, Samsung clipboard — the OEMs
 *      whose clipboard hosts do NOT register intent filters but still need the URI.
 *
 * Grant failures per package are silently ignored (a package that rejects the grant
 * was never going to read the URI anyway).
 *
 * Protection is STRENGTHENED (never weakened): the set of granted packages is a strict
 * subset of what the old all-packages loop granted. Paste still works on AOSP, MIUI,
 * and Samsung via the OEM hardlist + query results.
 *
 * @param mime MIME type of the payload (e.g. "image/png", "application/octet-stream")
 *             — used to build the ACTION_SEND intent for querying share targets.
 */
internal fun grantUriToAll(ctx: Context, uri: Uri, mime: String = "*/*") {
    val pm = ctx.packageManager
    // Collect candidate packages: clipboard-paste handlers + share targets + OEM hardlist.
    val candidates = mutableSetOf<String>()

    // 1. Apps that handle ACTION_PASTE (standard clipboard overlay, e.g. AOSP SystemUI).
    try {
        val pasteIntent = Intent(Intent.ACTION_PASTE)
        @Suppress("DEPRECATION") // MATCH_ALL is API 23+; fine for our minSdk
        val pasteReceivers = pm.queryIntentActivities(pasteIntent, PackageManager.MATCH_ALL)
        for (ri in pasteReceivers) candidates.add(ri.activityInfo.packageName)
    } catch (e: Exception) {
        android.util.Log.w("HistoryActivity", "grantUriToAll: ACTION_PASTE query failed: ${e.message}")
    }

    // 2. Apps that handle ACTION_SEND for this MIME type (share-target clipboard hosts).
    try {
        val sendIntent = Intent(Intent.ACTION_SEND).setType(mime)
        @Suppress("DEPRECATION")
        val sendReceivers = pm.queryIntentActivities(sendIntent, PackageManager.MATCH_ALL)
        for (ri in sendReceivers) candidates.add(ri.activityInfo.packageName)
    } catch (e: Exception) {
        android.util.Log.w("HistoryActivity", "grantUriToAll: ACTION_SEND query failed: ${e.message}")
    }

    // 3. Known OEM clipboard hosts that don't register standard intent filters
    //    but need the URI to be readable (MIUI, Samsung, AOSP hardlist).
    candidates.addAll(
        listOf(
            "com.android.systemui",          // AOSP clipboard overlay
            "com.miui.home",                 // MIUI clipboard host (Xiaomi)
            "com.samsung.android.app.clipboard", // Samsung clipboard manager
            "com.huawei.android.launcher",   // Huawei launcher clipboard
        )
    )

    for (pkg in candidates) {
        try {
            ctx.grantUriPermission(pkg, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        } catch (_: Exception) {
            // Some packages reject the grant; harmless — they were never going to read the URI.
        }
    }
}
