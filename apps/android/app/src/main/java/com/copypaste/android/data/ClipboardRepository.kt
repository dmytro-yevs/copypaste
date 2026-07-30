package com.copypaste.android.data

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.PersistableBundle
import androidx.core.content.getSystemService
import com.copypaste.android.keystore.DeviceSecret
import com.copypaste.ffi.ClipItem
import com.copypaste.ffi.CopyPaste
import com.copypaste.ffi.NewPairing
import com.copypaste.ffi.PairedDevice
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Everything the UI is allowed to touch, on the right thread.
 *
 * The FFI's history calls are blocking — they are SQLite plus an AEAD — and
 * manifest 06 INV-37 is explicit that blocking work must never run on the UI
 * thread. That is enforced here rather than remembered at each call site: every
 * function below is `suspend` and every one of them hops to [io] first, so
 * there is no way for a ViewModel to reach the store from the main dispatcher.
 *
 * The peer calls are already `suspend` on the UniFFI side (they are `async fn`s
 * in Rust driven on a tokio reactor), so they are passed straight through.
 */
class ClipboardRepository(
    private val core: CopyPaste,
    private val clipboard: ClipboardManager?,
    private val io: CoroutineDispatcher = Dispatchers.IO,
) {

    val deviceName: String get() = core.deviceName()

    // ------------------------------------------------------------- history

    suspend fun page(limit: Int, offset: Int): List<ClipItem> = withContext(io) {
        core.list(limit.toUInt(), offset.toUInt())
    }

    suspend fun search(query: String, limit: Int): List<ClipItem> = withContext(io) {
        core.search(query, limit.toUInt())
    }

    suspend fun add(text: String): ClipItem = withContext(io) {
        core.add(text, CONTENT_TYPE_TEXT)
    }

    suspend fun delete(id: String): Boolean = withContext(io) { core.delete(id) }

    suspend fun setPinned(id: String, pinned: Boolean): Boolean =
        withContext(io) { core.setPinned(id, pinned) }

    /**
     * The full plaintext of one item.
     *
     * The only route to a sensitive item's content, and a deliberate one — the
     * list never carries it. Callers must treat the result as short-lived.
     */
    suspend fun itemText(id: String): String = withContext(io) { core.itemText(id) }

    // ----------------------------------------------------- system clipboard

    /**
     * Put an item on the system clipboard.
     *
     * Marks the clip sensitive when the item is, which on Android 13+ stops the
     * system's clipboard-preview overlay from rendering the value in a toast
     * the user did not ask for — the same rule as the in-app list, applied to
     * the one surface this app does not draw.
     */
    suspend fun copyToClipboard(item: ClipItem) {
        val text = itemText(item.id)
        val clip = ClipData.newPlainText(CLIP_LABEL, text)
        if (item.isSensitive && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            clip.description.extras = PersistableBundle().apply {
                putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true)
            }
        }
        clipboard?.setPrimaryClip(clip)
    }

    /**
     * Read the system clipboard and store what is there.
     *
     * **Only ever correct to call while this app has window focus.** Since
     * Android 10 (API 29) `getPrimaryClip` returns `null` to an app that is not
     * the focused app or the active IME, and no permission changes that. There
     * is no background capture here because the platform does not have one —
     * see `apps/android/README.md`.
     *
     * Returns the stored item, or `null` when there was nothing to store: an
     * empty clipboard, a non-text clip, or a duplicate the core collapsed.
     */
    suspend fun captureFromClipboard(): ClipItem? {
        val text = clipboard
            ?.primaryClip
            ?.takeIf { it.itemCount > 0 }
            ?.getItemAt(0)
            ?.coerceToText(null)
            ?.toString()
            ?: return null
        return addIgnoringEmpty(text)
    }

    /**
     * Store text that arrived from the share sheet or the text-selection menu.
     *
     * `EmptyContent` is not an error the user needs to see — an empty share is
     * a normal thing to do by accident — so it becomes `null`.
     */
    suspend fun addIgnoringEmpty(text: String): ClipItem? = try {
        add(text)
    } catch (_: com.copypaste.ffi.CopyPasteException.EmptyContent) {
        null
    }

    // --------------------------------------------------------------- peers

    /**
     * Mint a pairing.
     *
     * The returned [NewPairing] carries a live credential. Callers must
     * destructure it immediately, keep only `pairingId` in state, and let
     * `code` reach nothing but the composable that draws it — UniFFI generates
     * `NewPairing` as a Kotlin `data class`, so its `toString()` prints the
     * code and any `Log.d(TAG, "$pairing")` would put it in logcat.
     */
    suspend fun createPairing(name: String): NewPairing =
        withContext(io) { core.createPairing(name) }

    suspend fun acceptPairing(code: String, address: String, name: String): PairedDevice =
        core.acceptPairing(code, address, name)

    suspend fun listPeers(): List<PairedDevice> = withContext(io) { core.listPeers() }

    suspend fun unpair(pairingId: String): Boolean = withContext(io) { core.unpair(pairingId) }

    suspend fun checkPeer(pairingId: String): PairedDevice = core.checkPeer(pairingId)

    suspend fun syncPeer(pairingId: String) = core.syncPeer(pairingId)

    companion object {
        private const val CONTENT_TYPE_TEXT = "text"

        /**
         * What the system's clipboard UI calls the clip. Never the content:
         * on some launchers the label is shown in the paste preview.
         */
        private const val CLIP_LABEL = "CopyPaste"

        /**
         * Open the core and wrap it.
         *
         * Call once per process. Opening compiles the ~100-rule secret-detection
         * table, which is the expensive part; the handle is meant to outlive
         * every screen.
         */
        fun open(context: Context): ClipboardRepository {
            val app = context.applicationContext
            // `filesDir` is scoped storage: no permission, no other app can read
            // it, removed with the app. Nothing in CopyPaste ever writes outside
            // it, which is why there is no storage permission in the manifest.
            val secret = DeviceSecret.loadOrCreate(app.filesDir)
            val core = try {
                CopyPaste.open(
                    dataDir = app.filesDir.absolutePath,
                    deviceSecret = secret,
                    deviceName = Build.MODEL ?: "Android device",
                )
            } finally {
                // The core has copied what it needs into its own zeroizing
                // types. Nothing keeps this array alive afterwards.
                secret.fill(0)
            }
            return ClipboardRepository(core, app.getSystemService<ClipboardManager>())
        }
    }
}
