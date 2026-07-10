package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.InputChip
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.IdeSwitch
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_LABELS
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_VALUES
import com.copypaste.android.ui.theme.QUOTA_STEP_LABELS
import com.copypaste.android.ui.theme.QUOTA_STEP_VALUES
import com.copypaste.android.ui.theme.SectionLabel
import com.copypaste.android.ui.theme.SteppedSliderRow
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_VALUES

// ─────────────────────────────────────────────────────────────────────────────
// C-P1-1 step arrays — BINARY MiB units (* 1024 * 1024) to match the Rust core
// (crates/copypaste-core/src/config/defaults.rs) and the macOS SettingsView, and
// to fix the decimal-vs-binary drift for these new size fields.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Sensitive auto-clear TTL steps (seconds). `0` is the "disabled" sentinel
 * (never auto-wipe) and is intentionally the first step. Mirrors the macOS
 * SENSITIVE_TTL_STEPS, with 0 added for the disabled case.
 */
internal val SENSITIVE_TTL_STEP_VALUES: LongArray = longArrayOf(
    0L, 10L, 30L, 60L, 5L * 60, 15L * 60, 60L * 60,
)
internal val SENSITIVE_TTL_STEP_LABELS: Array<String> = arrayOf(
    "Off", "10 s", "30 s", "1 min", "5 min", "15 min", "1 hour",
)

