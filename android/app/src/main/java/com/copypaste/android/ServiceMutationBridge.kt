package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.launch

/**
 * CopyPaste-vp63.32: outbound mutation-queue drain hook — extracted VERBATIM
 * from [ClipboardService]'s companion object.
 *
 * [ClipboardViewModel.onMutationSync] calls [requestMutationQueueDrain] so pin/
 * unpin/reorder/delete/clear propagate over relay + Supabase. The active
 * service instance registers its scope + resources here at `onCreate` via
 * [setHook]; the hook is cleared at `onDestroy` via [clearHook]. Process-wide
 * @Volatile so the hook is visible to the main-thread ViewModel coroutines.
 */
object ServiceMutationBridge {
    private const val TAG = "ClipboardService"

    /**
     * Optional hook: set to the active service's drain function at `onCreate`,
     * cleared at `onDestroy`. Callers invoke via [requestMutationQueueDrain].
     * Null when the FGS is not running (ViewModel falls back to a no-op).
     */
    @Volatile
    private var mutationDrainHook: (suspend () -> Unit)? = null

    /** Register the active service's drain hook. Called from [ClipboardService.onCreate]. */
    fun setHook(hook: suspend () -> Unit) {
        mutationDrainHook = hook
    }

    /** Clear the drain hook. Called from [ClipboardService.onDestroy]. */
    fun clearHook() {
        mutationDrainHook = null
    }

    /**
     * Request that the active [ClipboardService] drain [OutboundMutationQueue].
     *
     * Called by [ClipboardViewModel.onMutationSync] after every UI mutation.
     * Safe to call from any thread. No-op when the FGS is not running (the
     * queue remains durable and will be drained on next service start).
     *
     * The actual drain ([SyncManager.drainOutboundMutationQueue]) runs on the
     * service's IO scope so it cannot block the ViewModel coroutine.
     */
    fun requestMutationQueueDrain() {
        // Cannot call a suspend fun directly here; the hook is a no-arg lambda that
        // launches the drain on the service's existing IO scope internally.
        val hook = mutationDrainHook ?: run {
            Log.d(TAG, "requestMutationQueueDrain: FGS not running — queue persisted for later")
            return
        }
        // The hook itself launches on a dedicated IO scope (non-blocking fire-and-forget).
        // Using a daemon CoroutineScope (not GlobalScope) so the drain does not keep the
        // process alive past the service lifecycle. The drain is idempotent and bounded.
        @Suppress("OPT_IN_USAGE") // explicit opt-in: bounded work, not a long-running coroutine
        MainScope().launch(Dispatchers.IO) {
            try {
                hook()
            } catch (e: Exception) {
                Log.w(TAG, "requestMutationQueueDrain: hook failed: ${e.message}")
            }
        }
    }
}
