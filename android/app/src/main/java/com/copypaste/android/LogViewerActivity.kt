package com.copypaste.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.selection.SelectionContainer
import com.copypaste.android.ui.theme.GlassAlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
// TextButton removed — replaced by CopyPasteButton (CopyPaste-bdac.8)
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.SecureWindowChrome
import com.copypaste.android.ui.theme.EmptyStateCard
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.icons.LucideIcons
import com.copypaste.android.ui.theme.ideTextFieldColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/**
 * In-app log viewer for CopyPaste diagnostic logs.
 *
 * Reads the persistent log files written by [AppLogger] (app.log, app.log.1, crash_*.txt).
 * To avoid OOM on large files, only the last [TAIL_BYTES] of each file are read.
 * A header note is shown when the log is truncated.
 *
 * Features:
 *   - Scrollable monospace view of log lines (newest-at-bottom, auto-scroll on open)
 *   - Toggle to jump to top / bottom
 *   - Refresh action to re-read files
 *   - Share/Export action (delegates to [LogExportHelper])
 *   - Clear-logs action with confirmation dialog
 *   - Substring filter field to filter visible lines
 *   - File size + line count shown in top bar subtitle
 *   - Log-level colour coding: E=red, W=amber, D=dim, I=text
 */
class LogViewerActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // CopyPaste-1g00: screenshot protection is now pref-driven (Settings.allowScreenshots).
        // SecureWindowChrome applies FLAG_SECURE centrally when allowScreenshots=false (the default).
        applyScreenshotPolicy(Settings(this))
        enableEdgeToEdge()
        setContent {
            SecureWindowChrome {
                LogViewerScreen(onBack = { finish() })
            }
        }
    }
}

