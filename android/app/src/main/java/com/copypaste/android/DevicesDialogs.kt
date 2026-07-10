package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.ideTextFieldColors

/**
 * The Devices screen's dialog set — unpair confirm, the two-path revoke
 * confirm, the revoke+rotate passphrase dialog, the revoke-error surface, the
 * "revoke all" confirm, the SAS pairing modal, and the scan-error surface.
 *
 * CopyPaste-vp63.39: extracted verbatim (appearance + logic unchanged) from
 * the former `DevicesScreen` god-composable in DevicesActivity.kt. Each
 * dialog is data-driven off [controller]'s state (unpair/revoke state lives on
 * [DevicesController.revoke]); [settings] is only needed by [SasPairingDialog].
 * Call once from [DevicesScreen] — every dialog no-ops (renders nothing) when
 * its target state is null/false.
 */
@Composable
fun DevicesDialogs(controller: DevicesController, settings: Settings) {
    UnpairConfirmDialog(controller)
    RevokeConfirmDialog(controller)
    RevokeRotateDialog(controller)
    RevokeErrorDialog(controller)
    RevokeAllConfirmDialog(controller)
    SasPairingModalHost(controller, settings)
    ScanErrorDialog(controller)
}

// ── Unpair confirmation ──────────────────────────────────────────────────────
@Composable
private fun UnpairConfirmDialog(controller: DevicesController) {
    val target = controller.revoke.unpairTarget ?: return
    val cp = LocalCpColors.current
    // §9.9 destructive-modal dialog — appearance only; unpair logic unchanged.
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.unpairTarget = null },
        // CopyPaste-bdac.51: standardized to "Unpair" — was "Forget" (terminology conflict).
        title = { Text(stringResource(R.string.dialog_forget_device_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(CpSpacing.s4)) {
                Text(
                    stringResource(R.string.dialog_forget_device_body, target.displayName()),
                    style = CpTypography.body,
                )
                // android-devices spec "Unpair/Revoke warning copy remains visible".
                Text(
                    stringResource(R.string.devices_local_only_notice),
                    style = CpTypography.meta,
                    color = cp.faint,
                )
            }
        },
        confirmButton = {
            CopyPasteButton(
                onClick = { controller.revoke.confirmUnpair(target) },
                variant = ButtonVariant.DANGER,
            ) { Text(stringResource(R.string.dialog_forget_btn)) }
        },
        dismissButton = {
            CopyPasteButton(
                onClick = { controller.revoke.unpairTarget = null },
                variant = ButtonVariant.GHOST,
            ) { Text(stringResource(R.string.dialog_cancel)) }
        },
    )
}

// ── Revoke confirmation (CopyPaste-8qcm: two-path dialog) ─────────────────
// First dialog: presents the user with two revoke options:
//   • "Revoke only"        → plain audit + roster removal (RevokeMode.AUDIT_ONLY).
//   • "Revoke & rotate key" → opens the passphrase dialog (RevokeMode.REVOKE_AND_ROTATE).
//
// The "Revoke only" path preserves the atomic CopyPaste-94o4 ordering:
//   revokeDeviceAudit (IO) → removePeer only if audit succeeded.
@Composable
private fun RevokeConfirmDialog(controller: DevicesController) {
    val target = controller.revoke.revokeTarget ?: return
    val cp = LocalCpColors.current
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.revokeTarget = null },
        title = { Text(stringResource(R.string.dialog_revoke_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(CpSpacing.s4)) {
                Text(
                    stringResource(R.string.devices_revoke_body_primary, target.displayName()),
                    style = CpTypography.body,
                )
                Text(
                    stringResource(R.string.devices_revoke_body_secondary),
                    style = CpTypography.meta,
                    color = cp.dim,
                )
                // android-devices spec "Unpair/Revoke warning copy remains visible".
                Text(
                    stringResource(R.string.devices_local_only_notice),
                    style = CpTypography.meta,
                    color = cp.faint,
                )
            }
        },
        // "Revoke & rotate key" is the primary action (right-side confirm button).
        // Tapping it closes this dialog and opens the passphrase dialog.
        confirmButton = {
            CopyPasteButton(
                onClick = { controller.revoke.openRevokeRotate(target) },
                variant = ButtonVariant.DANGER,
            ) {
                Text(stringResource(R.string.devices_btn_revoke_rotate))
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                // "Revoke only" — left; performs the plain audit+remove path.
                CopyPasteButton(
                    onClick = { controller.revoke.revokeOnly(target) },
                    variant = ButtonVariant.DANGER,
                ) { Text(stringResource(R.string.devices_btn_revoke_only)) }

                CopyPasteButton(
                    onClick = { controller.revoke.revokeTarget = null },
                    variant = ButtonVariant.GHOST,
                ) { Text(stringResource(R.string.dialog_cancel)) }
            }
        },
    )
}

