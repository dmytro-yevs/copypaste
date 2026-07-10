package com.copypaste.android.ui.theme

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import com.copypaste.android.R

// CopyPaste-myh8.13 S13 Wave a: consolidates the "Xs/m/h/d ago" formatting that was
// duplicated (with slightly different literal strings) across PeerRow.kt,
// HistoryUrlUtils.kt, PreviewActionRow.kt, ui/SyncStatusBadge.kt and SyncTab.kt.
// Callers keep OWNERSHIP of the epoch-diff computation (some use the system clock,
// some an injected `nowMs` for deterministic paparazzi/unit tests) and of the
// "older than N" cutoff/absolute-date fallback, which differs per call site — this
// only formats an already-computed elapsed duration.

/**
 * Formats [elapsedMs] (must be >= 0) as a short relative-time label: "just now",
 * "42s ago", "3m ago", "5h ago", "2d ago". Callers are responsible for falling back
 * to an absolute date/time string once [elapsedMs] exceeds their own cutoff — this
 * function has no upper bound and will happily return "40d ago".
 */
@Composable
internal fun relativeTimeAgoLabel(elapsedMs: Long): String {
    val elapsedSeconds = (elapsedMs / 1000L).coerceAtLeast(0L)
    return when {
        elapsedSeconds < 5L -> stringResource(R.string.relative_time_just_now)
        elapsedSeconds < 60L -> {
            val s = elapsedSeconds.toInt()
            pluralStringResource(R.plurals.relative_time_seconds, s, s)
        }
        elapsedSeconds < 3_600L -> {
            val m = (elapsedSeconds / 60L).toInt()
            pluralStringResource(R.plurals.relative_time_minutes, m, m)
        }
        elapsedSeconds < 86_400L -> {
            val h = (elapsedSeconds / 3_600L).toInt()
            pluralStringResource(R.plurals.relative_time_hours, h, h)
        }
        else -> {
            val d = (elapsedSeconds / 86_400L).toInt()
            pluralStringResource(R.plurals.relative_time_days, d, d)
        }
    }
}

/**
 * "Never" / relative-time label for a last-activity timestamp, matching the
 * SyncTab / SyncStatusBadge diagnostics sheet format exactly (falls back to a
 * locale short date+time once [lastMs] is more than a day old).
 */
@Composable
internal fun relativeSyncLabel(nowMs: Long, lastMs: Long): String {
    if (lastMs <= 0L) return stringResource(R.string.relative_time_never)
    val elapsedMs = nowMs - lastMs
    if (elapsedMs >= 86_400_000L) {
        return java.text.DateFormat
            .getDateTimeInstance(java.text.DateFormat.SHORT, java.text.DateFormat.SHORT)
            .format(java.util.Date(lastMs))
    }
    return relativeTimeAgoLabel(elapsedMs)
}
