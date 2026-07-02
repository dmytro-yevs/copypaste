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
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import com.copypaste.android.ui.shell.MainShell
import com.copypaste.android.ui.theme.CommittedCopyPasteTheme
import com.copypaste.android.ui.theme.SecureWindowChrome
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
        // CopyPaste-1g00: screenshot protection is now pref-driven (Settings.allowScreenshots).
        // SecureWindowChrome applies FLAG_SECURE centrally when allowScreenshots=false (the default).
        // The old hardcoded setFlags(FLAG_SECURE) is removed so the user's pref is honoured.
        applyScreenshotPolicy(Settings(this))
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
            // CopyPaste-crh3.102: the relay cloud-upload path is re-enabled. The
            // server-issued bearer token is persisted into Settings.relayToken at
            // registration time (SyncManager.ensureRelayToken / RelaySubscriptionClient)
            // and the producer (pushToRelay → ensureRelayToken) reads it from there,
            // self-registering on a miss and re-registering once on a 401. We seed the
            // SyncManager with the last-cached token here for parity; an empty/stale
            // value is transparently refreshed by ensureRelayToken before the first push.
            syncManager = SyncManager(relayClient, settings.deviceId, token = settings.relayToken, settings = settings)
        } catch (e: Exception) {
            Log.w(TAG, "SyncManager init failed — proceeding without relay sync: ${e.javaClass.simpleName} ${e.message}")
            val fallback = RelayClient("")
            syncManager = SyncManager(fallback, settings.deviceId, token = settings.relayToken, settings = settings)
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
            SecureWindowChrome {
                // android-appearance D5: committed-appearance root — wraps ALL
                // three tabs (Clips/Devices/Settings) so a Save from the embedded
                // Settings tab re-themes the whole shell without recreate().
                CommittedCopyPasteTheme {
                    MainShell(viewModel = viewModel)
                }
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
