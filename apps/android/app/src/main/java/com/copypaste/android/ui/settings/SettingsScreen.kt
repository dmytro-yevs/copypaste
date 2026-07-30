package com.copypaste.android.ui.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.copypaste.android.R

/**
 * Settings. Minimal on purpose.
 *
 * Every preference on this screen is one the app can actually honour today.
 * The rest of v1's settings surface — poll interval, exclusion lists, cloud
 * credentials — describes machinery this build does not have, and a switch that
 * does nothing is worse than an absent one.
 *
 * A11Y-8: the toggle exposes its state through `Switch`, which Compose already
 * reports as a `ToggleableState`; the row's label is bound to it so TalkBack
 * announces name and state together.
 */
@Composable
fun SettingsScreen(
    captureOnOpen: Boolean,
    onCaptureOnOpenChange: (Boolean) -> Unit,
    serviceRunning: Boolean,
    onServiceRunningChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(0.dp),
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(contentPadding)
            .padding(horizontal = 16.dp),
    ) {
        SettingRow(
            title = stringResource(R.string.settings_capture_on_open),
            body = stringResource(R.string.settings_capture_on_open_body),
            checked = captureOnOpen,
            onCheckedChange = onCaptureOnOpenChange,
        )

        HorizontalDivider(Modifier.padding(vertical = 8.dp))

        SettingRow(
            title = stringResource(R.string.settings_service),
            body = stringResource(R.string.settings_service_body),
            checked = serviceRunning,
            onCheckedChange = onServiceRunningChange,
        )

        HorizontalDivider(Modifier.padding(vertical = 8.dp))

        // Not a setting — a statement of what the platform allows, on the
        // screen where a user would otherwise go looking for a "capture in the
        // background" switch and conclude the app is broken for not having one.
        Card(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
            Column(Modifier.padding(16.dp)) {
                Text(
                    text = stringResource(R.string.settings_background_title),
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = stringResource(R.string.settings_background_body),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
        }

        Text(
            text = stringResource(R.string.settings_storage_note),
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(vertical = 16.dp),
        )
    }
}

@Composable
private fun SettingRow(
    title: String,
    body: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    androidx.compose.foundation.layout.Row(
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(text = title, style = MaterialTheme.typography.bodyLarge)
            Text(text = body, style = MaterialTheme.typography.bodySmall)
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            // A11Y-9: the control needs a name of its own; the visual label is
            // a sibling, not its accessible name.
            modifier = Modifier.semantics { contentDescription = title },
        )
    }
}