// Maximum bytes to read from all log files combined (to avoid OOM).
private const val TAIL_BYTES = 256 * 1024L // 256 KB

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LogViewerScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val listState: LazyListState = rememberLazyListState()

    // ── State ──────────────────────────────────────────────────────────────
    val toastState = remember { GlassToastState() }
    val exportedMsg = stringResource(R.string.log_export_success)
    var allLines by remember { mutableStateOf<List<String>>(emptyList()) }
    var filterText by remember { mutableStateOf("") }
    var fileSizeDesc by remember { mutableStateOf("") }
    var isTruncated by remember { mutableStateOf(false) }
    var showClearDialog by remember { mutableStateOf(false) }
    var atBottom by remember { mutableStateOf(true) }
    // CopyPaste-bdac.4: error state for IO failures in readLogs().
    var readError by remember { mutableStateOf<String?>(null) }
    // S11 W3: distinguishes "still reading files" from "read, found nothing" so the
    // empty-state card doesn't flash before the first successful/failed load resolves.
    var isLoading by remember { mutableStateOf(false) }

    // Filtered lines derived from allLines + filterText
    val displayLines = remember(allLines, filterText) {
        if (filterText.isBlank()) allLines
        else allLines.filter { it.contains(filterText, ignoreCase = true) }
    }

    // ── Load logs ──────────────────────────────────────────────────────────
    fun loadLogs() {
        scope.launch {
            isLoading = true
            // CopyPaste-bdac.4: wrap IO in try/catch so a missing or unreadable log
            // file shows a friendly error instead of silently staying empty.
            try {
                val result = withContext(Dispatchers.IO) { readLogs(ctx) }
                readError = null
                allLines = result.lines
                fileSizeDesc = result.sizeDesc
                isTruncated = result.truncated
                // Auto-scroll to bottom after load
                atBottom = true
            } catch (e: Exception) {
                readError = "Could not read log file — try restarting the app. (${e.message})"
            } finally {
                isLoading = false
            }
        }
    }

    // Initial load + auto-scroll to end
    LaunchedEffect(Unit) { loadLogs() }

    // Scroll to bottom when atBottom is requested
    LaunchedEffect(atBottom, displayLines.size) {
        if (displayLines.isNotEmpty()) {
            // The toggle flips atBottom: true → newest (bottom), false → oldest
            // (top). Both directions must scroll, or the "scroll to top" button
            // does nothing.
            listState.scrollToItem(if (atBottom) displayLines.size - 1 else 0)
        }
    }

    // ── Clear-logs confirmation dialog ──────────────────────────────────────
    if (showClearDialog) {
        GlassAlertDialog(
            onDismissRequest = { showClearDialog = false },
            title = {
                Text(
                    text = "Clear Logs",
                    color = MaterialTheme.colorScheme.onSurface,
                )
            },
            text = {
                Text(
                    text = "Delete all log files (app.log, app.log.1, crash_*.txt)? " +
                        "This cannot be undone.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            confirmButton = {
                CopyPasteButton(onClick = {
                    showClearDialog = false
                    scope.launch {
                        withContext(Dispatchers.IO) { clearLogs(ctx) }
                        loadLogs()
                    }
                }, variant = ButtonVariant.DANGER) {
                    Text(stringResource(R.string.logs_action_clear))
                }
            },
            dismissButton = {
                CopyPasteButton(onClick = { showClearDialog = false }, variant = ButtonVariant.GHOST) {
                    Text(stringResource(R.string.dialog_cancel))
                }
            },
        )
    }

    // ── Top bar subtitle: file size + line count ───────────────────────────
    val subtitle = buildString {
        append(fileSizeDesc)
        if (allLines.isNotEmpty()) {
            append("  •  ${allLines.size} lines")
        }
        if (filterText.isNotBlank() && displayLines.size != allLines.size) {
            append("  •  ${displayLines.size} shown")
        }
    }

    Box(Modifier.fillMaxSize()) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.logs_title),
                showBackButton = true,
                onBack = onBack,
                backContentDescription = "Back",
                actions = {
                    // Scroll to top / bottom toggle.
                    // Only mutate atBottom here; the LaunchedEffect(atBottom, displayLines.size)
                    // below is the single scroll driver — two concurrent scroll paths would race.
                    IconButton(onClick = {
                        atBottom = !atBottom
                    }) {
                        Text(if (atBottom) "Top" else "Bottom")
                    }
                    // Refresh
                    IconButton(onClick = { loadLogs() }) {
                        Text(stringResource(R.string.logs_action_refresh))
                    }
                    // Share / Export
                    IconButton(onClick = {
                        LogExportHelper.shareLogsZip(
                            ctx,
                            onError = { msg ->
                                scope.launch { toastState.show(msg, GlassToastKind.DANGER) }
                            },
                            onSuccess = {
                                scope.launch {
                                    toastState.show(exportedMsg, GlassToastKind.SUCCESS)
                                }
                            },
                        )
                    }) {
                        Text(stringResource(R.string.logs_action_export))
                    }
                    // Clear logs
                    IconButton(onClick = { showClearDialog = true }) {
                        Text(stringResource(R.string.logs_action_clear))
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            // ── Subtitle row (size / line count) ──────────────────────────
            if (subtitle.isNotBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 4.dp),
                )
            }

            // ── Truncation notice ─────────────────────────────────────────
            if (isTruncated) {
                Text(
                    text = "Showing last ${TAIL_BYTES / 1024} KB — older lines omitted to avoid OOM.",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.tertiary,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 4.dp),
                )
            }

            // ── Filter field ──────────────────────────────────────────────
            OutlinedTextField(
                value = filterText,
                onValueChange = { filterText = it },
                placeholder = {
                    Text(
                        "Filter lines…",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
                singleLine = true,
                colors = ideTextFieldColors(),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
            )

            // ── Log lines ─────────────────────────────────────────────────
            if (isLoading && allLines.isEmpty()) {
                // Distinct from the empty-state card below — matches StorageTab's
                // in-flight indicator (16dp/2dp) so "reading" doesn't look like "empty".
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp),
                        strokeWidth = 2.dp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else if (displayLines.isEmpty()) {
                // CopyPaste-bdac.15: use shared EmptyStateCard (icon+card) so the empty state
                // matches HistoryActivity's pattern (was a bare centered Text composable).
                // Error state (bdac.4) still uses accent2 icon but with danger text below.
                val errorMsg = readError
                val isFilter = allLines.isNotEmpty() && errorMsg == null
                EmptyStateCard(
                    icon = {},
                    title = when {
                        errorMsg != null -> "Could not read logs"
                        isFilter -> "No lines match filter"
                        else -> "No log entries yet"
                    },
                    subtitle = when {
                        errorMsg != null -> errorMsg
                        isFilter -> "Clear the filter above to see all log lines"
                        else -> "Diagnostic logs appear here after app activity"
                    },
                    padding = PaddingValues(0.dp),
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                SelectionContainer {
                    LazyColumn(
                        state = listState,
                        modifier = Modifier
                            .fillMaxSize(),
                    ) {
                        itemsIndexed(displayLines) { _, line ->
                            LogLine(line)
                        }
                    }
                }
            }
        }
    }
    GlassToastHost(state = toastState)
    } // end Box
}

