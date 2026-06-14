package com.copypaste.android

import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentPaste
import androidx.compose.material.icons.outlined.Devices
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import com.copypaste.android.ui.SyncStatusBadge
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.glassContainerColor
import com.copypaste.android.ui.theme.rememberTranslucency
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
 *   - [LogcatCaptureService] covers API 29+ background access via the logcat+overlay path
 *     (requires READ_LOGS via adb + SYSTEM_ALERT_WINDOW).
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

    // CopyPaste-l080: request POST_NOTIFICATIONS BEFORE starting the FGS on
    // Android 13+ first launch. Whatever the result, start the service afterwards
    // so capture still works — but if granted, the FGS status notification (Pause/
    // Resume) is now actually visible instead of being silently dropped.
    private val notifLauncher = registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.RequestPermission()
    ) { granted ->
        Log.d(TAG, "MainActivity POST_NOTIFICATIONS granted=$granted")
        startClipboardServiceIfPossible()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // AB-7: FLAG_SECURE. This window hosts the clipboard history (Clips tab),
        // which can contain passwords, tokens, and other sensitive copied text.
        // Block screenshots and keep the contents out of the recents/overview
        // thumbnail. Set before setContent so the flag covers the whole lifetime
        // (PairActivity already does the same for its QR screen).
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE,
        )
        // Edge-to-edge: the bottom NavigationBar and each tab's TopAppBar apply
        // their own system-bar insets so nothing is clipped on notched phones.
        enableEdgeToEdge()

        onboardingShownThisSession =
            savedInstanceState?.getBoolean(KEY_ONBOARDING_SHOWN, false) ?: false

        settings = Settings(this)
        repository = ClipboardRepository(this)
        // [P2] Wrap sync setup in try/catch so a constructor failure does not
        // prevent the clipboard listener from registering. Falls back to a stub
        // RelayClient pointing at an empty base URL; the relay cloud path is already
        // disabled (SyncBackend.RELAY is a no-op) so the listener still works.
        try {
            val relayClient = RelayClient(settings.relayUrl)
            // [P1] The relay bearer token (used by RelayClient.uploadItem) is obtained
            // from RelayClient.registerDevice() at pairing time and was never persisted
            // to Settings — there is no relayToken field on Settings. The SyncBackend.RELAY
            // cloud upload path is DISABLED (ClipboardService.notifySyncManager logs a
            // warning and returns without calling uploadItem), so token="" causes no 401s
            // in practice. If the relay path is ever re-enabled, store the Device.token
            // returned by registerDevice() in Settings and pass it here.
            syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
        } catch (e: Exception) {
            Log.w(TAG, "SyncManager init failed — proceeding without relay sync: ${e.javaClass.simpleName} ${e.message}")
            val fallback = RelayClient("")
            syncManager = SyncManager(fallback, settings.deviceId, token = "", settings = settings)
        }
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboardManager.addPrimaryClipChangedListener(clipListener)

        // CopyPaste-l080: on Android 13+ first launch, request POST_NOTIFICATIONS
        // BEFORE starting the foreground service so its status notification is
        // visible (previously the FGS started first and the notification was
        // silently dropped — no Pause/Resume — until the next launch after grant).
        // The launcher callback starts the service whatever the user chooses; if
        // already granted (or pre-Tiramisu) we start it directly.
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU &&
            !NotificationPermissionHelper.isGranted(this) &&
            !NotificationPermissionHelper.isPermanentlyDenied(this)
        ) {
            NotificationPermissionHelper.markRequested(this)
            notifLauncher.launch(android.Manifest.permission.POST_NOTIFICATIONS)
        } else {
            startClipboardServiceIfPossible()
        }

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
        // Route through the shared capture pipeline so foreground-captured clips
        // are counted in the notification, trigger copy sound/notification, and
        // sync — exactly like ClipboardService and LogcatCaptureService.
        // Previously this called repository.storeItem directly, skipping all of
        // bumpTodayCounter / postCopyNotification / playCopySound / notifySyncManager.
        ClipboardService.captureClip(this, text, settings, repository, syncManager)
        // Do NOT call viewModel.loadItems() here. The ViewModel's storeListener
        // (registered on SharedPreferences KEY_ITEM_IDS) already fires a loadItems()
        // automatically whenever captureClip actually stores a new item. Calling it
        // unconditionally here caused a redundant refresh on every clipboard change —
        // including suppressed echo-copies (copy-from-history taps) — and could
        // interact with HW-A3 by triggering a UI reload before the dedup window
        // had a chance to suppress all concurrent listener fires.
    }

    companion object {
        private const val TAG = "MainActivity"
        private const val KEY_ONBOARDING_SHOWN = "onboarding_shown_this_session"
    }
}

