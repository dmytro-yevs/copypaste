package com.copypaste.android

import java.text.DateFormat
import java.util.Date

// ─────────────────────────────────────────────────────────────────────────────
// Relative time helper — §5 tabular-nums timestamps
// ─────────────────────────────────────────────────────────────────────────────

internal fun relativeTime(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    return when {
        diff < 60_000L      -> "just now"
        diff < 3_600_000L   -> "${diff / 60_000}m ago"
        diff < 86_400_000L  -> "${diff / 3_600_000}h ago"
        diff < 7 * 86_400_000L -> "${diff / 86_400_000}d ago"
        else -> DateFormat.getDateInstance(DateFormat.SHORT).format(Date(ms))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// URL host/path split — audit #13 (bold host + dim path, web parity)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Split a URL [raw] into (host, path) for the §-13 bold-host / dim-path render.
 *
 * PG-52: aligns with macOS parseUrl which uses `new URL(raw).hostname` for the
 * bold part and `pathname + search + hash` for the dim suffix. The scheme is not
 * shown (matches macOS — the hostname alone is the visual anchor).
 * Returns (raw, "") when the URL cannot be parsed so the caller shows the full text.
 */
internal fun splitUrl(raw: String): Pair<String, String> {
    val schemeSep = raw.indexOf("://")
    if (schemeSep < 0) return raw to ""
    val afterScheme = schemeSep + 3
    // First '/' (or '?' / '#') after the authority marks the start of the path.
    val pathStart = raw
        .drop(afterScheme)
        .indexOfFirst { it == '/' || it == '?' || it == '#' }
    val hostEnd = if (pathStart < 0) raw.length else afterScheme + pathStart
    val host = raw.substring(afterScheme, hostEnd)
    if (host.isEmpty()) return raw to ""
    // path = everything after the host (path + query + fragment), empty when host-only.
    val path = if (pathStart < 0) "" else raw.substring(hostEnd)
    return host to path
}
