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

internal object AndroidClipboardReader {
    private const val maximumBinaryBytes = 16 * 1024 * 1024
    private const val maximumDecodedImageBytes = 50 * 1024 * 1024L
    private val mimeType = Regex("^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$")
    private val ingestion = java.util.concurrent.ThreadPoolExecutor(1, 1, 0L, java.util.concurrent.TimeUnit.MILLISECONDS, java.util.concurrent.ArrayBlockingQueue(32))

    private val main by lazy { android.os.Handler(android.os.Looper.getMainLooper()) }
    // Native admission also limits hosts to 32. Every accepted host/input owns
    // capacity for its eventual cleanup, even if another cleanup blocks.
    private val hostSlots = CaptureSlots(32)
    private val pendingSlots = CaptureSlots(33)
    private val inputSlots = CaptureSlots(33)
    // Up to 4 Mi UTF-16 units: owned UTF-16 plus transient and JNI UTF-8 copies.
    private val textBytes = CaptureBytes(32L * 1024 * 1024)
    private val lifecycle = captureCleanupExecutor(32)
    private val streamCleanup = captureCleanupExecutor(33)
    private val heavy = CaptureHeavyAdmission()

    internal class Host(val id: Long, private val slot: CaptureSlots.Lease? = null) {
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
                slot?.close()
                main.post { callbacks.forEach { it(result) } }
            }
        }
    }

    fun openHost(explicit: Boolean = false): Host? {
        val slot = hostSlots.reserve() ?: return null
        val id = runCatching { NativeRuntimeCapture.openHost(explicit) }.getOrDefault(0L)
        if (id <= 0L) { slot.close(); return null }
        return Host(id, slot)
    }

    private val foregroundOwner = ForegroundCaptureOwner<Host>(
        open = { openHost() },
        close = { host, completion -> host.close(completion) },
    )
    fun acquireForeground(previous: Host?): Host? = foregroundOwner.acquire(previous)
    fun retireForeground(host: Host?) = foregroundOwner.retire(host)
    fun disableForeground(completion: (Boolean) -> Unit) = foregroundOwner.disable(completion)

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

    internal data class Snapshot(val text: String? = null, val uri: Uri? = null, val type: String? = null, val capturedAt: Long = 0, val secret: Boolean = false)

    internal interface PendingRuntime {
        fun enqueue(task: Runnable)
        fun remove(task: Runnable): Boolean
        fun postMain(action: () -> Unit)
        fun closeInput(input: java.io.InputStream)
        fun abandon(token: Long)
        fun scoped(token: Long, completion: Boolean, contentType: String, callback: CaptureCallback): Boolean
        fun onCaptured(context: Context?, preview: CaptureFeedbackPreview?)
        fun closeHost(host: Host)
    }

    private object NativePendingRuntime : PendingRuntime {
        override fun enqueue(task: Runnable) { ingestion.execute(task) }
        override fun remove(task: Runnable): Boolean = ingestion.remove(task)
        override fun postMain(action: () -> Unit) { main.post(action) }
        override fun closeInput(input: java.io.InputStream) {
            streamCleanup.execute { runCatching { input.close() } }
        }
        override fun abandon(token: Long) { NativeRuntimeCapture.abandon(token) }
        override fun scoped(token: Long, completion: Boolean, contentType: String, callback: CaptureCallback): Boolean =
            NativeRuntimeCapture.scoped(token, completion, contentType, callback)
        override fun onCaptured(context: Context?, preview: CaptureFeedbackPreview?) {
            AndroidCaptureFeedback.onCaptured(requireNotNull(context), preview)
        }
        override fun closeHost(host: Host) { host.close() }
    }

    internal class Pending(
        private val host: Host,
        completion: ((Boolean) -> Unit)? = null,
        private val runtime: PendingRuntime = NativePendingRuntime,
        private val slot: CaptureSlots.Lease? = null,
    ) : Runnable {
        private val workLock = Any()
        // NEW, QUEUED, RUNNING, DONE, SNAPSHOTTING; slots survive in-flight work.
        private var workState = 0
        private var textLease: CaptureSlots.Lease? = null
        private fun doneLocked() { workState = 3; textLease?.close(); textLease = null; slot?.close() }
        fun retainText(lease: CaptureSlots.Lease): Boolean = synchronized(workLock) {
            if ((workState != 0 && workState != 4) || textLease != null || !reading()) false else { textLease = lease; true }
        }
        private val capability = java.util.concurrent.atomic.AtomicLong(0)
        val token: Long get() = capability.get()
        private val cancelled = java.util.concurrent.atomic.AtomicBoolean(false)
        private val completing = java.util.concurrent.atomic.AtomicBoolean(false)
        private val result = java.util.concurrent.atomic.AtomicReference(completion)
        private val payload = java.util.concurrent.atomic.AtomicReference<Snapshot?>(null)
        private val feedback = java.util.concurrent.atomic.AtomicReference<CaptureFeedbackPreview?>(null)
        private val confidential = java.util.concurrent.atomic.AtomicBoolean(false)
        fun confidential(value: Boolean) { if (value) confidential.set(true) }
        fun feedbackPreview(): CaptureFeedbackPreview? = if (confidential.get()) null else feedback.get()
        fun feedbackPreview(preview: CaptureFeedbackPreview) { feedback.set(preview) }
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
            synchronized(workLock) {
                if (workState == 0 || (workState == 1 && runCatching { runtime.remove(this) }.getOrDefault(false))) doneLocked()
            }
            heavy.wake()
            inputSlots.wake()
            stream.getAndSet(null)?.let { input -> runCatching { runtime.closeInput(input) } }
            if (result.get() != null) finish(false) else cleanup()
        }
        fun enqueue(snapshot: Snapshot, work: (Snapshot) -> Boolean) {
            synchronized(workLock) {
                if (workState != 0 || !reading()) { cancel(); cleanup(); return }
                payload.set(snapshot)
                action.set(work)
                workState = 1
            }
            try {
                runtime.enqueue(this)
                synchronized(workLock) {
                    if (!reading() && workState == 1 && runtime.remove(this)) doneLocked()
                }
            } catch (_: RuntimeException) {
                synchronized(workLock) { if (workState == 1) doneLocked() }
                cancel()
            }
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
            feedback.set(null)
            action.set(null)
            host.pending.remove(this)
            synchronized(workLock) { if (workState == 0) doneLocked() }
            val token = capability.getAndSet(0)
            if (token > 0) runCatching { runtime.abandon(token) }
        }
        fun finish(saved: Boolean) {
            if (!completing.compareAndSet(false, true)) return
            runtime.postMain {
                val callback = result.getAndSet(null)
                try {
                    var published = false
                    if (saved && reading()) {
                        runCatching {
                            runtime.scoped(token, true, "text/plain") {
                                runtime.onCaptured(applicationContext, feedbackPreview())
                                published = true
                                callback?.invoke(true)
                            }
                        }
                    }
                    if (!published) callback?.invoke(false)
                } finally { cleanup(); runtime.closeHost(host) }
            }
        }
        fun read(contentType: String, callback: CaptureCallback): Boolean =
            runtime.scoped(token, false, contentType, callback)
        private var applicationContext: Context? = null
        fun explicitContext(context: Context) { applicationContext = context.applicationContext }
        fun snapshot(read: () -> Boolean): Boolean {
            synchronized(workLock) {
                if (workState != 0 || !reading()) return false
                workState = 4
            }
            return try { read() } finally {
                synchronized(workLock) {
                    if (workState == 4) {
                        if (reading()) workState = 0 else doneLocked()
                    }
                }
            }
        }
        fun direct(work: () -> Boolean): Boolean {
            synchronized(workLock) {
                if (workState != 0 || !reading()) return false
                workState = 2
            }
            return try { work() } finally { synchronized(workLock) { doneLocked() } }
        }
        override fun run() {
            synchronized(workLock) { if (workState != 1) return; workState = 2 }
            var saved = false
            try {
                val snapshot = payload.getAndSet(null)
                val work = action.getAndSet(null)
                if (snapshot != null && work != null && reading()) saved = work(snapshot)
            } catch (_: Exception) {
                saved = false
            } finally {
                synchronized(workLock) { doneLocked() }
                if (result.get() != null) finish(saved) else cleanup()
            }
        }
    }

    private fun begin(host: Host, completion: ((Boolean) -> Unit)? = null): Pending? {
        if (host.closed.get()) return null
        val slot = pendingSlots.reserve() ?: return null
        val pending = Pending(host, completion, slot = slot)
        host.pending.add(pending)
        pending.attach(runCatching { NativeRuntimeCapture.begin(host.id, Runnable(pending::cancel)) }.getOrDefault(0L))
        if (pending.token <= 0L || host.closed.get()) { pending.cancel(); return if (completion != null) pending else null }
        return pending
    }

    private fun boundedText(value: CharSequence?, limit: Long, pending: Pending): String? {
        if (value == null || value.length.toLong() > limit) return null
        val lease = textBytes.reserve(value.length.toLong() * 8) ?: return null
        var retained = false
        try {
            val text = value.toString().takeIf { it.toByteArray(Charsets.UTF_8).size.toLong() <= limit } ?: return null
            retained = pending.retainText(lease)
            return if (retained) text else null
        } finally { if (!retained) lease.close() }
    }

    @Suppress("DEPRECATION")
    fun captureExplicit(context: Context, intent: Intent, host: Host, completion: (Boolean) -> Unit) {
        val pending = begin(host, completion) ?: return host.close { completion(false) }
        pending.explicitContext(context)
        if (pending.token <= 0L || !pending.reading()) return
        var snapshot: Snapshot? = null
        val read = pending.snapshot { runCatching { pending.read("text/plain") { limit ->
            val uri = if (intent.action == Intent.ACTION_SEND) {
                intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                    ?: intent.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri
            } else null
            val text = intent.getCharSequenceExtra(if (intent.action == Intent.ACTION_PROCESS_TEXT) Intent.EXTRA_PROCESS_TEXT else Intent.EXTRA_TEXT)
            snapshot = Snapshot(text = boundedText(text, limit, pending), uri = uri, type = intent.type?.lowercase())
        } }.getOrDefault(false) }
        val captured = snapshot
        if (!read || captured == null) { pending.cancel(); return }
        val app = context.applicationContext
        pending.enqueue(captured) { data -> materialize(app, pending, data) }
    }

    private fun captureClipboard(context: Context, host: Host, background: Boolean) {
        val pending = begin(host) ?: return
        val clipboard = context.getSystemService(ClipboardManager::class.java)
        var snapshot: Snapshot? = null
        val read = pending.snapshot { runCatching { pending.read("text/plain") { limit ->
            val description = clipboard?.primaryClipDescription
            val secret = description?.extras?.getBoolean("android.content.extra.IS_SENSITIVE", false) == true
            if (!NativeRuntimeCapture.classify(pending.token, secret, false)) return@read
            val primary = clipboard?.primaryClip
            if (primary != null && !AndroidClipboardWriter.isOwned(context, primary.description)) {
                val currentSecret = primary.description.extras?.getBoolean("android.content.extra.IS_SENSITIVE", false) == true
                if (!NativeRuntimeCapture.classify(pending.token, currentSecret, false) || currentSecret != secret) return@read
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && description?.timestamp != primary.description.timestamp) return@read
                pending.confidential(currentSecret)
                snapshot = Snapshot(
                    secret = currentSecret,
                    text = text(primary, limit, pending),
                    uri = primary.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri,
                    type = primary.description.takeIf { it.mimeTypeCount > 0 }?.getMimeType(0)?.lowercase(),
                    capturedAt = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) primary.description.timestamp else System.currentTimeMillis(),
                )
            }
        } }.getOrDefault(false) }
        val captured = snapshot
        if (!read || captured == null) { pending.cancel(); return }
        val app = context.applicationContext
        pending.enqueue(captured) { data ->
            val saved = materialize(app, pending, data)
            if (saved) {
                NativeRuntimeCapture.scoped(pending.token, true, "text/plain") {
                    if (background) AndroidCaptureState.recordBackgroundCapture(app, data.capturedAt)
                    AndroidCaptureFeedback.onCaptured(app, pending.feedbackPreview())
                }
            }
            saved
        }
    }

    fun readCaptureMetadata(host: Host, action: () -> Unit): Boolean {
        val pending = begin(host) ?: return false
        return try { pending.direct { pending.read("image/png") { action() } } } finally { pending.cleanup() }
    }

    fun captureScreenshot(context: Context, host: Host, uri: Uri, type: String, displayName: String): Boolean {
        if (!ScreenshotCaptureState.enabled(context) || !ScreenshotCaptureState.mediaGranted(context)) return false
        val pending = begin(host) ?: return false
        return try {
            pending.direct {
                val saved = binary(context, pending, uri, type, mediaAccess = true, sourceDisplayName = displayName)
                if (saved) NativeRuntimeCapture.scoped(pending.token, true, "image/png") {
                    AndroidCaptureFeedback.onCaptured(context, null)
                }
                saved
            }
        } finally { pending.cleanup() }
    }

    private fun materialize(context: Context, pending: Pending, snapshot: Snapshot): Boolean {
        // A binary payload never falls back to textual/base64 capture.
        return if (snapshot.uri != null) binary(context, pending, snapshot.uri, snapshot.type ?: return false)
        else snapshot.text?.takeIf(String::isNotBlank)?.let {
            val saved = NativeRuntimeCapture.ingestText(pending.token, it)
            if (saved && !snapshot.secret) pending.feedbackPreview(CaptureFeedbackPreview(AndroidCaptureFeedback.textPreview(it)))
            saved
        } == true
    }

    private fun text(clip: ClipData, limit: Long, pending: Pending): String? {
        for (index in 0 until clip.itemCount) {
            val value = clip.getItemAt(index)?.text
            if (!value.isNullOrBlank()) return boundedText(value, limit, pending)
        }
        return null
    }

    private fun binary(context: Context, pending: Pending, uri: Uri, declaredType: String, mediaAccess: Boolean = false, sourceDisplayName: String = ""): Boolean {
        if (uri.scheme != "content" || !mimeType.matches(declaredType)) return false
        if (!declaredType.startsWith("image/") && !mediaAccess) return captureFile(context,pending,uri,declaredType)
        if (!heavy.acquire(pending::reading)) return false
        return try { binaryImage(context, pending, uri, declaredType, mediaAccess, sourceDisplayName) } finally { heavy.release() }
    }

    private fun openInput(context: Context, uri: Uri, pending: Pending): CaptureInput? {
        val slot = inputSlots.await(pending::reading) ?: return null
        return try {
            val input = context.contentResolver.openInputStream(uri)
            if (input == null) { slot.close(); null } else CaptureInput(input, slot)
        } catch (error: Exception) { slot.close(); throw error }
    }

    private fun binaryImage(context: Context, pending: Pending, uri: Uri, declaredType: String, mediaAccess: Boolean, sourceDisplayName: String): Boolean {
        var bytes: BinaryPayload? = null
        var sourceApp: ScreenshotSourceApp? = null
        val read = pending.read(declaredType) { limit ->
            if (mediaAccess) {
                if (!ScreenshotCaptureState.enabled(context) || !ScreenshotCaptureState.mediaGranted(context) ||
                    uri.authority != "media" || !declaredType.startsWith("image/")) return@read
            } else if (context.checkUriPermission(uri, Process.myPid(), Process.myUid(), Intent.FLAG_GRANT_READ_URI_PERMISSION) != PackageManager.PERMISSION_GRANTED) return@read
            val resolver = context.contentResolver
            val resolvedType = resolver.getType(uri)?.lowercase()
            if (resolvedType != null && resolvedType != declaredType) return@read
            val cap = minOf(maximumBinaryBytes.toLong(), limit).toInt()
            val source = openInput(context, uri, pending)?.use { input ->
                pending.input(input)
                try { readBounded(input, cap, pending) } finally { pending.releaseInput(input) }
            } ?: return@read
            if (!pending.reading()) return@read
            bytes = if (declaredType.startsWith("image/")) normaliseImage(source, cap, pending) else BinaryPayload(source, source.size)
            if (mediaAccess) sourceApp = ScreenshotSourceApps.resolve(context, sourceDisplayName)
        }
        val payload = bytes ?: return false
        if (!read) return false
        val saved = NativeRuntimeCapture.ingestBinary(pending.token, payload.bytes, payload.length, if (declaredType.startsWith("image/")) "image/png" else declaredType, if (declaredType.startsWith("image/")) "" else filename(uri), uri.toString(), sourceApp?.packageName.orEmpty(), sourceApp?.name.orEmpty(), sourceApp?.icon ?: byteArrayOf())
        if (saved && !declaredType.startsWith("image/")) pending.feedbackPreview(CaptureFeedbackPreview(filename(uri)))
        return saved
    }

    private fun captureFile(context: Context, pending: Pending, uri: Uri, type: String): Boolean {
        var staged: java.io.File? = null
        var name = filename(uri)
        try {
            val read = pending.read("file") { limit ->
                if (context.checkUriPermission(uri, Process.myPid(), Process.myUid(), Intent.FLAG_GRANT_READ_URI_PERMISSION) != PackageManager.PERMISSION_GRANTED) return@read
                context.contentResolver.query(uri,arrayOf(android.provider.OpenableColumns.DISPLAY_NAME),null,null,null)?.use { cursor ->
                    if (cursor.moveToFirst() && !cursor.isNull(0)) name=cursor.getString(0)
                }
                val file=java.io.File.createTempFile("capture-file-", ".bin",context.cacheDir)
                staged=file
                openInput(context, uri, pending)?.use { input ->
                    pending.input(input)
                    try {
                        file.outputStream().use { output ->
                            val buffer=ByteArray(64*1024); var total=0L
                            while (pending.reading()) {
                                val count=input.read(buffer)
                                if (count<0) break
                                total+=count
                                require(total<=limit) { "The file exceeds the file size limit in Settings." }
                                output.write(buffer,0,count)
                            }
                        }
                    } finally {pending.releaseInput(input)}
                } ?: throw java.io.IOException("The file could not be read.")
            }
            val file=staged ?: return false
            return read && pending.reading() && NativeRuntimeCapture.ingestFile(pending.token,file.absolutePath,name,type,uri.toString())
        } finally {staged?.delete()}
    }

    private fun filename(uri: Uri): String =
        uri.lastPathSegment
            ?.substringAfterLast('/')
            ?.substringAfterLast('\\')
            ?.takeIf { it.isNotBlank() && it.length <= 255 }
            ?: "attachment"

    internal fun readBounded(input: java.io.InputStream, maximum: Int, pending: Pending): ByteArray? {
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

    private fun normaliseImage(source: ByteArray, maximum: Int, pending: Pending): BinaryPayload? {
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
            runCatching { AndroidCaptureFeedback.imagePreview(bitmap) }
                .getOrNull()?.let(pending::feedbackPreview)
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

    internal data class BinaryPayload(val bytes: ByteArray, val length: Int)

    internal class SizeLimitExceeded : IOException()

    internal class BoundedOutputStream(private val maximum: Int) : OutputStream() {
        private class OwnedOutput : ByteArrayOutputStream() {
            fun payload(): BinaryPayload = BinaryPayload(buf, count)
        }
        private val output = OwnedOutput()

        override fun write(value: Int) = write(byteArrayOf(value.toByte()), 0, 1)

        override fun write(bytes: ByteArray, offset: Int, length: Int) {
            if (length < 0 || output.size() > maximum - length) throw SizeLimitExceeded()
            output.write(bytes, offset, length)
        }

        fun bytes(): BinaryPayload = output.payload()
    }
}
