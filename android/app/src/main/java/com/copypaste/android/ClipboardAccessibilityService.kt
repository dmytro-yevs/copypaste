package com.copypaste.android

import android.accessibilityservice.AccessibilityService
import android.content.ClipboardManager
import android.content.Context
import android.view.accessibility.AccessibilityEvent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
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

    // SupervisorJob: one failing child coroutine does not cancel sibling capture
    // coroutines — all clipboard capture paths remain active after any one failure.
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    private lateinit var settings: Settings
    private lateinit var repository: ClipboardRepository
    // Nullable: may remain null when sync-init fails; handleClip skips sync safely.
    private var syncManager: SyncManager? = null
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
                // syncManager may be null if sync init failed; captureImageClip
                // takes non-null, so supply a fallback instance (image sync is not
                // wired anyway — the parameter is @Suppress UNUSED_PARAMETER there).
                val sm = syncManager ?: try {
                    SyncManager(RelayClient(""), settings.deviceId, token = "", settings = settings)
                } catch (_: Exception) { return@OnPrimaryClipChangedListener }
                scope.launch { ClipboardService.captureImageClip(this@ClipboardAccessibilityService, uri, imageMime, settings, repository, sm) }
            }
            return@OnPrimaryClipChangedListener
        }

        // File branch: non-text, non-image URI → real file (PDF, ZIP, etc.).
        val itemUri = clip.getItemAt(0)?.uri
        if (itemUri != null) {
            val mimeTypes = (0 until clip.description.mimeTypeCount)
                .map { clip.description.getMimeType(it) }
            val fileMime = mimeTypes.firstOrNull { mime ->
                mime != null && !mime.startsWith("text/") && !mime.startsWith("image/")
            }
            if (fileMime != null) {
                scope.launch {
                    ClipboardService.captureFileClip(
                        this@ClipboardAccessibilityService,
                        itemUri,
                        fileMime,
                        settings,
                        repository,
                    )
                }
                return@OnPrimaryClipChangedListener
            }
        }

        val text = clip.getItemAt(0)?.text?.toString() ?: return@OnPrimaryClipChangedListener
        if (text.isBlank()) return@OnPrimaryClipChangedListener
        scope.launch { handleClip(text) }
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        AppLogger.i(TAG, "ClipboardAccessibilityService.onServiceConnected called")

        // Initialize settings and repository FIRST — these must succeed for the
        // clipboard listener to function at all. They only need the application
        // context, so they are safe to construct here unconditionally.
        settings = Settings(this)
        repository = ClipboardRepository(this)

        // Sync init is best-effort: a bad relay URL, missing .so, or any other
        // transient failure must not prevent local clipboard capture. When this
        // block throws, syncManager remains null and handleClip skips sync while
        // continuing to store clips locally via captureClip's fallback path.
        try {
            val relayClient = RelayClient(settings.relayUrl)
            syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
        } catch (e: Exception) {
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
        //
        // syncManager may be null when sync init failed in onServiceConnected.
        // In that case, captureClip is still called with a no-op SyncManager so
        // that local capture (store + count + notification) works correctly.
        // The SyncManager.syncEnabled path inside captureClip will be skipped
        // because settings.syncEnabled is read fresh each time.
        val sm = syncManager ?: run {
            // Sync init failed — construct a minimal SyncManager for the call.
            // It will not be used because captureClip checks settings.syncEnabled,
            // and without a valid relay URL sync would be disabled anyway.
            try {
                val relayClient = RelayClient("")
                SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
            } catch (e: Exception) {
                AppLogger.w(TAG, "handleClip: cannot construct fallback SyncManager — sync will be skipped", e)
                return  // extremely unlikely; local capture will miss this clip
            }
        }
        ClipboardService.captureClip(this, text, settings, repository, sm)
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