/** Log severity extracted from a log line by [logLevelMarker]. Null = no level code present. */
internal enum class LogLevel { E, W, I, D }

/**
 * Detects the severity level of a raw log line, reusing the precompiled
 * `RE_LEVEL_*` regexes so recomposition and tests share one source of truth.
 * Pure function — no Compose/Android dependency — so it is JVM-unit-testable
 * without the merged-resources/native-.so constraints that block Compose tests.
 */
internal fun logLevelMarker(line: String): LogLevel? {
    if (!RE_LEVEL_ANY.containsMatchIn(line)) return null
    return when {
        RE_LEVEL_E.containsMatchIn(line) -> LogLevel.E
        RE_LEVEL_W.containsMatchIn(line) -> LogLevel.W
        RE_LEVEL_I.containsMatchIn(line) -> LogLevel.I
        RE_LEVEL_D.containsMatchIn(line) -> LogLevel.D
        else -> null
    }
}

/**
 * Renders a single log line with level-based colour coding AND a leading
 * marker (icon or compact text badge) — colour alone is not an accessible
 * level signal (CopyPaste-myh8.11 S11 W3).
 *
 * Log format written by AppLogger:
 *   `2026-01-15 12:34:56.789 E/MyTag: message`
 * Crash files have a plain-text header followed by a stack trace.
 *
 * Colour mapping:
 *   E/ → IdeDanger (red)
 *   W/ → IdeWarning (amber)
 *   I/ → IdeText (normal)
 *   D/ → IdeDim (subdued)
 *   everything else (crash headers, traces) → IdeDim
 */
@Composable
private fun LogLine(line: String) {
    val colorScheme = MaterialTheme.colorScheme
    val level = logLevelMarker(line)
    val color = when (level) {
        LogLevel.E -> colorScheme.error
        LogLevel.W -> colorScheme.tertiary
        LogLevel.I -> colorScheme.onSurface
        LogLevel.D -> colorScheme.onSurfaceVariant
        null -> when {
            // Stack trace lines
            line.trimStart().startsWith("at ") -> colorScheme.onSurfaceVariant
            // Crash report header lines (=== ... ===)
            line.startsWith("=") -> colorScheme.primary
            else -> colorScheme.onSurfaceVariant
        }
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp),
        verticalAlignment = Alignment.Top,
    ) {
        // Leading marker: E/W/I get a StatusXxx icon (LucideIcons role reuse); D has
        // no clean status-icon role, so it falls back to a compact text badge.
        when (level) {
            LogLevel.E -> Icon(
                imageVector = LucideIcons.StatusErr,
                contentDescription = stringResource(R.string.log_level_error),
                tint = color,
                modifier = Modifier.size(11.dp),
            )
            LogLevel.W -> Icon(
                imageVector = LucideIcons.StatusWarn,
                contentDescription = stringResource(R.string.log_level_warning),
                tint = color,
                modifier = Modifier.size(11.dp),
            )
            LogLevel.I -> Icon(
                imageVector = LucideIcons.StatusInfo,
                contentDescription = stringResource(R.string.log_level_info),
                tint = color,
                modifier = Modifier.size(11.dp),
            )
            LogLevel.D -> Text(
                text = "D",
                style = MaterialTheme.typography.bodySmall.copy(
                    fontFamily = FontFamily.Monospace,
                    fontSize = CpTypography.micro.fontSize,
                ),
                color = color,
            )
            null -> {}
        }
        // A6: soft-wrap long lines so everything is visible without horizontal scrolling
        Text(
            text = line,
            style = MaterialTheme.typography.bodySmall.copy(
                fontFamily = FontFamily.Monospace,
                fontSize = 11.sp,
                lineHeight = 16.sp,
            ),
            color = color,
            softWrap = true,
            modifier = Modifier.padding(start = if (level != null) 4.dp else 0.dp),
        )
    }
}

