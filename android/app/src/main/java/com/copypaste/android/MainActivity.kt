package com.copypaste.android

import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Devices
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdePanel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Root activity — hosts the three-tab bottom navigation shell:
 *   0. Clips     (clipboard history, start destination)
 *   1. Devices   (pair a new device / relay)
 *   2. Settings
 *
 * On first launch (or when critical permissions are missing) the user is
 * forwarded to [OnboardingActivity]. On resume the permission check runs
 * again so the onboarding prompt can re-appear if the user revokes access.
 *
 * Clipboard monitoring:
 *   - [ClipboardService] covers API 26-28 (background allowed).
 *   - [ClipboardAccessibilityService] covers API 29+ background access
 *     (requires the user to enable it via Accessibility Settings).
 *   - The in-activity [ClipboardManager] listener below covers the foreground
 *     window while this activity is visible (all API levels).
 */
class MainActivity : ComponentActivity() {

    private val viewModel: ClipboardViewModel by viewModels()
    private lateinit var clipboardManager: ClipboardManager
    private lateinit var repository: ClipboardRepository
    private lateinit var settings: Settings
    private lateinit var syncManager: SyncManager

    /**
     * L10: whether the onboarding screen has already been forwarded to during
     * this Activity's lifetime. Instance-scoped (was a process-static `var`,
     * which suppressed onboarding forever after the first launch even across
     * fresh Activity instances). Persisted into savedInstanceState so a config
     * change / process-death restore does not re-trigger onboarding mid-task.
     */
    private var onboardingShownThisSession = false

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener

        // Image branch: check all MIME types before falling through to text.
        // M7: lifecycleScope is used here too so the coroutine is cancelled in onDestroy.
        val imageMime = (0 until clip.description.mimeTypeCount)
            .map { clip.description.getMimeType(it) }
            .firstOrNull { it.startsWith("image/") }
        if (imageMime != null) {
            val uri = clip.getItemAt(0)?.uri
            if (uri != null) {
                lifecycleScope.launch(Dispatchers.IO) {
                    ClipboardService.captureImageClip(this@MainActivity, uri, imageMime, settings, repository, syncManager)
                }
            }
            return@OnPrimaryClipChangedListener
        }

        val text = clip.getItemAt(0)?.text?.toString() ?: return@OnPrimaryClipChangedListener
        // M7: use the Activity's lifecycleScope so the coroutine is cancelled
        // automatically in onDestroy — the old hand-rolled CoroutineScope was
        // never cancelled, leaking the Activity/ViewModel via the captured `this`.
        lifecycleScope.launch(Dispatchers.IO) { handleClipboardChange(text) }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Edge-to-edge: the bottom NavigationBar and each tab's TopAppBar apply
        // their own system-bar insets so nothing is clipped on notched phones.
        enableEdgeToEdge()

        onboardingShownThisSession =
            savedInstanceState?.getBoolean(KEY_ONBOARDING_SHOWN, false) ?: false

        settings = Settings(this)
        repository = ClipboardRepository(this)
        val relayClient = RelayClient(settings.relayUrl)
        syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboardManager.addPrimaryClipChangedListener(clipListener)

        startClipboardServiceIfPossible()

        setContent {
            CopyPasteTheme {
                MainShell(viewModel = viewModel)
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // Re-check permissions on every resume; open onboarding if needed.
        if (!onboardingShownThisSession && !OnboardingActivity.allCriticalGranted(this)) {
            onboardingShownThisSession = true
            startActivity(Intent(this, OnboardingActivity::class.java))
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putBoolean(KEY_ONBOARDING_SHOWN, onboardingShownThisSession)
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        super.onDestroy()
    }

    private fun startClipboardServiceIfPossible() {
        try {
            ContextCompat.startForegroundService(this, Intent(this, ClipboardService::class.java))
            Log.d(TAG, "ClipboardService start requested")
        } catch (e: Exception) {
            Log.w(TAG, "ClipboardService start failed: ${e.javaClass.simpleName} ${e.message}")
        }
    }

    private suspend fun handleClipboardChange(text: String) {
        if (text.isBlank()) return
        val sensitive = try { isSensitive(text) } catch (_: UnsatisfiedLinkError) { false }
        if (sensitive) return
        val storedId = repository.storeItem(text, settings.encryptionKey)
        if (storedId.isNotEmpty()) {
            Log.d(TAG, "MainActivity stored clip")
            viewModel.loadItems() // refresh the Clips tab
        }
    }

    companion object {
        private const val TAG = "MainActivity"
        private const val KEY_ONBOARDING_SHOWN = "onboarding_shown_this_session"
    }
}

// ── Navigation structure ───────────────────────────────────────────────────────

private enum class NavTab(val label: String, val icon: ImageVector) {
    CLIPS("Clips", Icons.Filled.ContentPaste),
    DEVICES("Devices", Icons.Filled.Devices),
    SETTINGS("Settings", Icons.Filled.Settings),
}

@Composable
private fun MainShell(viewModel: ClipboardViewModel) {
    var selectedTab by rememberSaveable { mutableIntStateOf(NavTab.CLIPS.ordinal) }

    Scaffold(
        containerColor = IdeBg,
        // The NavigationBar (bottomBar) consumes the navigation-bar inset itself.
        // We zero the Scaffold's *content* insets so the TOP (status-bar / cutout)
        // inset is NOT also added to innerPadding — each embedded screen's own
        // TopAppBar already applies that top inset, and applying it twice would
        // push the header down by a doubled status-bar height. Without this the
        // inner screens would be either double-inset (here) or clipped (if the
        // TopAppBar inset were removed). See HistoryScreen / PairScreen / etc.
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        bottomBar = {
            NavigationBar(
                containerColor = IdePanel,
            ) {
                NavTab.entries.forEachIndexed { index, tab ->
                    NavigationBarItem(
                        selected = selectedTab == index,
                        onClick = { selectedTab = index },
                        icon = { Icon(tab.icon, contentDescription = tab.label) },
                        label = { Text(tab.label) }
                    )
                }
            }
        }
    ) { innerPadding ->
        when (NavTab.entries[selectedTab]) {
            NavTab.CLIPS -> HistoryScreen(
                viewModel = viewModel,
                modifier = Modifier.padding(innerPadding),
                showBackButton = false,
                onBack = {}
            )
            NavTab.DEVICES -> PairScreen(
                modifier = Modifier.padding(innerPadding),
                showBackButton = false,
                onBack = {}
            )
            NavTab.SETTINGS -> SettingsScreen(
                modifier = Modifier.padding(innerPadding),
                showBackButton = false,
                onBack = {}
            )
        }
    }
}
