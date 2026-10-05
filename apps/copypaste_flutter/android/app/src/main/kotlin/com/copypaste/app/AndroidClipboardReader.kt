package com.copypaste.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Process
import android.os.Build
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.OutputStream
import java.util.concurrent.Executors

internal object AndroidClipboardReader {
    private const val maximumBinaryBytes = 16 * 1024 * 1024
    private const val maximumDecodedImageBytes = 50 * 1024 * 1024L
    private val mimeType = Regex("^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$")
    private val ingestion = java.util.concurrent.ThreadPoolExecutor(1, 1, 0L, java.util.concurrent.TimeUnit.MILLISECONDS, java.util.concurrent.ArrayBlockingQueue(32))

    private val main = android.os.Handler(android.os.Looper.getMainLooper())
    private val lifecycle = Executors.newCachedThreadPool()
    private val streamCleanup = Executors.newCachedThreadPool()

    internal class Host(val id: Long) {
        val closed = java.util.concurrent.atomic.AtomicBoolean(false)
        val pending = java.util.concurrent.ConcurrentHashMap.newKeySet<Pending>()
        private val completions = mutableListOf<(Boolean) -> Unit>()
        private var drained: Boolean? = null
        fun close(completion: (Boolean) -> Unit = {}) {
            synchronized(completions) {
                val result = drained
                if (result != null) { main.post { completion(result) }; return }
                completions.add(completion)
            }
            if (!closed.compareAndSet(false, true)) return
            // Revoke the native capability before returning to the lifecycle caller.
            val revoked = runCatching { NativeRuntimeCapture.revokeHost(id) }.getOrDefault(false)
            pending.toList().forEach(Pending::cancel)
            lifecycle.execute {
                val result = revoked && runCatching { NativeRuntimeCapture.drainHost(id) }.getOrDefault(false)
                val callbacks = synchronized(completions) {
                    drained = result
                    completions.toList().also { completions.clear() }
                }
                main.post { callbacks.forEach { it(result) } }
            }
        }
    }

    fun openHost(explicit: Boolean = false): Host? =
        runCatching { NativeRuntimeCapture.openHost(explicit) }.getOrDefault(0L).takeIf { it > 0L }?.let(::Host)

    @Volatile private var backgroundHost: Host? = null
    private val retiringBackgroundHosts = mutableSetOf<Host>()
    @Synchronized fun startBackground(): Host? = openHost()?.also { backgroundHost = it }
    @Synchronized fun stopBackground(completion: (Boolean) -> Unit = {}) {
        backgroundHost?.let { retiringBackgroundHosts.add(it) }
        backgroundHost = null
        val hosts = retiringBackgroundHosts.toList()
        if (hosts.isEmpty()) { completion(true); return }
        val remaining = java.util.concurrent.atomic.AtomicInteger(hosts.size)
        val succeeded = java.util.concurrent.atomic.AtomicBoolean(true)
        hosts.forEach { host ->
            host.close { drained ->
                synchronized(this) { if (drained) retiringBackgroundHosts.remove(host) }
                if (!drained) succeeded.set(false)
                if (remaining.decrementAndGet() == 0) completion(succeeded.get())
            }
        }
    }
    fun captureBackground(context: Context, expectedHost: Long) {
        val host = backgroundHost?.takeIf { it.id == expectedHost && !it.closed.get() } ?: return
        if (AndroidCaptureState.captureEnabled(context) && AndroidCaptureState.privilegedGrants(context)) {
            captureClipboard(context, host, background = true)
        }
    }
    fun captureForeground(context: Context, host: Host?) {
        if (host != null && AndroidCaptureState.foregroundCaptureEnabled(context)) {
            captureClipboard(context, host, background = false)
        }
    }

    internal data class Snapshot(val text: String? = null, val uri: Uri? = null, val type: String? = null, val capturedAt: Long = 0)

