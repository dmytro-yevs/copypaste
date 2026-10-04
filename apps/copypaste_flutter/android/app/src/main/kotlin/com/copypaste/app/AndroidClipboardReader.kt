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
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.OutputStream

internal object AndroidClipboardReader {
    private const val maximumBinaryBytes = 16 * 1024 * 1024
    private const val maximumDecodedImageBytes = 50 * 1024 * 1024L
    private val mimeType = Regex("^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$")

    fun captureBackground(context: Context): Boolean = captureClipboard(context, background = true)

    fun captureForeground(context: Context): Boolean =
        AndroidCaptureState.foregroundCaptureEnabled(context) &&
            captureClipboard(context, background = false)

    @Suppress("DEPRECATION")
    fun captureExplicit(context: Context, intent: Intent): Boolean {
        val captured = if (intent.action == Intent.ACTION_SEND) {
            val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                ?: intent.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri
            val declaredType = intent.type?.lowercase()
            val binaryCaptured = uri != null &&
                declaredType != null &&
                binary(context, uri, declaredType, explicit = true)
            binaryCaptured || intent.getCharSequenceExtra(Intent.EXTRA_TEXT)
                ?.takeIf { it.isNotBlank() }
                ?.let { NativeRuntimeCapture.ingestExplicitText(it.toString()) } == true
        } else if (intent.action == Intent.ACTION_PROCESS_TEXT) {
            intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)
                ?.takeIf { it.isNotBlank() }
                ?.let { NativeRuntimeCapture.ingestExplicitText(it.toString()) } == true
        } else {
            false
        }
        if (captured) AndroidCaptureFeedback.onCaptured(context)
        return captured
    }

    private fun captureClipboard(context: Context, background: Boolean): Boolean {
        if (!NativeRuntimeCapture.isImplicitCaptureAllowed()) return false
        val clipboard = context.getSystemService(ClipboardManager::class.java) ?: return false
        val primary = clipboard.primaryClip ?: return false
        if (primary.description.label?.toString() == "CopyPaste") return false
        val captured = binary(context, primary, explicit = false) ||
            text(primary)?.let(NativeRuntimeCapture::ingestText) == true
        if (captured) {
            if (background) AndroidCaptureState.recordBackgroundCapture(context)
            AndroidCaptureFeedback.onCaptured(context)
        }
        return captured
    }

    private fun text(clip: ClipData): String? {
        for (index in 0 until clip.itemCount) {
            val value = clip.getItemAt(index)?.text
            if (!value.isNullOrBlank()) return value.toString()
        }
        return null
    }

    private fun binary(context: Context, clip: ClipData, explicit: Boolean): Boolean {
        val uri = clip.getItemAt(0)?.uri ?: return false
        val declaredType = clip.description.getMimeType(0)?.lowercase() ?: return false
        return binary(context, uri, declaredType, explicit)
    }

    private fun binary(
        context: Context,
        uri: Uri,
        declaredType: String,
        explicit: Boolean,
    ): Boolean {
        if (uri.scheme != "content" || !mimeType.matches(declaredType) || declaredType.startsWith("text/")) {
            return false
        }
        if (context.checkUriPermission(
                uri,
                Process.myPid(),
                Process.myUid(),
                Intent.FLAG_GRANT_READ_URI_PERMISSION,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            return false
        }
        val resolver = context.contentResolver
        val resolvedType = runCatching { resolver.getType(uri)?.lowercase() }.getOrNull()
        if (resolvedType != null && resolvedType != declaredType) return false
        val source = try {
            resolver.openInputStream(uri)?.use(::readBounded)
        } catch (_: IOException) {
            null
        } catch (_: SecurityException) {
            null
        } ?: return false
        return if (declaredType.startsWith("image/")) {
            val png = normaliseImage(source) ?: return false
            if (explicit) {
                NativeRuntimeCapture.ingestExplicitBinary(png, "image/png", "", uri.toString())
            } else {
                NativeRuntimeCapture.ingestBinary(png, "image/png", "", uri.toString())
            }
        } else {
            if (explicit) {
                NativeRuntimeCapture.ingestExplicitBinary(
                    source,
                    declaredType,
                    filename(uri),
                    uri.toString(),
                )
            } else {
                NativeRuntimeCapture.ingestBinary(
                    source,
                    declaredType,
                    filename(uri),
                    uri.toString(),
                )
            }
        }
    }

    private fun filename(uri: Uri): String =
        uri.lastPathSegment
            ?.substringAfterLast('/')
            ?.substringAfterLast('\\')
            ?.takeIf { it.isNotBlank() && it.length <= 255 }
            ?: "attachment"

    private fun readBounded(input: java.io.InputStream): ByteArray? {
        val output = ByteArrayOutputStream()
        val buffer = ByteArray(8 * 1024)
        while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            if (output.size() > maximumBinaryBytes - read) return null
            output.write(buffer, 0, read)
        }
        return output.toByteArray().takeIf(ByteArray::isNotEmpty)
    }

    private fun normaliseImage(source: ByteArray): ByteArray? {
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
            BoundedOutputStream(maximumBinaryBytes).use { output ->
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
