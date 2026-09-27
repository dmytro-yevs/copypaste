package com.copypaste.app

import java.util.ArrayDeque
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject

/**
 * The hand-off between whichever Android component captured a clip and the Rust
 * side that stores it.
 *
 * A process-wide singleton rather than plugin state, because the components
 * that capture — a share target, a text-selection action, a Quick Settings tile
 * — can run before the WebView and the Rust library exist. A bounded native
 * callback wakes Rust to drain this, and anything sitting here when the
 * process dies is lost, which is why [dropped] is reported rather than silently
 * absorbed.
 *
 * Deliberately in memory and never on disk: a spool file would be clipboard
 * plaintext at rest outside SQLCipher, which is a worse trade than a bounded
 * loss window.
 */
object ClipQueue {
    /** Beyond this, the oldest go. Matches `MAX_BUFFERED` on the Rust side. */
    private const val CAPACITY = 128

    const val MAX_TEXT_BYTES = 4 * 1024 * 1024
    const val MAX_BINARY_BYTES = 4 * 1024 * 1024

    private val queue = ArrayDeque<CapturedClip>(CAPACITY)
    private var dropped = 0L
    private var privateMode = false
    private var queueReady: (() -> Unit)? = null
    private var wakePending = false
    private var stateDirty = false

    /**
     * Set by [CapturePlugin.load] and cleared when its activity is destroyed;
     * false means nothing is draining this.
     *
     * It must be cleared: the foreground service can outlive the WebView, and a
     * process that keeps this true with no drain task turns every share into a
     * clip that waits in memory until the process dies.
     */
    @Volatile
    var rustIsUp = false

    fun offer(
        text: String,
        source: CaptureSource,
        sourceAppBundleId: String? = null,
        sourceAppName: String? = null,
    ) {
        val wake = synchronized(this) {
            if (privateMode || text.isBlank()) return
            if (text.toByteArray(Charsets.UTF_8).size > MAX_TEXT_BYTES) {
                dropped++
                return@synchronized nextWakeLocked()
            }
            queue.addLast(
                CapturedClip(
                    text, null, null, null, source, System.currentTimeMillis(), sourceAppBundleId, sourceAppName,
                ),
            )
            while (queue.size > CAPACITY) {
                queue.removeFirst()
                dropped++
            }
            nextWakeLocked()
        }
        wake?.invoke()
    }

    fun offerBinary(
        bytesBase64: String,
        contentType: String,
        filename: String?,
        source: CaptureSource,
        sourceAppBundleId: String? = null,
        sourceAppName: String? = null,
    ) {
        val wake = synchronized(this) {
            if (privateMode || bytesBase64.isBlank()) return
            // Base64 is bridge transport only.  The cap names raw bytes and is
            // enforced before this queue owns the encoded representation.
            if (bytesBase64.length > ((MAX_BINARY_BYTES + 2) / 3) * 4) {
                dropped++
                return@synchronized nextWakeLocked()
            }
            queue.addLast(
                CapturedClip(
                    null, bytesBase64, contentType, filename, source,
                    System.currentTimeMillis(), sourceAppBundleId, sourceAppName,
                ),
            )
            while (queue.size > CAPACITY) {
                queue.removeFirst()
                dropped++
            }
            nextWakeLocked()
        }
        wake?.invoke()
    }

    @Synchronized
    fun acceptsCapture(source: CaptureSource, sourcePackage: String?): Boolean {
        if (privateMode) return false
        return source != CaptureSource.BACKGROUND ||
            CaptureExclusions.decide(sourcePackage) == ExternalReadDecision.READ
    }

    @Synchronized
    fun setPrivateMode(enabled: Boolean) {
        if (!enabled) {
            privateMode = false
            return
        }

        privateMode = true
        // Counted, not zeroed, exactly as `intake::Buffer::discard_all` counts
        // the clips Rust discards for the same reason. Clearing the tally would
        // erase drops that happened before private mode and were never
        // reported, so history would have a hole nobody was told about.
        dropped += queue.size
        queue.clear()
        wakePending = false
    }

    fun markCaptureStateDirty() {
        val wake = synchronized(this) {
            stateDirty = true
            nextWakeLocked()
        }
        wake?.invoke()
    }

    /** Everything captured since the last call, oldest first. */
    @Synchronized
    fun drain(): Triple<List<CapturedClip>, Long, Boolean> {
        val taken = queue.toList()
        val lost = dropped
        val changed = stateDirty
        queue.clear()
        dropped = 0
        wakePending = false
        stateDirty = false
        return Triple(taken, lost, changed)
    }

    fun subscribeQueueReady(channel: Channel?) {
        setQueueReady(channel?.let { ready -> { ready.send(JSObject()) } })
    }

    internal fun subscribeQueueReadyForTest(callback: (() -> Unit)?) {
        setQueueReady(callback)
    }

    private fun setQueueReady(callback: (() -> Unit)?) {
        val wake = synchronized(this) {
            queueReady = callback
            if (callback == null) return@synchronized null
            if (queue.isNotEmpty() || dropped > 0 || stateDirty) {
                nextWakeLocked()
            } else {
                null
            }
        }
        wake?.invoke()
    }

    private fun nextWakeLocked(): (() -> Unit)? =
        if (!wakePending) queueReady?.also { wakePending = true } else null
}
