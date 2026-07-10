package com.copypaste.android

import androidx.compose.runtime.Composable
import com.copypaste.android.ui.theme.relativeTimeAgoLabel
import java.text.DateFormat
import java.util.Date

// ─────────────────────────────────────────────────────────────────────────────
// Relative time helper — §5 tabular-nums timestamps
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun relativeTime(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    if (diff >= 7 * 86_400_000L) return DateFormat.getDateInstance(DateFormat.SHORT).format(Date(ms))
    return relativeTimeAgoLabel(diff)
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

/**
 * CopyPaste-myh8.5 (S5 5.4, P0-7 partial-span masking) — the URL host/path
 * split HistoryRow feeds into its bold-host/dim-path `AnnotatedString`, but
 * ALWAYS sourced from [spanMaskedDisplay] when it is non-null (i.e. the item
 * has partial sensitive spans and is not fully sensitive — see
 * `HistoryRowModel.resolveSpanMaskedDisplay`).
 *
 * SECURITY: the previous inline call site built the annotated string straight
 * from the raw (unmasked) `display` string whenever [chipLabel] was "URL" —
 * bypassing span masking entirely, so a sensitive sub-string embedded in a URL
 * (e.g. a token in the query string) rendered in plaintext inside the bold
 * host / dim path spans even though the row was not "fully" sensitive. Routing
 * through [spanMaskedDisplay] first closes that leak: spans are already
 * bullet-replaced (see `applySpanMasking`) BEFORE this function ever splits
 * the string into host/path, so no plaintext sensitive span reaches the
 * `AnnotatedString` this feeds.
 */
internal fun urlPartsForRow(
    chipLabel: String,
    spanMaskedDisplay: String?,
    display: String,
): Pair<String, String>? = if (chipLabel == "URL") splitUrl(spanMaskedDisplay ?: display) else null