// ── Pre-compiled log-level regexes (hoisted to avoid per-line-per-recomposition allocation) ──
private val RE_LEVEL_ANY = Regex("""\d \w/""")
private val RE_LEVEL_E   = Regex("""\d E/""")
private val RE_LEVEL_W   = Regex("""\d W/""")
private val RE_LEVEL_I   = Regex("""\d I/""")
private val RE_LEVEL_D   = Regex("""\d D/""")

// ── I/O helpers (run on Dispatchers.IO) ──────────────────────────────────────

private data class LogReadResult(
    val lines: List<String>,
    val sizeDesc: String,
    val truncated: Boolean,
)

/**
 * Reads all log files from [AppLogger.logDir], concatenating their content,
 * but caps the total byte read to [TAIL_BYTES] to prevent OOM on large files.
 *
 * Files are sorted newest-modified-first so the most recent log (app.log) comes
 * last (displayed at the bottom of the viewer after we reverse the combined list).
 *
 * Within each file we take the tail — if the file is larger than our budget, we
 * seek to (fileSize - budget) and read from there, adding a truncation marker.
 */
private fun readLogs(context: android.content.Context): LogReadResult {
    val dir = AppLogger.logDir(context)
    val files = dir.listFiles()
        ?.filter { it.isFile && it.length() > 0 }
        ?.sortedBy { it.lastModified() } // oldest first → newest last (bottom)
        ?: emptyList()

    if (files.isEmpty()) {
        return LogReadResult(emptyList(), "No log files", false)
    }

    val totalBytes = files.sumOf { it.length() }
    var byteBudget = TAIL_BYTES
    val allLines = mutableListOf<String>()
    var truncated = false

    // Process files oldest-first; each file gets a proportional share of the
    // budget based on its size relative to the total.  When total <= TAIL_BYTES
    // every file is read completely.
    for (file in files) {
        val fileSize = file.length()
        // Proportional share: this file's fraction of remaining total size
        val share = if (totalBytes <= TAIL_BYTES) fileSize else {
            (fileSize.toDouble() / totalBytes * TAIL_BYTES).toLong().coerceAtLeast(0L)
        }
        val toRead = share.coerceAtMost(byteBudget)
        if (toRead <= 0L) continue

        allLines.add("── ${file.name} (${formatSize(context, fileSize)}) ──")

        val content: String
        if (fileSize <= toRead) {
            content = file.readText(Charsets.UTF_8)
        } else {
            // Tail: skip (fileSize - toRead) bytes, read the rest
            val skip = fileSize - toRead
            content = file.inputStream().use { stream ->
                stream.skip(skip)
                stream.readBytes().toString(Charsets.UTF_8)
            }
            // Find first newline so we don't start mid-line
            val firstNl = content.indexOf('\n')
            val trimmed = if (firstNl >= 0) content.substring(firstNl + 1) else content
            allLines.add("  [… ${formatSize(context, skip)} omitted — showing last ${formatSize(context, toRead)} …]")
            allLines.addAll(trimmed.lines())
            truncated = true
            byteBudget -= toRead
            continue
        }
        allLines.addAll(content.lines())
        byteBudget -= toRead
    }

    // Remove trailing blank lines introduced by String.lines().
    // Use removeAt(size-1) instead of removeLast() — removeLast() is API 35+ on java.util.List.
    while (allLines.isNotEmpty() && allLines.last().isBlank()) allLines.removeAt(allLines.size - 1)

    val sizeDesc = "${formatSize(context, totalBytes)} across ${files.size} file${if (files.size == 1) "" else "s"}"
    return LogReadResult(allLines, sizeDesc, truncated)
}

/**
 * Deletes all files in the AppLogger log directory.
 * Crash files and rotated log files are all removed.
 */
private fun clearLogs(context: android.content.Context) {
    val dir = AppLogger.logDir(context)
    dir.listFiles()?.forEach { file ->
        if (file.isFile) file.delete()
    }
}

// Locale-aware short size format (e.g. "1.2 MB") via the platform formatter
// instead of a hand-rolled, English-only unit string.
private fun formatSize(context: android.content.Context, bytes: Long): String {
    return android.text.format.Formatter.formatShortFileSize(context, bytes)
}
