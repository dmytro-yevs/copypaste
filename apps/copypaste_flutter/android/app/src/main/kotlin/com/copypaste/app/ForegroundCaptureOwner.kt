package com.copypaste.app

/** Process-wide foreground host ownership, independent of Activity lifetime. */
internal class ForegroundCaptureOwner<H : Any>(
    private val open: () -> H?,
    private val close: (H, (Boolean) -> Unit) -> Unit,
) {
    private val lock = Any()
    private val active = mutableSetOf<H>()
    private val retiring = mutableSetOf<H>()
    private val stops = mutableListOf<(Boolean) -> Unit>()
    private var generation = 0L
    private var opening = 0
    private var revoking = 0
    private var completing = 0
    private var failed = false

    fun acquire(previous: H?): H? {
        val admittedGeneration = synchronized(lock) {
            if (stops.isNotEmpty() || completing != 0 || failed) return null
            if (previous != null && active.contains(previous)) return previous
            opening += 1
            generation
        }
        // Native open can invoke cancellation. No owner lock crosses it.
        val host = runCatching(open).getOrNull()
        val accepted = synchronized(lock) {
            opening -= 1
            if (host != null && generation == admittedGeneration && stops.isEmpty() && completing == 0 && !failed) {
                active.add(host)
                true
            } else {
                if (host != null) { retiring.add(host); revoking += 1 }
                false
            }
        }
        if (host != null && !accepted) revoke(host) else settle()
        return host.takeIf { accepted }
    }

    fun retire(host: H?) {
        if (host == null) return
        synchronized(lock) {
            active.remove(host)
            retiring.add(host)
            revoking += 1
        }
        revoke(host)
    }

    fun disable(completion: (Boolean) -> Unit) {
        val hosts = synchronized(lock) {
            stops.add(completion)
            if (generation == Long.MAX_VALUE) failed = true else generation += 1
            retiring.addAll(active)
            active.clear()
            retiring.toList().also { revoking += it.size }
        }
        hosts.forEach(::revoke)
        settle()
    }

    private fun revoke(host: H) {
        try {
            close(host) { drained ->
                synchronized(lock) {
                    if (drained) retiring.remove(host) else failed = true
                }
                settle()
            }
        } catch (_: RuntimeException) {
            synchronized(lock) { failed = true }
        } finally {
            synchronized(lock) { revoking -= 1 }
            settle()
        }
    }

    private fun settle() {
        val completed = synchronized(lock) {
            if (stops.isEmpty() || completing != 0 || opening != 0 || revoking != 0 || (!failed && retiring.isNotEmpty())) return
            val result = !failed
            stops.toList().also { completing += it.size; stops.clear() }.map { it to result }
        }
        completed.forEach { (completion, result) ->
            runCatching { completion(result) }
            synchronized(lock) { completing -= 1 }
        }
        settle()
    }
}
