package com.copypaste.app

import android.content.Context
import android.util.AtomicFile
import android.os.SystemClock
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

/** Immutable URI payloads: retain the current publication and a one-day grace period. */
internal object AndroidClipboardCache {
    private val lock = Any()
    private val cleaning = AtomicBoolean(false)
    private val lastCleanup = AtomicLong(-1)
    private const val maxAgeMs = 24 * 60 * 60 * 1000L
    private const val unknownPublication = "*"

    /** Persist both possible owners before publishing, so a crash cannot unprotect either URI. */
    fun publish(context: Context, file: File?, action: () -> Unit) = synchronized(lock) {
        val root = File(context.cacheDir, "clipboard").apply { mkdirs() }
        val state = AtomicFile(File(root, ".active-publication"))
        val previous = read(state) ?: setOf(unknownPublication)
        val next = file?.parentFile?.name?.let { setOf(it) } ?: emptySet()
        if (previous.isEmpty() && next.isEmpty()) {
            action()
            return@synchronized
        }
        // Binary publication depends on durable cache bytes already. If its
        // ownership cannot be persisted, leave the previous clipboard intact.
        val staged = runCatching { write(state, previous + next) }.isSuccess
        if (!staged && file != null) error("Clipboard cache ownership could not be stored")
        try {
            action()
        } catch (error: RuntimeException) {
            if (staged) runCatching { write(state, previous) }
            throw error
        }
        if (staged) runCatching { write(state, next) }
    }

    fun scheduleCleanup(context: Context) {
        val previous = lastCleanup.get()
        if (previous >= 0 && SystemClock.elapsedRealtime() - previous < 60 * 60 * 1000L) return
        if (!cleaning.compareAndSet(false, true)) return
        // Unknown legacy ownership and filesystem failures must not turn
        // repeated clipboard writes into repeated maintenance threads either.
        lastCleanup.set(SystemClock.elapsedRealtime())
        val application = context.applicationContext
        try {
            Thread({
                try {
                    cleanup(application, System.currentTimeMillis())
                }
                catch (_: Exception) { /* Maintenance must never terminate the application. */ }
                finally { cleaning.set(false) }
            }, "clipboard-cache-cleanup").start()
        } catch (_: RuntimeException) { cleaning.set(false) }
    }

    internal fun cleanup(context: Context, nowMs: Long): Boolean {
        val root = File(context.cacheDir, "clipboard")
        // On the first upgraded launch the legacy clipboard owner is unknown.
        // A successful native publication establishes it without reading the
        // system clipboard in the background or prompting the user.
        val protected = synchronized(lock) { read(AtomicFile(File(root, ".active-publication"))) }
            ?: setOf(unknownPublication)
        val cutoff = nowMs - maxAgeMs
        val canonicalRoot = root.canonicalFile
        root.listFiles()?.forEach { entry ->
            if (entry.name in protected || entry.lastModified() >= cutoff) return@forEach
            // PlatformFileGateway owns these flat share/export files. Their
            // existing one-day grace also applies when no new share occurs.
            if (entry.isFile && entry.name.startsWith("copypaste-")) {
                runCatching { entry.delete() }
                return@forEach
            }
            // Only this writer's UUID directories belong to this cleanup.
            // Export/share files under the same root have a separate owner.
            if (unknownPublication !in protected && entry.isDirectory && uuid.matches(entry.name) &&
                entry.canonicalFile == File(canonicalRoot, entry.name)) {
                runCatching {
                    // These directories contain immutable leaf payloads only.
                    // Never walk a nested directory or follow a directory link.
                    val files = entry.listFiles() ?: return@runCatching
                    if (files.none { it.isDirectory }) {
                        files.forEach { it.delete() }
                        entry.delete()
                    }
                }
            }
        }
        return unknownPublication !in protected
    }

    private val uuid = Regex("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
    private fun read(state: AtomicFile): Set<String>? = runCatching {
        state.openRead().use { stream ->
            if (stream.channel.size() > 64 * 1024) null
            else stream.bufferedReader().readLines().filter(String::isNotBlank).toSet()
                .takeIf { names -> names.all { it == unknownPublication || uuid.matches(it) } }
        }
    }.getOrNull()

    private fun write(state: AtomicFile, names: Set<String>) {
        val stream = state.startWrite()
        try {
            stream.write(names.joinToString("\n").toByteArray(Charsets.UTF_8))
            state.finishWrite(stream)
        } catch (error: Exception) {
            state.failWrite(stream)
            throw error
        }
    }
}