// ── Revoke + rotate key passphrase dialog (CopyPaste-8qcm) ─────────────────
// Shown after the user selects "Revoke & rotate key" above. The user enters
// the new passphrase (min 8 chars); "Confirm" calls revokeDeviceAndRotateKey.
@Composable
private fun RevokeRotateDialog(controller: DevicesController) {
    val target = controller.revoke.revokeRotateTarget ?: return
    val cp = LocalCpColors.current
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.cancelRevokeRotate() },
        title = { Text(stringResource(R.string.devices_rotate_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(CpSpacing.s4)) {
                Text(
                    stringResource(R.string.devices_rotate_body),
                    style = CpTypography.body,
                )
                // Passphrase text field — skin-aware surface colors, password masking.
                OutlinedTextField(
                    value = controller.revoke.revokePassphrase,
                    onValueChange = { controller.revoke.revokePassphrase = it },
                    label = { Text(stringResource(R.string.devices_rotate_passphrase_label)) },
                    visualTransformation = PasswordVisualTransformation(),
                    singleLine = true,
                    enabled = !controller.revoke.revokeRotateInFlight,
                    colors = ideTextFieldColors(),
                    modifier = Modifier.fillMaxWidth(),
                )
                if (!isValidRotatePassphrase(controller.revoke.revokePassphrase) &&
                    controller.revoke.revokePassphrase.isNotEmpty()
                ) {
                    Text(
                        stringResource(R.string.devices_rotate_passphrase_error),
                        style = CpTypography.meta,
                        color = cp.err,
                    )
                }
            }
        },
        confirmButton = {
            CopyPasteButton(
                enabled = isValidRotatePassphrase(controller.revoke.revokePassphrase) &&
                    !controller.revoke.revokeRotateInFlight,
                onClick = { controller.revoke.confirmRevokeRotate() },
                variant = ButtonVariant.DANGER,
            ) {
                if (controller.revoke.revokeRotateInFlight) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp),
                        strokeWidth = 2.dp,
                        color = LocalContentColor.current,
                    )
                } else {
                    Text(stringResource(R.string.devices_btn_confirm_revoke_rotate))
                }
            }
        },
        dismissButton = {
            CopyPasteButton(
                enabled = !controller.revoke.revokeRotateInFlight,
                onClick = { controller.revoke.cancelRevokeRotate() },
                variant = ButtonVariant.GHOST,
            ) { Text(stringResource(R.string.dialog_cancel)) }
        },
    )
}

// ── Revoke failure surface ────────────────────────────────────────────────
@Composable
private fun RevokeErrorDialog(controller: DevicesController) {
    val msg = controller.revoke.revokeError ?: return
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.dismissRevokeError() },
        title = { Text(stringResource(R.string.dialog_revoke_incomplete_title)) },
        text = { Text(msg, style = CpTypography.body) },
        confirmButton = {
            CopyPasteButton(
                onClick = { controller.revoke.dismissRevokeError() },
                variant = ButtonVariant.GHOST,
            ) { Text(stringResource(R.string.devices_dialog_ok)) }
        },
    )
}

// ── CopyPaste-crh3.34: "Revoke all" confirmation dialog ──────────────────
// Mirrors macOS: title "Revoke all paired devices?" + two-sentence body +
// DANGER confirm button + GHOST cancel button.
@Composable
private fun RevokeAllConfirmDialog(controller: DevicesController) {
    if (!controller.revoke.revokeAllConfirmOpen) return
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.dismissRevokeAllConfirm() },
        title = { Text(stringResource(R.string.devices_revoke_all_title)) },
        text = { Text(revokeAllConfirmBody(), style = CpTypography.body) },
        confirmButton = {
            CopyPasteButton(
                enabled = !controller.revoke.revokeAllInFlight,
                // Snapshot the CURRENT roster at click time (mirrors the original
                // inline `peers.toList()` capture in the former god-composable).
                onClick = { controller.revoke.confirmRevokeAll(controller.peers) },
                variant = ButtonVariant.DANGER,
            ) {
                if (controller.revoke.revokeAllInFlight) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp),
                        strokeWidth = 2.dp,
                        color = LocalContentColor.current,
                    )
                } else {
                    Text(stringResource(R.string.devices_btn_revoke_all_confirm))
                }
            }
        },
        dismissButton = {
            CopyPasteButton(
                enabled = !controller.revoke.revokeAllInFlight,
                onClick = { controller.revoke.dismissRevokeAllConfirm() },
                variant = ButtonVariant.GHOST,
            ) { Text(stringResource(R.string.dialog_cancel)) }
        },
    )
}

// ── SAS pairing modal (port of macOS SasPairingModal) ─────────────────────
@Composable
private fun SasPairingModalHost(controller: DevicesController, settings: Settings) {
    val peer = controller.pairingPeer ?: return
    SasPairingDialog(
        peer = peer,
        settings = settings,
        onClose = { controller.closePairing() },
        onPaired = { controller.refresh() },
    )
}

// ── Scan error surface ────────────────────────────────────────────────────
@Composable
private fun ScanErrorDialog(controller: DevicesController) {
    val msg = controller.scanError ?: return
    GlassAlertDialog(
        onDismissRequest = { controller.dismissScanError() },
        title = { Text(stringResource(R.string.dialog_scanner_unavailable_title)) },
        text = { Text(msg, style = CpTypography.body) },
        confirmButton = {
            CopyPasteButton(onClick = { controller.dismissScanError() }, variant = ButtonVariant.GHOST) {
                Text(stringResource(R.string.devices_dialog_ok))
            }
        },
    )
}
