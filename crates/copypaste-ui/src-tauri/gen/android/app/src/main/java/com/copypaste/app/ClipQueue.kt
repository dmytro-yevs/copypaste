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

    private val queue = ArrayDeque<Clip>(CAPACITY)
    private var dropped = 0L

    /** Set by [CapturePlugin.load]; false means nothing is draining this yet. */
    @Volatile
    var rustIsUp = false

    data class Clip(val text: String, val source: String, val atMs: Long)

    @Synchronized
    fun offer(text: String, source: String) {
        if (text.isBlank()) return
        queue.addLast(Clip(text, source, System.currentTimeMillis()))
        while (queue.size > CAPACITY) {
            queue.removeFirst()
            dropped++
        }
    }

    /** Everything captured since the last call, oldest first. */
    @Synchronized
    fun drain(): Pair<List<Clip>, Long> {
        val taken = queue.toList()
        val lost = dropped
        queue.clear()
        dropped = 0
        return taken to lost
    }
}
