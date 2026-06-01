package com.copypaste.android

import android.accessibilityservice.AccessibilityService
import android.content.ClipboardManager
import android.content.Context
import android.view.accessibility.AccessibilityEvent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Minimal AccessibilityService that retains background clipboard access on Android 10+.
 *
 * Android 10+ (API 29+) blocks [ClipboardManager.getPrimaryClip] from any app that is not
 * the current foreground app, the default IME, or an enabled AccessibilityService.
 *
 * This service uses the third exception. It declares no window-content retrieval
 * ([canRetrieveWindowContent]="false") and registers no event types that would let it
 * read UI content — it solely uses the binding to retain clipboard access in background.
 *
 * The user must enable it in Settings > Accessibility. [OnboardingActivity] guides them
 * there with [android.provider.Settings.ACTION_ACCESSIBILITY_SETTINGS].
 *
 * On Android 9 and below, [ClipboardService] and [MainActivity]'s listener already cover
 * background access, so this service is a no-op on older APIs (still registered, just
 * never fires meaningful events).
 */
class ClipboardAccessibilityService : AccessibilityService() {

    private val scope = CoroutineScope(Dispatchers.IO)
    private lateinit var settings: Settings
    private lateinit var repository: ClipboardRepository
    private lateinit var syncManager: SyncManager
    private lateinit var clipboardManager: ClipboardManager

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener

        // Image branch: check all MIME types before falling through to text.
        val imageMime = (0 until clip.description.mimeTypeCount)
            .map { clip.description.getMimeType(it) }
            .firstOrNull { it.startsWith("image/") }
        if (imageMime != null) {
            val uri = clip.getItemAt(0)?.uri
            if (uri != null) {
                scope.launch { ClipboardService.captureImageClip(this@ClipboardAccessibilityService, uri, imageMime, settings, repository, syncManager) }
            }
            return@OnPrimaryClipChangedListener
        }

        val text = clip.getItemAt(0)?.text?.toString() ?: return@OnPrimaryClipChangedListener
        if (text.isBlank()) return@OnPrimaryClipChangedListener
        scope.launch { handleClip(text) }
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        AppLogger.i(TAG, "ClipboardAccessibilityService.onServiceConnected called")
        try {
            settings = Settings(this)
            repository = ClipboardRepository(this)
            val relayClient = RelayClient(settings.relayUrl)
            syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
        } catch (e: Exception) {
            // Sync/relay initialisation failed (bad URL, missing .so, etc.).
            // Log the failure so it shows up in the in-app log viewer and adb-pullable log,
            // then fall through and attempt to register the clipboard listener anyway so that
            // local capture still works even without sync.
            AppLogger.e(TAG, "onServiceConnected: sync init failed — local-capture-only mode", e)
        }

        // Always attempt to register the clipboard listener. If clipboardManager cannot be
        // obtained (e.g. the service context is broken) we at least log the failure rather
        // than silently dying.
        try {
            clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboardManager.addPrimaryClipChangedListener(clipListener)
            AppLogger.i(TAG, "ClipboardAccessibilityService connected — background clipboard access active")
        } catch (e: Exception) {
            AppLogger.e(TAG, "onServiceConnected: failed to register clipboard listener", e)
        }
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // No events subscribed — this block is intentionally empty.
        // The service exists solely to hold the accessibility binding.
    }

    override fun onInterrupt() {
        AppLogger.w(TAG, "ClipboardAccessibilityService interrupted")
    }

    override fun onDestroy() {
        AppLogger.i(TAG, "ClipboardAccessibilityService.onDestroy called")
        runCatching { clipboardManager.removePrimaryClipChangedListener(clipListener) }
        scope.cancel()
        super.onDestroy()
    }

    private suspend fun handleClip(text: String) {
        // HIGH-2: route through the same store + count + sync pipeline as the
        // foreground service so background-captured clips are synced and counted,
        // not just stored locally.
        ClipboardService.captureClip(this, text, settings, repository, syncManager)
        AppLogger.d(TAG, "AccessibilityService captured background clip")
    }

    companion object {
        private const val TAG = "ClipboardA11yService"

        /**
         * Returns true if this service is currently enabled in Accessibility Settings.
         * Use to decide whether to show the onboarding prompt.
         */
        fun isEnabled(context: Context): Boolean {
            val enabledServices = android.provider.Settings.Secure.getString(
                context.contentResolver,
                android.provider.Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
            ) ?: return false
            val componentName = "${context.packageName}/${ClipboardAccessibilityService::class.java.name}"
            return enabledServices.split(":").any { it.equals(componentName, ignoreCase = true) }
        }
    }
}
