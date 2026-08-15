package com.copypaste.app

import java.util.ArrayDeque

/**
 * The hand-off between whichever Android component captured a clip and the Rust
 * side that stores it.
 *
 * A process-wide singleton rather than plugin state, because the components
 * that capture — a share target, a text-selection action, a Quick Settings tile
 * — can run before the WebView and the Rust library exist. Rust drains this
 * about once a second (`capture::intake`), and anything sitting here when the
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

    private val queue = ArrayDeque<CapturedClip>(CAPACITY)
    private var dropped = 0L
    private var privateMode = false

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

    @Synchronized
    fun offer(
        text: String,
        source: CaptureSource,
        sourceAppBundleId: String? = null,
        sourceAppName: String? = null,
    ) {
        if (privateMode || text.isBlank()) return
        queue.addLast(
            CapturedClip(text, source, System.currentTimeMillis(), sourceAppBundleId, sourceAppName),
        )
        while (queue.size > CAPACITY) {
            queue.removeFirst()
            dropped++
        }
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
    }

    /** Everything captured since the last call, oldest first. */
    @Synchronized
    fun drain(): Pair<List<CapturedClip>, Long> {
        val taken = queue.toList()
        val lost = dropped
        queue.clear()
        dropped = 0
        return taken to lost
    }
}
