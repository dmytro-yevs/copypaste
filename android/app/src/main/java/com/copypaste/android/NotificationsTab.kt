package com.copypaste.android

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.SectionLabel

/**
 * CopyPaste-1jms.18: Notifications is an intentional Android-only tab.
 *
 * macOS exposes notification preferences through the OS-level System Settings
 * (Notification Center) rather than an in-app tab. Android requires the app to
 * manage its own notification behaviour (notify-on-copy sound/vibration), so this
 * tab is a valid platform-specific addition and NOT a parity gap. It should NOT be
 * removed to match macOS; instead, the macOS SettingsView could add equivalent rows
 * if the daemon ever exposes fine-grained notification control there.
 */
@Composable
internal fun NotificationsTab(
    notifyOnCopy: Boolean,
    onNotifyOnCopyChange: (Boolean) -> Unit,
    soundOnCopy: Boolean,
    onSoundOnCopyChange: (Boolean) -> Unit,
) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        SectionLabel(stringResource(R.string.section_notifications))
        SettingsCard {
            SettingsRow(
                title = stringResource(R.string.setting_notify_on_copy_title),
                subtitle = stringResource(R.string.setting_notify_on_copy_subtitle),
                checked = notifyOnCopy,
                onCheckedChange = onNotifyOnCopyChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sound_on_copy_title),
                subtitle = stringResource(R.string.setting_sound_on_copy_subtitle),
                checked = soundOnCopy,
                onCheckedChange = onSoundOnCopyChange,
            )
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}
