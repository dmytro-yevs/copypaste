package com.copypaste.android

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Share
import com.copypaste.android.ui.theme.GlassAlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.ideTextFieldColors
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.paletteAurora
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.skinTokens
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
        // CopyPaste-92qs (MEDIUM): FLAG_SECURE. Logs can contain device IDs,
        // fingerprints, and relay URLs. Block screenshots / recents capture.
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE,
        )
        enableEdgeToEdge()
        setContent {
            CopyPasteTheme {
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

    // ── Skin / background tokens (A-C6) ────────────────────────────────────
    // Gate the aurora/canvas backdrop by tok.background so CLASSIC gets the
    // animated aurora, QUIET gets a plain solid background, and VAPOR gets a
    // tint-blob canvas. CLASSIC must remain byte-identical (always AURORA).
    val skin = LocalSkin.current
    val tok = skinTokens(skin)
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()

    // Which background variant to draw:
    //   AURORA    → full palette-aware animated aurora (auroraCanvas modifier)
    //   TINT_BLOB → static accent-tinted blob; no dedicated Modifier exists yet,
    //               so we reuse auroraCanvas as the closest available primitive.
    //               (Missing: dedicated tintBlobCanvas Modifier — noted in bd notes.)
    //   FLAT      → no canvas — plain c.bg solid fill
    val paintCanvas = translucent && tok.background != SkinBackground.FLAT

    // ── State ──────────────────────────────────────────────────────────────
    var allLines by remember { mutableStateOf<List<String>>(emptyList()) }
    var filterText by remember { mutableStateOf("") }
    var fileSizeDesc by remember { mutableStateOf("") }
    var isTruncated by remember { mutableStateOf(false) }
    var showClearDialog by remember { mutableStateOf(false) }
    var atBottom by remember { mutableStateOf(true) }

    // Filtered lines derived from allLines + filterText
    val displayLines = remember(allLines, filterText) {
        if (filterText.isBlank()) allLines
        else allLines.filter { it.contains(filterText, ignoreCase = true) }
    }

    // ── Load logs ──────────────────────────────────────────────────────────
    fun loadLogs() {
        scope.launch {
            val result = withContext(Dispatchers.IO) { readLogs(ctx) }
            allLines = result.lines
            fileSizeDesc = result.sizeDesc
            isTruncated = result.truncated
            // Auto-scroll to bottom after load
            atBottom = true
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

    val c = LocalIdeColors.current

    // ── Clear-logs confirmation dialog ──────────────────────────────────────
    if (showClearDialog) {
        GlassAlertDialog(
            onDismissRequest = { showClearDialog = false },
            title = {
                Text(
                    text = "Clear Logs",
                    style = MaterialTheme.typography.titleMedium,
                    color = c.text,
                )
            },
            text = {
                Text(
                    text = "Delete all log files (app.log, app.log.1, crash_*.txt)? " +
                        "This cannot be undone.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    showClearDialog = false
                    scope.launch {
                        withContext(Dispatchers.IO) { clearLogs(ctx) }
                        loadLogs()
                    }
                }) {
                    Text("Clear", color = c.danger)
                }
            },
            dismissButton = {
                TextButton(onClick = { showClearDialog = false }) {
                    Text("Cancel", color = c.dim)
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

    Scaffold(
        // A-C6: gate the aurora/canvas backdrop by tok.background.
        // CLASSIC (AURORA) → animated palette aurora, transparent container.
        // VAPOR  (TINT_BLOB) → auroraCanvas fallback (dedicated tintBlobCanvas missing),
        //   transparent container so the canvas shows through glass surfaces.
        // QUIET  (FLAT) → no canvas, opaque solid c.bg container.
        modifier = if (paintCanvas)
            Modifier.auroraCanvas(dark, paletteAurora(LocalPalette.current))
        else Modifier,
        containerColor = if (paintCanvas) androidx.compose.ui.graphics.Color.Transparent else c.bg,
        topBar = {
            CopyPasteTopBar(
                title = "Logs",
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
                        Icon(
                            imageVector = if (atBottom) Icons.Default.KeyboardArrowUp
                                          else Icons.Default.KeyboardArrowDown,
                            contentDescription = if (atBottom) "Scroll to top" else "Scroll to bottom",
                            tint = c.dim,
                        )
                    }
                    // Refresh
                    IconButton(onClick = { loadLogs() }) {
                        Icon(
                            imageVector = Icons.Default.Refresh,
                            contentDescription = "Refresh logs",
                            tint = c.dim,
                        )
                    }
                    // Share / Export
                    IconButton(onClick = { LogExportHelper.shareLogsZip(ctx) }) {
                        Icon(
                            imageVector = Icons.Default.Share,
                            contentDescription = "Export logs",
                            tint = c.dim,
                        )
                    }
                    // Clear logs
                    IconButton(onClick = { showClearDialog = true }) {
                        Icon(
                            imageVector = Icons.Default.Delete,
                            contentDescription = "Clear logs",
                            tint = c.danger,
                        )
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
                    color = c.faint,
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(c.panel)
                        .padding(horizontal = 12.dp, vertical = 4.dp),
                )
            }

            // ── Truncation notice ─────────────────────────────────────────
            if (isTruncated) {
                Text(
                    text = "Showing last ${TAIL_BYTES / 1024} KB — older lines omitted to avoid OOM.",
                    style = MaterialTheme.typography.labelSmall,
                    color = c.warning,
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(c.panel)
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
                        color = c.faint,
                    )
                },
                singleLine = true,
                colors = ideTextFieldColors(),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
            )

            // ── Log lines ─────────────────────────────────────────────────
            if (displayLines.isEmpty()) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = if (allLines.isEmpty()) "No log entries yet." else "No lines match filter.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.dim,
                    )
                }
            } else {
                SelectionContainer {
                    LazyColumn(
                        state = listState,
                        modifier = Modifier
                            .fillMaxSize()
                            .background(c.bg),
                    ) {
                        itemsIndexed(displayLines) { _, line ->
                            LogLine(line)
                        }
                    }
                }
            }
        }
    }
}

/**
 * Renders a single log line with level-based colour coding.
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
    val c = LocalIdeColors.current
    val color = when {
        // Level codes come after the timestamp: "... E/Tag:" or "... W/Tag:"
        // Uses file-level precompiled regexes to avoid allocation on every recomposition.
        RE_LEVEL_ANY.containsMatchIn(line) -> {
            when {
                RE_LEVEL_E.containsMatchIn(line) -> c.danger
                RE_LEVEL_W.containsMatchIn(line) -> c.warning
                RE_LEVEL_I.containsMatchIn(line) -> c.text
                RE_LEVEL_D.containsMatchIn(line) -> c.dim
                else -> c.dim
            }
        }
        // Stack trace lines
        line.trimStart().startsWith("at ") -> c.faint
        // Crash report header lines (=== ... ===)
        line.startsWith("=") -> c.accent
        else -> c.dim
    }

    // A6: soft-wrap long lines so everything is visible without horizontal scrolling
    Text(
        text = line,
        style = MaterialTheme.typography.bodySmall.copy(
            fontFamily = MonoFontFamily,
            fontSize = 11.sp,
            lineHeight = 16.sp,
        ),
        color = color,
        softWrap = true,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp),
    )
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

        allLines.add("── ${file.name} (${formatSize(fileSize)}) ──")

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
            allLines.add("  [… ${formatSize(skip)} omitted — showing last ${formatSize(toRead)} …]")
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

    val sizeDesc = "${formatSize(totalBytes)} across ${files.size} file${if (files.size == 1) "" else "s"}"
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

private fun formatSize(bytes: Long): String {
    return when {
        bytes >= 1024 * 1024 -> "%.1f MB".format(bytes.toDouble() / (1024 * 1024))
        bytes >= 1024 -> "${bytes / 1024} KB"
        else -> "$bytes B"
    }
}
