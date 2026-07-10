package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.R
import com.copypaste.android.SettingsCard
import com.copypaste.android.SettingsCardDivider
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// S9 Wave 5 — StorageTab's export/import/vacuum action-button rows, normal vs
// in-flight (CircularProgressIndicator visible per Wave 3's exportInFlight/
// importInFlight/vacuumInFlight). [com.copypaste.android.StorageTab] itself is
// a single ~350-line composable spanning six unrelated sections (size sliders,
// excluded apps, destructive-action dialogs) — snapshotting it whole would bury
// the loading-state signal in an oversized, mostly-irrelevant image and risks
// content overflowing the fixed PIXEL_5 viewport. So this reproduces StorageTab's
// exact Row/CopyPasteButton/CircularProgressIndicator structure for just the
// three action rows, using the SAME shared building blocks (SettingsCard,
// SettingsCardDivider, CopyPasteButton) with literal fixture labels — mirroring
// DevicesCardSnapshotTest's DevicesDialogFixture "representative golden"
// convention. Smell flag: this duplicates StorageTab's row layout rather than
// calling StorageTab directly; if StorageTab's action-row markup changes this
// fixture must be updated in lockstep (see kdoc, not a hidden landmine).
// ---------------------------------------------------------------------------
class StorageTabLoadingSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, actions normal`() {
        paparazzi.snapshot {
            ActionsFixture(exportInFlight = false, importInFlight = false, vacuumInFlight = false)
        }
    }

    @Test
    fun `dark theme, actions in flight`() {
        paparazzi.snapshot {
            ActionsFixture(exportInFlight = true, importInFlight = true, vacuumInFlight = true)
        }
    }
}

@Composable
private fun ActionsFixture(
    exportInFlight: Boolean,
    importInFlight: Boolean,
    vacuumInFlight: Boolean,
) {
    CopyPasteTheme(isDark = true) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(LocalCpColors.current.bg)
                .padding(16.dp),
        ) {
            SettingsCard {
                Column {
                    ActionRow(
                        label = stringResource(R.string.setting_export_history_label),
                        buttonLabel = stringResource(R.string.action_export),
                        inFlight = exportInFlight,
                    )
                    SettingsCardDivider()
                    ActionRow(
                        label = stringResource(R.string.setting_import_history_label),
                        buttonLabel = stringResource(R.string.action_import),
                        inFlight = importInFlight,
                    )
                    SettingsCardDivider()
                    ActionRow(
                        label = stringResource(R.string.setting_compact_db_label),
                        buttonLabel = stringResource(R.string.btn_compact_db),
                        inFlight = vacuumInFlight,
                    )
                }
            }
        }
    }
}

@Composable
private fun ActionRow(label: String, buttonLabel: String, inFlight: Boolean) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(text = label, color = MaterialTheme.colorScheme.onSurface)
        CopyPasteButton(
            onClick = {},
            variant = ButtonVariant.PRIMARY,
            enabled = !inFlight,
        ) {
            if (inFlight) {
                CircularProgressIndicator(
                    modifier = Modifier.size(16.dp),
                    strokeWidth = 2.dp,
                    color = LocalContentColor.current,
                )
            } else {
                Text(buttonLabel)
            }
        }
    }
}
