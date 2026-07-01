package com.copypaste.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import com.copypaste.android.ui.theme.SecureWindowChrome

/**
 * History screen Activity shell.
 *
 * CopyPaste-vp63.37: this file used to hold the entire History screen —
 * `HistoryScreen` (Scaffold/state/effects), `HistoryList` (LazyColumn), the
 * in-app file picker, and every bulk-copy/save-file/open-file action body.
 * Those pieces now live in (respectively) HistoryScreen.kt,
 * HistoryScreenState.kt, HistoryList.kt, HistoryFilePicker.kt, and
 * HistoryItemActions.kt. This Activity is now just the platform shell:
 * screenshot policy + edge-to-edge + the mutation-sync hook + `setContent`.
 */
class HistoryActivity : ComponentActivity() {

    private val viewModel: ClipboardViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // CopyPaste-1g00: screenshot protection is now pref-driven (Settings.allowScreenshots).
        // SecureWindowChrome applies FLAG_SECURE centrally when allowScreenshots=false (the default).
        // The old hardcoded setFlags(FLAG_SECURE) is removed so the user's pref is honoured.
        applyScreenshotPolicy(Settings(this))
        enableEdgeToEdge()

        // CopyPaste-0qpn: wire the mutation sync hook so pin/unpin/reorder/delete/clear
        // operations propagate to peers over relay + Supabase. Delegates to
        // ClipboardService.requestMutationQueueDrain which fires a drain on the service's
        // IO scope (non-blocking, fire-and-forget). Hook is a no-op when FGS is not running.
        viewModel.onMutationSync = {
            ClipboardService.requestMutationQueueDrain()
        }

        setContent {
            SecureWindowChrome {
                HistoryScreen(
                    viewModel = viewModel,
                    onBack = { finish() }
                )
            }
        }
    }

    companion object {
        /** Fallback used only when Settings cannot be read (e.g. test context). */
        const val HISTORY_LIMIT = 50
    }
}
