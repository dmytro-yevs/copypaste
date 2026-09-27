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
import android.util.Base64
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.OutputStream

internal data class ClipboardRead(
    val outcome: ReadOutcome,
    val clip: CapturedClip?,
    val sourceAppBundleId: String?,
    val sourceAppName: String?,
)

private const val MAX_BINARY_BYTES = ClipQueue.MAX_BINARY_BYTES
// Matches the shared default decoder budget. The current capture bridge has no
// config field for a live override, so Android fails closed at that maximum.
private const val MAX_DECODED_IMAGE_BYTES = 50 * 1024 * 1024L
private val MIME_TYPE = Regex("^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$")

internal fun clipboardRead(context: Context, source: CaptureSource): ClipboardRead {
    val sourcePackage = ShizukuClipboard.sourcePackage()
    // This gate is deliberately before obtaining a provider-backed payload.
    // A private or excluded source is never dereferenced, even transiently.
    if (!ClipQueue.acceptsCapture(source, sourcePackage)) {
        return ClipboardRead(ReadOutcome.EMPTY, null, sourcePackage, null)
    }
    val sourceName = PackageFacts.label(context, sourcePackage)
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    val primary = clipboard.primaryClip
    val captured = primary?.let { clipFromClipboard(context, it, source, sourcePackage, sourceName) }
    val outcome = when {
        captured != null -> ReadOutcome.SUCCEEDED
        primary != null -> ReadOutcome.EMPTY
        clipboard.hasPrimaryClip() -> ReadOutcome.REFUSED
        else -> ReadOutcome.EMPTY
    }
    return ClipboardRead(outcome, captured, sourcePackage, sourceName)
}

/** Reads an explicit ACTION_SEND attachment through the same bounded intake. */
internal fun sharedClip(context: Context, intent: Intent): CapturedClip? {
    if (!ClipQueue.acceptsCapture(CaptureSource.SHARE, null)) return null
    val text = intent.getStringExtra(Intent.EXTRA_TEXT)?.takeIf(String::isNotBlank)
    if (text != null) return textClip(text, CaptureSource.SHARE, null, null)
    val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM) ?: intent.clipData?.getItemAt(0)?.uri
    if (uri == null || intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION == 0) return null
    return binaryClip(context, uri, intent.type, CaptureSource.SHARE, null, null, requireGrant = true)
}

private fun clipFromClipboard(
    context: Context,
    primary: ClipData,
    source: CaptureSource,
    sourcePackage: String?,
    sourceName: String?,
): CapturedClip? {
    if (ClipSensitivity.isSensitive(primary)) return null
    ClipSensitivity.asText(primary)?.let { return textClip(it, source, sourcePackage, sourceName) }
    val uri = primary.getItemAt(0)?.uri ?: return null
    val declared = primary.description.getMimeType(0)
    return binaryClip(context, uri, declared, source, sourcePackage, sourceName, requireGrant = true)
}

private fun textClip(
    text: String,
    source: CaptureSource,
    sourcePackage: String?,
    sourceName: String?,
) = CapturedClip(text, null, null, null, source, System.currentTimeMillis(), sourcePackage, sourceName)

private fun binaryClip(
    context: Context,
    uri: Uri,
    declaredType: String?,
    source: CaptureSource,
    sourcePackage: String?,
    sourceName: String?,
    requireGrant: Boolean,
): CapturedClip? {
    if (uri.scheme != "content" || !validBinaryMime(declaredType)) return null
    if (requireGrant && context.checkUriPermission(
            uri,
            Process.myPid(),
            Process.myUid(),
            Intent.FLAG_GRANT_READ_URI_PERMISSION,
        ) != PackageManager.PERMISSION_GRANTED
    ) return null

    val resolver = context.contentResolver
    val resolvedType = try {
        resolver.getType(uri)?.lowercase()
    } catch (_: SecurityException) {
        return null
    } catch (_: RuntimeException) {
        return null
    }
    val contentType = declaredType?.lowercase() ?: return null
    // The provider must agree with the clipboard declaration; no URI or file
    // extension is ever used as a fallback MIME classifier.
    if (resolvedType != null && resolvedType != contentType) return null
    val bytes = try {
        resolver.openInputStream(uri)?.use { input -> readBounded(input) }
    } catch (_: IOException) {
        null
    } catch (_: SecurityException) {
        null
    } catch (_: RuntimeException) {
        null
    } ?: return null
    val payload = if (contentType.startsWith("image/")) normaliseImage(bytes) ?: return null else bytes
    val queuedType = if (contentType.startsWith("image/")) "image/png" else contentType
    val filename = if (contentType.startsWith("image/")) null else uri.lastPathSegment
        ?.substringAfterLast('/')
        ?.substringAfterLast('\\')
        ?.takeIf { it.isNotBlank() && it.length <= 255 }
        ?: "attachment"
    return CapturedClip(
        null,
        Base64.encodeToString(payload, Base64.NO_WRAP),
        queuedType,
        filename,
        source,
        System.currentTimeMillis(),
        sourcePackage,
        sourceName,
    )
}

private fun validBinaryMime(value: String?): Boolean {
    val mime = value?.lowercase() ?: return false
    return MIME_TYPE.matches(mime) && !mime.startsWith("text/")
}

private fun readBounded(input: java.io.InputStream): ByteArray? {
    val output = ByteArrayOutputStream()
    val buffer = ByteArray(8 * 1024)
    while (true) {
        val read = input.read(buffer)
        if (read < 0) break
        if (output.size() > MAX_BINARY_BYTES - read) return null
        output.write(buffer, 0, read)
    }
    return output.toByteArray()
}

/** Decode dimensions before pixels, then emit the one image format every
 * platform's existing binary preview and clipboard paths understand. */
internal fun normaliseImage(source: ByteArray): ByteArray? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    try {
        BitmapFactory.decodeByteArray(source, 0, source.size, bounds)
    } catch (_: RuntimeException) {
        return null
    }
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    if (exceedsDecodedImageBudget(bounds.outWidth, bounds.outHeight)) return null
    val bitmap = try {
        BitmapFactory.decodeByteArray(
            source,
            0,
            source.size,
            BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.ARGB_8888 },
        )
    } catch (_: RuntimeException) {
        null
    } ?: return null
    return try {
        BoundedOutputStream(MAX_BINARY_BYTES).use { output ->
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) return null
            output.bytes()
        }
    } catch (_: SizeLimitExceeded) {
        null
    } finally {
        bitmap.recycle()
    }
}

internal fun exceedsDecodedImageBudget(width: Int, height: Int): Boolean =
    width <= 0 || height <= 0 ||
        width.toLong() > MAX_DECODED_IMAGE_BYTES / 4 / height.toLong()

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

internal fun clipboardOutcome(context: Context): ReadOutcome =
    clipboardRead(context, CaptureSource.IN_APP).outcome
