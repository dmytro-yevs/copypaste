package com.copypaste.android

import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
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
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdePanel
import kotlinx.coroutines.CoroutineScope
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
    private val scope = CoroutineScope(Dispatchers.Main)

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener
        val text = clip.getItemAt(0)?.text?.toString() ?: return@OnPrimaryClipChangedListener
        scope.launch(Dispatchers.IO) { handleClipboardChange(text) }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        settings = Settings(this)
        repository = ClipboardRepository(this)
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
        val stored = repository.storeItem(text, settings.encryptionKey)
        if (stored) {
            Log.d(TAG, "MainActivity stored clip")
            viewModel.loadItems() // refresh the Clips tab
        }
    }

    companion object {
        private const val TAG = "MainActivity"
        // Suppress repeat onboarding launches within the same Activity instance.
        private var onboardingShownThisSession = false
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