    internal class Pending(private val host: Host, completion: ((Boolean) -> Unit)? = null) : Runnable {
        private val capability = java.util.concurrent.atomic.AtomicLong(0)
        val token: Long get() = capability.get()
        private val cancelled = java.util.concurrent.atomic.AtomicBoolean(false)
        private val completing = java.util.concurrent.atomic.AtomicBoolean(false)
        private val result = java.util.concurrent.atomic.AtomicReference(completion)
        private val payload = java.util.concurrent.atomic.AtomicReference<Snapshot?>(null)
        private val stream = java.util.concurrent.atomic.AtomicReference<java.io.InputStream?>(null)
        private val action = java.util.concurrent.atomic.AtomicReference<((Snapshot) -> Boolean)?>(null)
        fun attach(token: Long) {
            capability.set(token)
            if (!reading()) cleanup()
        }
        fun cancel() {
            if (!cancelled.compareAndSet(false, true)) return
            payload.set(null)
            action.set(null)
            ingestion.remove(this)
            stream.getAndSet(null)?.let { input -> streamCleanup.execute { runCatching { input.close() } } }
            if (result.get() != null) finish(false) else cleanup()
        }
        fun enqueue(snapshot: Snapshot, work: (Snapshot) -> Boolean) {
            payload.set(snapshot)
            action.set(work)
            if (!reading()) { cancel(); cleanup(); return }
            try { ingestion.execute(this) } catch (_: RuntimeException) { cancel() }
        }
        fun input(input: java.io.InputStream) {
            stream.set(input)
            if (!reading()) {
                stream.compareAndSet(input, null)
                runCatching { input.close() }
                throw IOException("Capture cancelled")
            }
        }
        fun releaseInput(input: java.io.InputStream) { stream.compareAndSet(input, null) }
        fun reading(): Boolean = !cancelled.get() && !host.closed.get()
        fun cleanup() {
            payload.set(null)
            action.set(null)
            host.pending.remove(this)
            val token = capability.getAndSet(0)
            if (token > 0) runCatching { NativeRuntimeCapture.abandon(token) }
        }
        fun finish(saved: Boolean) {
            if (!completing.compareAndSet(false, true)) return
            main.post {
                val callback = result.getAndSet(null)
                try {
                    var published = false
                    if (saved && reading()) {
                        NativeRuntimeCapture.scoped(token, true, "text/plain") {
                            AndroidCaptureFeedback.onCaptured(applicationContext!!)
                            published = true
                            callback?.invoke(true)
                        }
                    }
                    if (!published) callback?.invoke(false)
                } finally { cleanup(); host.close() }
            }
        }
        private var applicationContext: Context? = null
        fun explicitContext(context: Context) { applicationContext = context.applicationContext }
        override fun run() {
            var saved = false
            try {
                val snapshot = payload.getAndSet(null)
                val work = action.getAndSet(null)
                if (snapshot != null && work != null && reading()) saved = work(snapshot)
            } catch (_: Exception) {
                saved = false
            } finally {
                if (result.get() != null) finish(saved) else cleanup()
            }
        }
    }

    private fun begin(host: Host, completion: ((Boolean) -> Unit)? = null): Pending? {
        if (host.closed.get()) return null
        val pending = Pending(host, completion)
        host.pending.add(pending)
        pending.attach(runCatching { NativeRuntimeCapture.begin(host.id, Runnable(pending::cancel)) }.getOrDefault(0L))
        if (pending.token <= 0L || host.closed.get()) { pending.cancel(); return if (completion != null) pending else null }
        return pending
    }

    private fun boundedText(value: CharSequence?, limit: Long): String? {
        if (value == null || value.length.toLong() > limit) return null
        return value.toString().takeIf { it.toByteArray(Charsets.UTF_8).size.toLong() <= limit }
    }

    @Suppress("DEPRECATION")
    fun captureExplicit(context: Context, intent: Intent, host: Host, completion: (Boolean) -> Unit) {
        val pending = begin(host, completion) ?: return host.close { completion(false) }
        pending.explicitContext(context)
        if (pending.token <= 0L || !pending.reading()) return
        var snapshot: Snapshot? = null
        val read = runCatching { NativeRuntimeCapture.scoped(pending.token, false, "text/plain") { limit ->
            val uri = if (intent.action == Intent.ACTION_SEND) {
                intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                    ?: intent.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri
            } else null
            val text = intent.getCharSequenceExtra(if (intent.action == Intent.ACTION_PROCESS_TEXT) Intent.EXTRA_PROCESS_TEXT else Intent.EXTRA_TEXT)
            snapshot = Snapshot(text = boundedText(text, limit), uri = uri, type = intent.type?.lowercase())
        } }.getOrDefault(false)
        val captured = snapshot
        if (!read || captured == null) { pending.cancel(); return }
        val app = context.applicationContext
        pending.enqueue(captured) { data -> materialize(app, pending, data) }
    }

