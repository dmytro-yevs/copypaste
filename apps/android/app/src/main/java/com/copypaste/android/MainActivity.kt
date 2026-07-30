package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import com.copypaste.android.ui.CopyPasteRoot
import kotlinx.coroutines.launch

/**
 * The only activity.
 *
 * Three platform obligations live here because this is where they arrive:
 *
 * * **Screen-capture protection** (manifest 06 INV-35: on by default).
 *   `FLAG_SECURE` keeps the history out of screenshots, out of the recent-apps
 *   thumbnail, and off a cast or screen-share. A clipboard manager's window is
 *   a list of everything the user has copied; the recents thumbnail alone would
 *   put that behind the lock screen preview.
 * * **Notification permission.** API 33+ makes it a runtime grant, and without
 *   it a foreground service runs with a suppressed notification — a background
 *   service the user cannot see or stop. Asked for before the service starts,
 *   and a refusal simply means the service does not start.
 * * **Foreground capture.** The only route Android permits: the clipboard is
 *   readable when this window has focus, and at no other time. See
 *   [com.copypaste.android.service.SyncService] for the full reasoning.
 */
class MainActivity : ComponentActivity() {

    private var notificationsGranted by mutableStateOf(false)

    private val requestNotifications = registerForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> notificationsGranted = granted }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // INV-35. Set before the first frame so there is no window between
        // launch and protection.
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE,
        )

        notificationsGranted =
            Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
                checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) ==
                android.content.pm.PackageManager.PERMISSION_GRANTED

        val app = application as CopyPasteApp

        setContent {
            CopyPasteRoot(
                repository = app.repository,
                notificationsGranted = notificationsGranted,
                onRequestNotifications = ::requestNotificationPermission,
                sharedText = sharedTextFrom(intent),
            )
        }

        // Capture on focus. `repeatOnLifecycle(RESUMED)` is the closest thing
        // Android gives us to "the window has focus", and focus is the exact
        // condition under which `getPrimaryClip` returns anything at all.
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.RESUMED) {
                app.repository.getOrNull()?.let { runCatching { it.captureFromClipboard() } }
            }
        }
    }

    /**
     * A share or a text-selection action arrived while the app was already
     * open. `launchMode="singleTask"` routes it here rather than to a second
     * activity instance.
     */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            requestNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
        } else {
            notificationsGranted = true
        }
    }

    /**
     * Text handed to the app by the share sheet or the selection menu.
     *
     * These are the sanctioned capture routes on Android 10+ — the user
     * explicitly sends the content, so no clipboard read is involved and no
     * restriction applies.
     */
    private fun sharedTextFrom(intent: Intent?): String? = when (intent?.action) {
        Intent.ACTION_SEND ->
            intent.getStringExtra(Intent.EXTRA_TEXT)

        Intent.ACTION_PROCESS_TEXT ->
            intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()

        else -> null
    }
}
