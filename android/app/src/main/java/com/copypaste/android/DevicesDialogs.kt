package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.GlassAlertDialog

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
    // §8 glass dialog (audit #10) — appearance only; unpair logic unchanged.
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.unpairTarget = null },
        // CopyPaste-bdac.51: standardized to "Unpair" — was "Forget" (terminology conflict).
        title = { Text("Unpair device?") },
        text = {
            Text(
                "This device will no longer sync with ${target.displayName()} over P2P. " +
                "You can re-pair at any time by scanning a new QR code."
            )
        },
        confirmButton = {
            CopyPasteButton(
                onClick = { controller.revoke.confirmUnpair(target) },
                variant = ButtonVariant.DANGER,
            ) { Text("Unpair") }
        },
        dismissButton = {
            CopyPasteButton(
                onClick = { controller.revoke.unpairTarget = null },
                variant = ButtonVariant.GHOST,
            ) { Text("Cancel") }
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
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.revokeTarget = null },
        title = { Text("Revoke pairing?") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "${target.displayName()} will no longer connect over P2P and a " +
                    "revocation record is kept.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "A revoked device that still knows the sync passphrase can " +
                    "keep reading new relay and cloud items. To close that gap, " +
                    "choose “Revoke & rotate key” below.",
                    style = MaterialTheme.typography.bodySmall,
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
                Text("Revoke & rotate key")
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                // "Revoke only" — left; performs the plain audit+remove path.
                CopyPasteButton(
                    onClick = { controller.revoke.revokeOnly(target) },
                    variant = ButtonVariant.DANGER,
                ) { Text("Revoke only") }

                CopyPasteButton(
                    onClick = { controller.revoke.revokeTarget = null },
                    variant = ButtonVariant.GHOST,
                ) { Text("Cancel") }
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
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.cancelRevokeRotate() },
        title = { Text("Set new sync passphrase") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "Enter a new passphrase to rotate the sync key. All trusted " +
                    "devices will need to re-enter this passphrase to keep syncing.",
                    style = MaterialTheme.typography.bodySmall,
                )
                // Passphrase text field — skin-aware surface colors, password masking.
                OutlinedTextField(
                    value = controller.revoke.revokePassphrase,
                    onValueChange = { controller.revoke.revokePassphrase = it },
                    label = { Text("New passphrase (min 8 chars)") },
                    visualTransformation = PasswordVisualTransformation(),
                    singleLine = true,
                    enabled = !controller.revoke.revokeRotateInFlight,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (!isValidRotatePassphrase(controller.revoke.revokePassphrase) &&
                    controller.revoke.revokePassphrase.isNotEmpty()
                ) {
                    Text(
                        "Passphrase must be at least 8 characters.",
                        style = MaterialTheme.typography.labelSmall,
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
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                } else {
                    Text("Confirm revoke & rotate")
                }
            }
        },
        dismissButton = {
            CopyPasteButton(
                enabled = !controller.revoke.revokeRotateInFlight,
                onClick = { controller.revoke.cancelRevokeRotate() },
                variant = ButtonVariant.GHOST,
            ) { Text("Cancel") }
        },
    )
}

// ── Revoke failure surface ────────────────────────────────────────────────
@Composable
private fun RevokeErrorDialog(controller: DevicesController) {
    val msg = controller.revoke.revokeError ?: return
    GlassAlertDialog(
        onDismissRequest = { controller.revoke.dismissRevokeError() },
        title = { Text("Revocation incomplete") },
        text = { Text(msg) },
        confirmButton = {
            CopyPasteButton(
                onClick = { controller.revoke.dismissRevokeError() },
                variant = ButtonVariant.GHOST,
            ) { Text("OK") }
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
        title = { Text("Revoke all paired devices?") },
        text = { Text(revokeAllConfirmBody()) },
        confirmButton = {
            CopyPasteButton(
                enabled = !controller.revoke.revokeAllInFlight,
                // Snapshot the CURRENT roster at click time (mirrors the original
                // inline `peers.toList()` capture in the former god-composable).
                onClick = { controller.revoke.confirmRevokeAll(controller.peers) },
                variant = ButtonVariant.DANGER,
            ) {
                if (controller.revoke.revokeAllInFlight) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                } else {
                    Text("Revoke all")
                }
            }
        },
        dismissButton = {
            CopyPasteButton(
                enabled = !controller.revoke.revokeAllInFlight,
                onClick = { controller.revoke.dismissRevokeAllConfirm() },
                variant = ButtonVariant.GHOST,
            ) { Text("Cancel") }
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
        title = { Text("Scanner unavailable") },
        text = { Text(msg) },
        confirmButton = {
            CopyPasteButton(onClick = { controller.dismissScanError() }, variant = ButtonVariant.GHOST) { Text("OK") }
        },
    )
}