    private fun captureClipboard(context: Context, host: Host, background: Boolean) {
        val pending = begin(host) ?: return
        val clipboard = context.getSystemService(ClipboardManager::class.java)
        var snapshot: Snapshot? = null
        val read = runCatching { NativeRuntimeCapture.scoped(pending.token, false, "text/plain") { limit ->
            val primary = clipboard?.primaryClip
            if (primary != null && primary.description.label?.toString() != "CopyPaste") {
                snapshot = Snapshot(
                    text = text(primary, limit),
                    uri = primary.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri,
                    type = primary.description.takeIf { it.mimeTypeCount > 0 }?.getMimeType(0)?.lowercase(),
                    capturedAt = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) primary.description.timestamp else System.currentTimeMillis(),
                )
            }
        } }.getOrDefault(false)
        val captured = snapshot
        if (!read || captured == null) { pending.cancel(); return }
        val app = context.applicationContext
        pending.enqueue(captured) { data ->
            val saved = materialize(app, pending, data)
            if (saved) {
                NativeRuntimeCapture.scoped(pending.token, true, "text/plain") {
                    if (background) AndroidCaptureState.recordBackgroundCapture(app, data.capturedAt)
                    AndroidCaptureFeedback.onCaptured(app)
                }
            }
            saved
        }
    }

    private fun materialize(context: Context, pending: Pending, snapshot: Snapshot): Boolean {
        // A binary payload never falls back to textual/base64 capture.
        return if (snapshot.uri != null) binary(context, pending, snapshot.uri, snapshot.type ?: return false)
        else snapshot.text?.takeIf(String::isNotBlank)?.let { NativeRuntimeCapture.ingestText(pending.token, it) } == true
    }

    private fun text(clip: ClipData, limit: Long): String? {
        for (index in 0 until clip.itemCount) {
            val value = clip.getItemAt(index)?.text
            if (!value.isNullOrBlank()) return boundedText(value, limit)
        }
        return null
    }

    private fun binary(context: Context, pending: Pending, uri: Uri, declaredType: String): Boolean {
        if (uri.scheme != "content" || !mimeType.matches(declaredType) || declaredType.startsWith("text/")) return false
        var bytes: ByteArray? = null
        val read = NativeRuntimeCapture.scoped(pending.token, false, declaredType) { limit ->
            if (context.checkUriPermission(uri, Process.myPid(), Process.myUid(), Intent.FLAG_GRANT_READ_URI_PERMISSION) != PackageManager.PERMISSION_GRANTED) return@scoped
            val resolver = context.contentResolver
            val resolvedType = resolver.getType(uri)?.lowercase()
            if (resolvedType != null && resolvedType != declaredType) return@scoped
            val cap = minOf(maximumBinaryBytes.toLong(), limit).toInt()
            val source = resolver.openInputStream(uri)?.use { input ->
                pending.input(input)
                try { readBounded(input, cap, pending) } finally { pending.releaseInput(input) }
            } ?: return@scoped
            if (!pending.reading()) return@scoped
            bytes = if (declaredType.startsWith("image/")) normaliseImage(source, cap) else source
        }
        val payload = bytes ?: return false
        if (!read) return false
        return NativeRuntimeCapture.ingestBinary(pending.token, payload, if (declaredType.startsWith("image/")) "image/png" else declaredType, if (declaredType.startsWith("image/")) "" else filename(uri), uri.toString())
    }

    private fun filename(uri: Uri): String =
        uri.lastPathSegment
            ?.substringAfterLast('/')
            ?.substringAfterLast('\\')
            ?.takeIf { it.isNotBlank() && it.length <= 255 }
            ?: "attachment"

    private fun readBounded(input: java.io.InputStream, maximum: Int, pending: Pending): ByteArray? {
        val output = ByteArrayOutputStream()
        val buffer = ByteArray(8 * 1024)
        while (true) {
            if (!pending.reading()) return null
            val read = input.read(buffer)
            if (read < 0) break
            if (output.size() > maximum - read) return null
            output.write(buffer, 0, read)
        }
        return output.toByteArray().takeIf(ByteArray::isNotEmpty)
    }

    private fun normaliseImage(source: ByteArray, maximum: Int): ByteArray? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        runCatching { BitmapFactory.decodeByteArray(source, 0, source.size, bounds) }
            .getOrNull()
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        if (bounds.outWidth.toLong() > maximumDecodedImageBytes / 4 / bounds.outHeight.toLong()) {
            return null
        }
        val bitmap = runCatching {
            BitmapFactory.decodeByteArray(
                source,
                0,
                source.size,
                BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.ARGB_8888 },
            )
        }.getOrNull() ?: return null
        return try {
            BoundedOutputStream(maximum).use { output ->
                if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) return null
                output.bytes()
            }
        } catch (_: SizeLimitExceeded) {
            null
        } finally {
            bitmap.recycle()
        }
    }

    private class SizeLimitExceeded : IOException()

    private class BoundedOutputStream(private val maximum: Int) : OutputStream() {
        private val output = ByteArrayOutputStream()

        override fun write(value: Int) = write(byteArrayOf(value.toByte()), 0, 1)

        override fun write(bytes: ByteArray, offset: Int, length: Int) {
            if (length < 0 || output.size() > maximum - length) throw SizeLimitExceeded()
            output.write(bytes, offset, length)
        }

        fun bytes(): ByteArray = output.toByteArray()
    }
}
