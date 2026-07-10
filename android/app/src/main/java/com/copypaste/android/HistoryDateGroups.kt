package com.copypaste.android

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import java.util.Calendar
import java.util.TimeZone

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-myh8.5 (S5 5.2) — date-group headers, `specs/android-history/spec.md`
// "Date Group Headers": PINNED / TODAY / YESTERDAY / EARLIER, sticky per group.
// Pure grouping logic lives here (unit-tested, no Compose dependency for the
// grouping itself) so [HistoryList] only has to fold the already-sorted item
// list into headers + rows.
// ─────────────────────────────────────────────────────────────────────────────

/** One of the four date groups a row can fall into (spec.md §"Date Group Headers"). */
internal enum class HistoryDateGroup(@StringRes val labelRes: Int) {
    PINNED(R.string.date_header_pinned),
    TODAY(R.string.date_header_today),
    YESTERDAY(R.string.date_header_yesterday),
    EARLIER(R.string.date_header_earlier),
}

/**
 * Resolves the date group for one row. Pinned rows always group under
 * [HistoryDateGroup.PINNED] regardless of their timestamp (spec.md "pinned rows
 * are grouped above the Today date group"). [nowMs]/[zone] are injected so this
 * stays a pure, deterministic, unit-testable function — no `System`/`Calendar`
 * default-timezone coupling inside a test run.
 */
internal fun dateGroupFor(
    pinned: Boolean,
    wallTimeMs: Long,
    nowMs: Long,
    zone: TimeZone = TimeZone.getDefault(),
): HistoryDateGroup {
    if (pinned) return HistoryDateGroup.PINNED
    val cal = Calendar.getInstance(zone)
    cal.timeInMillis = nowMs
    cal.set(Calendar.HOUR_OF_DAY, 0)
    cal.set(Calendar.MINUTE, 0)
    cal.set(Calendar.SECOND, 0)
    cal.set(Calendar.MILLISECOND, 0)
    val todayStart = cal.timeInMillis
    val yesterdayStart = todayStart - 86_400_000L
    return when {
        wallTimeMs >= todayStart -> HistoryDateGroup.TODAY
        wallTimeMs >= yesterdayStart -> HistoryDateGroup.YESTERDAY
        else -> HistoryDateGroup.EARLIER
    }
}

/** One entry in the flattened (header | row) list a `LazyColumn` renders. */
internal sealed class HistoryListEntry {
    data class Header(val group: HistoryDateGroup) : HistoryListEntry()
    data class Row(val item: ClipboardItem) : HistoryListEntry()
}

/**
 * Folds an ALREADY-SORTED (pinned-first, then recency-descending — see
 * `sortHistoryItems`) item list into a flat header/row sequence: a new
 * [HistoryListEntry.Header] is emitted every time the resolved
 * [HistoryDateGroup] changes from the previous row (a single boundary scan —
 * correct because the upstream sort keeps pinned items contiguous and unpinned
 * items in recency order, so TODAY/YESTERDAY/EARLIER runs are each contiguous
 * too). NOTE: when the list is additionally grouped by device
 * (`sortByDevice=true`), date groups may legitimately repeat once per device
 * section — a documented, non-blocking limitation (CopyPaste-myh8.5 bd notes),
 * not a bug in this fold.
 */
internal fun buildHistoryListEntries(items: List<ClipboardItem>, nowMs: Long): List<HistoryListEntry> {
    val result = ArrayList<HistoryListEntry>(items.size + 4)
    var lastGroup: HistoryDateGroup? = null
    for (item in items) {
        val group = dateGroupFor(item.pinned, item.wallTimeMs, nowMs)
        if (group != lastGroup) {
            result += HistoryListEntry.Header(group)
            lastGroup = group
        }
        result += HistoryListEntry.Row(item)
    }
    return result
}

/**
 * Sticky date-group header — spec.md "Header rendering": uppercase mono, 10px,
 * `--faint`, `letter-spacing .1em`, no background slab (the solid `cp.bg` fill
 * here is the list's OWN background color, not a distinct tinted slab — it
 * exists only so the sticky header stays legible over the rows scrolling
 * beneath it, matching every other sticky-header treatment in Android/M3).
 */
@Composable
internal fun HistoryDateHeaderRow(group: HistoryDateGroup, modifier: Modifier = Modifier) {
    val cp = LocalCpColors.current
    Text(
        text = stringResource(group.labelRes),
        style = CpTypography.micro.copy(color = cp.faint, letterSpacing = 0.1f.em),
        modifier = modifier
            .fillMaxWidth()
            .background(cp.bg)
            .padding(horizontal = 12.dp, vertical = 6.dp),
    )
}