/**
 * C-P1-1: editable "Excluded apps" list — a text input + Add button and a set of
 * removable chips, mirroring the macOS SettingsView excluded-apps control. Edits
 * are buffered in the parent's Compose state and persisted on Save (clamped via
 * the native clampConfig in [Settings.excludedAppBundleIds]).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ExcludedAppsRow(
    excludedApps: List<String>,
    onExcludedAppsChange: (List<String>) -> Unit,
) {
    var newApp by rememberSaveable { mutableStateOf("") }

    val addCurrent: () -> Unit = {
        val id = newApp.trim()
        if (id.isNotEmpty() && !excludedApps.contains(id)) {
            onExcludedAppsChange(excludedApps + id)
        }
        newApp = ""
    }

    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = stringResource(R.string.setting_excluded_apps_subtitle),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = newApp,
                onValueChange = { newApp = it },
                placeholder = { Text("com.example.app") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { addCurrent() }),
                modifier = Modifier.weight(1f),
            )
            // ulxa: add-item action → CopyPasteButton(primary) per styleguide primary-button.
            CopyPasteButton(
                onClick = addCurrent,
                variant = ButtonVariant.PRIMARY,
                enabled = newApp.trim().isNotEmpty(),
            ) {
                Text(stringResource(R.string.action_add))
            }
        }
        if (excludedApps.isNotEmpty()) {
            FlowRow(modifier = Modifier.fillMaxWidth()) {
                excludedApps.forEach { bundleId ->
                    InputChip(
                        selected = false,
                        onClick = { onExcludedAppsChange(excludedApps.filterNot { it == bundleId }) },
                        // CopyPaste-g5u1: dropped trailing close icon — label conveys the
                        // remove action via the chip's onClick (icon beside text removed).
                        label = { Text(bundleId) },
                    )
                }
            }
        }
    }
}

@Composable
internal fun StorageTab(
    maxTextSizeBytes: Long,
    onMaxTextSizeBytesChange: (Long) -> Unit,
    maxImageSizeBytes: Long,
    onMaxImageSizeBytesChange: (Long) -> Unit,
    maxFileSizeBytes: Long,
    onMaxFileSizeBytesChange: (Long) -> Unit,
    storageQuotaBytes: Long,
    onStorageQuotaBytesChange: (Long) -> Unit,
    sensitiveTtlSecs: Long,
    onSensitiveTtlSecsChange: (Long) -> Unit,
    maxItems: Long,
    onMaxItemsChange: (Long) -> Unit,
    excludedApps: List<String>,
    onExcludedAppsChange: (List<String>) -> Unit,
    // CopyPaste-wuek NG-1: clear-all in Settings (canonical parity with macOS Settings → Storage → Data).
    onClearHistory: () -> Unit,
    // CopyPaste-12f0: degraded-DB recovery — wipes the entire repository (macOS parity).
    onResetDatabase: () -> Unit,
    // CopyPaste-8jx8: export clipboard history as JSON (plaintext) via SAF.
    // CopyPaste-crh3.40: carries the includeSensitive toggle value (parity with macOS).
    onExportHistory: (includeSensitive: Boolean) -> Unit = {},
    // CopyPaste-8jx8: import clipboard history from a JSON export file via SAF.
    onImportHistory: () -> Unit = {},
    // CopyPaste-bdac.42: compact (VACUUM) the SQLCipher database (macOS parity).
    // Null → not yet available (no FFI vacuum entry point on Android yet).
    onVacuumDatabase: (() -> Unit)? = null,
    // Wave 3: transient loading state for the export/import/vacuum actions, hoisted
    // in SettingsActivity (set true before the coroutine, false in finally).
    exportInFlight: Boolean = false,
    importInFlight: Boolean = false,
    vacuumInFlight: Boolean = false,
) {
    Column {
        SectionLabel(stringResource(R.string.section_storage_limits))
        SettingsCard {
            Column {
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_text_size_label),
                    stepValues = TEXT_SIZE_STEP_VALUES,
                    stepLabels = TEXT_SIZE_STEP_LABELS,
                    currentValue = maxTextSizeBytes,
                    onRelease = onMaxTextSizeBytesChange,
                )
                SettingsCardDivider()
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_image_size_label),
                    stepValues = IMAGE_SIZE_STEP_VALUES,
                    stepLabels = IMAGE_SIZE_STEP_LABELS,
                    currentValue = maxImageSizeBytes,
                    onRelease = onMaxImageSizeBytesChange,
                )
                SettingsCardDivider()
                // C-P1-1: max clip file size — binary MiB steps (cap 100 MiB), macOS parity.
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_file_size_label),
                    stepValues = FILE_SIZE_STEP_VALUES,
                    stepLabels = FILE_SIZE_STEP_LABELS,
                    currentValue = maxFileSizeBytes,
                    onRelease = onMaxFileSizeBytesChange,
                )
                SettingsCardDivider()
                SteppedSliderRow(
                    label = stringResource(R.string.setting_storage_quota_label),
                    stepValues = QUOTA_STEP_VALUES,
                    stepLabels = QUOTA_STEP_LABELS,
                    currentValue = storageQuotaBytes,
                    onRelease = onStorageQuotaBytesChange,
                )
                SettingsCardDivider()
                // C-P1-1: sensitive auto-clear TTL — stepped, 0 = disabled sentinel. macOS parity.
                SteppedSliderRow(
                    label = stringResource(R.string.setting_sensitive_ttl_label),
                    stepValues = SENSITIVE_TTL_STEP_VALUES,
                    stepLabels = SENSITIVE_TTL_STEP_LABELS,
                    currentValue = sensitiveTtlSecs,
                    onRelease = onSensitiveTtlSecsChange,
                )
                SettingsCardDivider()
                // §6/§10 "Maximum stored items" slider — Unlimited sentinel = 100_000.
                //
                // CopyPaste-bdac.88/crh3.39: this is a STORED (destructive) cap. Unlike
                // macOS, whose "Max history items" slider is a DISPLAY-ONLY filter that
                // deletes nothing (crates/copypaste-ui/.../tabs/StorageTab.tsx), lowering
                // this slider permanently tombstones older UNPINNED items via
                // ClipboardRepository.pruneToLimits (continuously enforced after every
                // insert — crh3.108). The label + subtitle make the deletion explicit, and
                // SettingsActivity gates a REDUCTION behind a confirmation dialog.
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_items_label),
                    stepValues = MAX_ITEMS_STEP_VALUES,
                    stepLabels = MAX_ITEMS_STEP_LABELS,
                    currentValue = maxItems,
                    onRelease = onMaxItemsChange,
                )
                Text(
                    text = stringResource(R.string.setting_max_items_subtitle),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // ── EXCLUDED APPS ─────────────────────────────────────────────────
        SectionLabel(stringResource(R.string.setting_excluded_apps_label))
        SettingsCard {
            // C-P1-1: excluded apps — editable list (text input + Add + removable chips).
            ExcludedAppsRow(
                excludedApps = excludedApps,
                onExcludedAppsChange = onExcludedAppsChange,
            )
        }

        // CopyPaste-crh3.40: Include-sensitive-items toggle — UI-local state (not persisted),
        // matching the macOS export toggle which resets to OFF each time the Settings view
        // opens. Default OFF is the safe choice: sensitive items stay out of plaintext
        // export files unless the user explicitly opts in.
        var includeSensitiveExport by rememberSaveable { mutableStateOf(false) }

        // ── DATA — destructive actions (CopyPaste-wuek NG-1: parity with macOS) ──
        // Canonical per PARITY-SPEC §8: destructive data operations belong in
        // Settings, matching the Apple HIG and macOS Settings → Storage → Data.
        // Android previously only had "Clear All" in the History overflow menu (NG-1);
        // this section adds it to Settings so both platforms match.
        var showClearHistoryConfirm by remember { mutableStateOf(false) }
        if (showClearHistoryConfirm) {
            GlassAlertDialog(
                onDismissRequest = { showClearHistoryConfirm = false },
                title = { Text(stringResource(R.string.dialog_clear_all_title)) },
                text = { Text(stringResource(R.string.setting_clear_history_label)) },
                confirmButton = {
                    CopyPasteButton(
                        onClick = {
                            showClearHistoryConfirm = false
                            onClearHistory()
                        },
                        variant = ButtonVariant.DANGER,
                    ) {
                        Text(
                            text = stringResource(R.string.dialog_confirm),
                        )
                    }
                },
                dismissButton = {
                    CopyPasteButton(onClick = { showClearHistoryConfirm = false }, variant = ButtonVariant.GHOST) {
                        Text(stringResource(R.string.dialog_cancel))
                    }
                },
            )
        }
        // CopyPaste-12f0: Reset-database dialog (degraded-DB recovery, macOS parity).
        var showResetDbConfirm by remember { mutableStateOf(false) }
        if (showResetDbConfirm) {
            GlassAlertDialog(
                onDismissRequest = { showResetDbConfirm = false },
                title = { Text(stringResource(R.string.dialog_reset_db_title)) },
                text = { Text(stringResource(R.string.dialog_reset_db_body)) },
                confirmButton = {
                    CopyPasteButton(
                        onClick = {
                            showResetDbConfirm = false
                            onResetDatabase()
                        },
                        variant = ButtonVariant.DANGER,
                    ) {
                        Text(
                            text = stringResource(R.string.btn_reset_db),
                        )
                    }
                },
                dismissButton = {
                    CopyPasteButton(onClick = { showResetDbConfirm = false }, variant = ButtonVariant.GHOST) {
                        Text(stringResource(R.string.dialog_cancel))
                    }
                },
            )
        }
        SectionLabel(stringResource(R.string.section_data))
        SettingsCard {
            // CopyPaste-8jx8: Export history — produces a JSON file with text items
            // via the Storage Access Framework (ACTION_CREATE_DOCUMENT).
            // CopyPaste-crh3.40: Include-sensitive-items toggle (parity with macOS).
            Column(modifier = Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = stringResource(R.string.setting_export_history_label),
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        Text(
                            text = stringResource(R.string.setting_export_history_subtitle),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    CopyPasteButton(
                        onClick = { onExportHistory(includeSensitiveExport) },
                        variant = ButtonVariant.PRIMARY,
                        enabled = !exportInFlight,
                    ) {
                        if (exportInFlight) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(16.dp),
                                strokeWidth = 2.dp,
                                color = LocalContentColor.current,
                            )
                        } else {
                            Text(stringResource(R.string.action_export))
                        }
                    }
                }
                // Include-sensitive toggle row — mirrors macOS "Include sensitive items" checkbox.
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = stringResource(R.string.setting_export_include_sensitive_label),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                    )
                    IdeSwitch(
                        checked = includeSensitiveExport,
                        onCheckedChange = { includeSensitiveExport = it },
                        name = stringResource(R.string.setting_export_include_sensitive_label),
                    )
                }
                // Warning text — only visible when toggle is ON (mirrors macOS plaintext warning).
                if (includeSensitiveExport) {
                    Text(
                        text = stringResource(R.string.setting_export_include_sensitive_warning),
                        color = MaterialTheme.colorScheme.tertiary,
                    )
                }
            }
            SettingsCardDivider()
            // CopyPaste-8jx8: Import history — reads a previously exported JSON file
            // and inserts new items (deduplication by ID, re-encrypted with device key).
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = stringResource(R.string.setting_import_history_label),
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = stringResource(R.string.setting_import_history_subtitle),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                CopyPasteButton(
                    onClick = onImportHistory,
                    variant = ButtonVariant.PRIMARY,
                    enabled = !importInFlight,
                ) {
                    if (importInFlight) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(16.dp),
                            strokeWidth = 2.dp,
                            color = LocalContentColor.current,
                        )
                    } else {
                        Text(stringResource(R.string.action_import))
                    }
                }
            }
            SettingsCardDivider()
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = stringResource(R.string.setting_clear_history_label),
                    color = MaterialTheme.colorScheme.onSurface,
                )
                CopyPasteButton(
                    onClick = { showClearHistoryConfirm = true },
                    variant = ButtonVariant.DANGER,
                ) {
                    Text(stringResource(R.string.btn_clear_history))
                }
            }
            SettingsCardDivider()
            // CopyPaste-12f0: Reset database — degraded-DB recovery (macOS parity).
            // Wipes the entire clipboard store including pinned items. Intended as a
            // last resort when the DB is corrupted and normal operations fail.
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = stringResource(R.string.setting_reset_db_label),
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = stringResource(R.string.setting_reset_db_subtitle),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                CopyPasteButton(
                    onClick = { showResetDbConfirm = true },
                    variant = ButtonVariant.DANGER,
                ) {
                    Text(stringResource(R.string.btn_reset_db))
                }
            }
            SettingsCardDivider()
            // CopyPaste-bdac.42: Compact database — macOS parity (Settings → Storage → Compact).
            // Runs VACUUM on the SQLCipher DB to reclaim space after deletions.
            // onVacuumDatabase is null until the FFI exposes a vacuum entry point;
            // in that case the button is shown as disabled with an explanatory note.
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = stringResource(R.string.setting_compact_db_label),
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = stringResource(R.string.setting_compact_db_subtitle),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                CopyPasteButton(
                    onClick = { onVacuumDatabase?.invoke() },
                    variant = ButtonVariant.PRIMARY,
                    enabled = onVacuumDatabase != null && !vacuumInFlight,
                ) {
                    if (vacuumInFlight) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(16.dp),
                            strokeWidth = 2.dp,
                            color = LocalContentColor.current,
                        )
                    } else {
                        Text(stringResource(R.string.btn_compact_db))
                    }
                }
            }
        }
    }
}
