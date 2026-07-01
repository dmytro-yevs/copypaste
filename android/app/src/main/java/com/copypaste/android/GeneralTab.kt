package com.copypaste.android

import android.content.Intent
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.copypaste.android.ui.theme.SectionLabel

@Composable
internal fun GeneralTab(
    privateMode: Boolean,
    onPrivateModeChange: (Boolean) -> Unit,
    syncEnabled: Boolean,
    onSyncEnabledChange: (Boolean) -> Unit,
    collectPublicIp: Boolean,
    onCollectPublicIpChange: (Boolean) -> Unit,
    pasteAsPlainText: Boolean,
    onPasteAsPlainTextChange: (Boolean) -> Unit,
    logcatEnabled: Boolean,
    onLogcatEnabledChange: (Boolean) -> Unit,
    logcatStatus: LogcatCaptureStatus,
    ctx: android.content.Context,
    // CopyPaste-5917.17: replaces android.widget.Toast in AdbCmdRow and log-export error path.
    // Called with a human-readable message; the caller (SettingsScreen) routes it to GlassToastHost.
    onToastRequest: (String) -> Unit = {},
) {
    Column {
        // ── GENERAL section card ──────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_general))
        SettingsCard {
            SettingsRow(
                title = stringResource(R.string.setting_private_mode_title),
                subtitle = stringResource(R.string.setting_private_mode_subtitle),
                checked = privateMode,
                onCheckedChange = onPrivateModeChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sync_enabled_title),
                subtitle = stringResource(R.string.setting_sync_enabled_subtitle),
                checked = syncEnabled,
                onCheckedChange = onSyncEnabledChange,
            )
        }

        // ── PRIVACY section card ──────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_privacy))
        SettingsCard {
            // "Discover public IP" — allow a one-off STUN request to learn this
            // device's public IP (shown in the device-info card). Mirrors macOS.
            SettingsRow(
                title = stringResource(R.string.setting_collect_public_ip_title),
                subtitle = stringResource(R.string.setting_collect_public_ip_subtitle),
                checked = collectPublicIp,
                onCheckedChange = onCollectPublicIpChange,
            )
            SettingsCardDivider()
            // "Paste as plain text" — strip rich formatting (RTF/HTML) on paste. Mirrors macOS.
            SettingsRow(
                title = stringResource(R.string.setting_paste_as_plain_text_title),
                subtitle = stringResource(R.string.setting_paste_as_plain_text_subtitle),
                checked = pasteAsPlainText,
                onCheckedChange = onPasteAsPlainTextChange,
            )
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_permissions_title),
                subtitle = stringResource(R.string.setting_permissions_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, PermissionsSettingsActivity::class.java))
                }
            )
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_devices_title),
                subtitle = stringResource(R.string.setting_devices_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, DevicesActivity::class.java))
                }
            )
            // CopyPaste-bdac.7: BackgroundCaptureSetup moved from Storage tab to
            // General tab — parity/logical grouping: capture behaviour belongs with
            // other general/permissions settings, not alongside storage sliders.
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_bg_capture_title),
                subtitle = stringResource(R.string.setting_bg_capture_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, BackgroundCaptureSetupActivity::class.java))
                }
            )
        }

        // ── DIAGNOSTICS section card ──────────────────────────────────────
        SectionLabel(stringResource(R.string.section_diagnostics))
        SettingsCard {
            // CopyPaste-5917.77: NavIcons.Logs (doc.text SF-like icon) — parity with macOS Logs tab.
            // Android intentionally routes Logs via Settings rather than a bottom-nav tab;
            // see NavTabTest which asserts the 3-tab (Clips/Devices/Settings) nav is canonical.
            SettingsNavRow(
                title = stringResource(R.string.log_viewer_button),
                subtitle = stringResource(R.string.log_viewer_description),
                onClick = {
                    ctx.startActivity(Intent(ctx, LogViewerActivity::class.java))
                }
            )
            SettingsCardDivider()
            DiagnosticsNavRow(
                title = stringResource(R.string.log_export_button),
                subtitle = stringResource(R.string.log_export_description),
                buttonLabel = stringResource(R.string.log_export_button),
                // CopyPaste-5917.17: pass onError so failures route through GlassToastHost
                // instead of the android.widget.Toast fallback in LogExportHelper.
                onClick = { LogExportHelper.shareLogsZip(ctx, onError = onToastRequest) }
            )
        }

        // ── BACKGROUND CAPTURE (ADB) section card ────────────────────────
        SectionLabel(stringResource(R.string.bg_adb_section_title))
        SettingsCard {
            // Explainer
            Text(
                text = stringResource(R.string.bg_adb_explainer),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            // Live status line
            AdbCaptureStatusLine(logcatStatus = logcatStatus, ctx = ctx)
            SettingsCardDivider()
            // Toggle: user can disable logcat capture even when READ_LOGS is granted
            SettingsRow(
                title = stringResource(R.string.setting_logcat_capture_title),
                subtitle = stringResource(R.string.setting_logcat_capture_subtitle),
                checked = logcatEnabled,
                onCheckedChange = onLogcatEnabledChange,
            )
            SettingsCardDivider()
            // Tap-to-copy ADB commands
            // CopyPaste-5917.17: pass onToastRequest so the copy feedback routes through
            // GlassToastHost instead of android.widget.Toast.
            AdbCaptureCommandRows(ctx = ctx, onToastRequest = onToastRequest)
        }

        // ── ABOUT (last General entry) ────────────────────────────────────
        // Android intentionally routes About via Settings rather than a bottom-nav tab;
        // see NavTabTest which asserts the 3-tab (Clips/Devices/Settings) nav is canonical.
        SettingsCard {
            SettingsNavRow(
                title = stringResource(R.string.title_about),
                subtitle = stringResource(R.string.about_tagline),
                onClick = {
                    ctx.startActivity(Intent(ctx, AboutActivity::class.java))
                }
            )
        }
    }
}
