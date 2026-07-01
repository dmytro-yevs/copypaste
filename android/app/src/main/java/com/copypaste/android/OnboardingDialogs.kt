package com.copypaste.android

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.GlassAlertDialog

/**
 * Dialog shown on the onboarding screen when the previous run ended in an
 * uncaught crash — offers to export logs or dismiss. Moved verbatim out of
 * OnboardingActivity.kt (CopyPaste-vp63.41).
 */
@Composable
internal fun CrashDetectedDialog(
    onExport: () -> Unit,
    onDismiss: () -> Unit,
) {
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.crash_detected_title)) },
        text = { Text(stringResource(R.string.crash_detected_message)) },
        confirmButton = {
            CopyPasteButton(onClick = onExport, variant = ButtonVariant.PRIMARY) {
                Text(stringResource(R.string.crash_detected_export))
            }
        },
        dismissButton = {
            CopyPasteButton(onClick = onDismiss, variant = ButtonVariant.GHOST) {
                Text(stringResource(R.string.crash_detected_dismiss))
            }
        }
    )
}