// ── Navigation structure ───────────────────────────────────────────────────────

// Internal so NavTabTest (pure-JVM unit test) can verify the tab set.
// `labelRes` is the bottom-nav label string resource. HB-6: the DEVICES tab now
// reads R.string.title_devices ("Devices") instead of the old hardcoded "Pair",
// matching the Devices screen title — pairing lives INSIDE that screen now.
internal enum class NavTab(@StringRes val labelRes: Int, val icon: ImageVector) {
    CLIPS(R.string.title_history, Icons.Outlined.ContentPaste),
    DEVICES(R.string.title_devices, Icons.Outlined.Devices),
    SETTINGS(R.string.title_settings, Icons.Outlined.Settings),
}

@Composable
private fun MainShell(viewModel: ClipboardViewModel) {
    var selectedTab by rememberSaveable { mutableIntStateOf(NavTab.CLIPS.ordinal) }
    // Unsaved-changes guard registered by SettingsScreen. When the user has
    // pending edits and tries to switch tabs via the navbar, we route the tab
    // change through this guard so the Discard/Keep-editing dialog intercepts it
    // (parity with the back-press / top-bar back-arrow guard). Null when not on
    // Settings or when there are no unsaved changes.
    var settingsNavGuard by remember {
        mutableStateOf<((proceed: () -> Unit) -> Unit)?>(null)
    }

    // §3 Translucency: read once at the shell level so the pref is consistent
    // across the NavigationBar and all child screens. CopyPasteTopBar and
    // CopyPasteCard read it independently via rememberTranslucency() for
    // screens rendered without MainShell (standalone activities).
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    // Glass NavigationBar: panel at 72% alpha when translucent, solid when off.
    val navBarColor = glassContainerColor(c.panel, translucent)

    Scaffold(
        containerColor = c.bg,
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
                containerColor = navBarColor,
            ) {
                NavTab.entries.forEachIndexed { index, tab ->
                    val label = stringResource(tab.labelRes)
                    NavigationBarItem(
                        selected = selectedTab == index,
                        onClick = {
                            val leavingSettings =
                                NavTab.entries[selectedTab] == NavTab.SETTINGS && index != selectedTab
                            val guard = settingsNavGuard
                            if (leavingSettings && guard != null) {
                                // Intercept: the guard shows the Discard dialog and
                                // only runs `proceed` if the user confirms (or there
                                // are no unsaved changes).
                                guard { selectedTab = index }
                            } else {
                                selectedTab = index
                            }
                        },
                        // §5/§9: 20dp Outlined nav glyph.
                        // CopyPaste-n7ff: contentDescription = null — the visible label
                        // below already names the tab; describing the icon too makes
                        // TalkBack announce the name twice.
                        icon = { Icon(tab.icon, contentDescription = null, modifier = Modifier.size(20.dp)) },
                        label = { Text(label) },
                        // §9 spec: active = accent, inactive = uniform dim, indicator = accent/15.
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor       = c.accent,
                            selectedTextColor       = c.accent,
                            indicatorColor          = c.accent.copy(alpha = 0.15f),
                            unselectedIconColor     = c.dim,
                            unselectedTextColor     = c.dim,
                        ),
                    )
                }
            }
        }
    ) { innerPadding ->
        // Stack the active screen above a slim sync-status strip. The strip
        // hosts the online-devices badge (Android parity for the macOS sidebar
        // SyncStatusChip): app label on the left, coloured dot + count on the
        // right. innerPadding is applied to the Column so the screen content and
        // the strip both clear the bottom NavigationBar inset.
        Column(modifier = Modifier.padding(innerPadding)) {
            Box(modifier = Modifier.weight(1f)) {
                when (NavTab.entries[selectedTab]) {
                    NavTab.CLIPS -> HistoryScreen(
                        viewModel = viewModel,
                        showBackButton = false,
                        onBack = {}
                    )
                    NavTab.DEVICES -> DevicesScreen(
                        showBackButton = false,
                        onBack = {}
                    )
                    NavTab.SETTINGS -> SettingsScreen(
                        showBackButton = false,
                        onBack = {},
                        onRegisterNavGuard = { guard -> settingsNavGuard = guard },
                    )
                }
            }
            SyncStatusBadge()
        }
    }
}
