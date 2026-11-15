package com.copypaste.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.graphics.BitmapFactory
import android.graphics.Bitmap
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import java.io.ByteArrayOutputStream

internal data class CaptureFeedbackPreview(
    val text: String,
    val imagePng: ByteArray? = null,
    val image: Boolean = false,
    val imageWidth: Int? = null,
    val imageHeight: Int? = null,
)

/** What a CopyPaste clipboard write announces to the user. */
internal enum class ClipboardWriteFeedback {
    /** Content received from another device: the capture notification. */
    Captured,

    /** A copy the user asked for: the copy sound. */
    Copied,

    /** A transient delivery for one paste: the delivering surface reports it. */
    Silent;

    companion object {
        fun of(received: Boolean, transient: Boolean): ClipboardWriteFeedback = when {
            received -> Captured
            transient -> Silent
            else -> Copied
        }
    }
}

internal object AndroidCaptureFeedback {
    private const val channelId = "clipboard-capture-events-silent"
    private const val notificationId = 2300
    private const val thumbnailEdge = 256

    fun textPreview(text: String): String {
        val normalized = text.replace("\r\n", "\n").replace("\r", "\n")
            .replace("\n", "⏎").replace("\t", "⇥")
        val count = normalized.codePointCount(0, normalized.length)
        return if (count <= 1000) normalized
        else normalized.substring(0, normalized.offsetByCodePoints(0, 1000)) + "…"
    }

    /** [width] and [height] describe the source when [image] is subsampled. */
    fun imagePreview(
        image: Bitmap,
        width: Int = image.width,
        height: Int = image.height,
    ): CaptureFeedbackPreview {
        val size = CaptureFeedbackPreview("", image = true, imageWidth = width, imageHeight = height)
        if (!NativeRuntimeCapture.notifyOnCopyEnabled() || !NativeRuntimeCapture.notificationPreviewEnabled()) {
            return size
        }
        val scale = minOf(1.0, thumbnailEdge.toDouble() / maxOf(width, height))
        val thumbnail = Bitmap.createScaledBitmap(
            image,
            maxOf(1, (width * scale).toInt()),
            maxOf(1, (height * scale).toInt()),
            true,
        )
        return try {
            val output = ByteArrayOutputStream()
            if (thumbnail.compress(Bitmap.CompressFormat.PNG, 100, output)) {
                size.copy(imagePng = output.toByteArray())
            } else size
        } finally {
            if (thumbnail !== image) thumbnail.recycle()
        }
    }

    fun binaryPreview(bytes: ByteArray, filename: String, mimeType: String): CaptureFeedbackPreview {
        if (!mimeType.startsWith("image/")) return CaptureFeedbackPreview(textPreview(filename))
        // Avoid decoding received image content unless its preview is enabled.
        if (!NativeRuntimeCapture.notifyOnCopyEnabled() || !NativeRuntimeCapture.notificationPreviewEnabled()) {
            return CaptureFeedbackPreview("", image = true)
        }
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        val width = bounds.outWidth
        val height = bounds.outHeight
        if (width <= 0 || height <= 0 ||
            width.toLong() > AndroidClipboardReader.maximumDecodedImageBytes / 4 / height.toLong()
        ) {
            return CaptureFeedbackPreview("", image = true)
        }
        val options = BitmapFactory.Options().apply { inSampleSize = thumbnailSampleSize(width, height) }
        val image = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)
            ?: return CaptureFeedbackPreview("", image = true)
        return try {
            imagePreview(image, width, height)
        } finally {
            image.recycle()
        }
    }

    /** The largest power-of-two subsampling that still covers the thumbnail edge. */
    internal fun thumbnailSampleSize(width: Int, height: Int): Int {
        val edge = maxOf(width, height)
        var sample = 1
        while (edge / (sample * 2) >= thumbnailEdge) sample *= 2
        return sample
    }

    fun onCaptured(context: Context, preview: CaptureFeedbackPreview? = null) {
        onCopied()
        if (NativeRuntimeCapture.notifyOnCopyEnabled() &&
            AndroidCaptureState.notificationGranted(context)
        ) {
            notify(context, if (NativeRuntimeCapture.notificationPreviewEnabled()) preview else null)
        }
    }

    fun onCopied() {
        if (NativeRuntimeCapture.soundOnCopyEnabled()) runCatching { playSound() }
    }

    private fun notify(context: Context, preview: CaptureFeedbackPreview?) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(
                    channelId,
                    context.getString(R.string.capture_channel_name),
                    NotificationManager.IMPORTANCE_DEFAULT,
                ).apply {
                    description = context.getString(R.string.capture_channel_description)
                    setSound(null, null)
                },
            )
        }
        val body = when {
            preview == null -> context.getString(R.string.capture_body_default)
            preview.image && preview.imageWidth != null && preview.imageHeight != null ->
                context.getString(R.string.capture_image_size, preview.imageWidth, preview.imageHeight)
            preview.image -> context.getString(R.string.capture_image)
            else -> preview.text
        }
        val builder = NotificationCompat.Builder(context, channelId)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentTitle(context.getString(R.string.capture_title))
                .setContentText(body)
                .setStyle(NotificationCompat.BigTextStyle().bigText(body))
                .setSilent(true)
                .setAutoCancel(true)
        val image = preview?.imagePng?.let { BitmapFactory.decodeByteArray(it, 0, it.size) }
        try {
            if (image != null) builder.setStyle(NotificationCompat.BigPictureStyle().bigPicture(image))
            NotificationManagerCompat.from(context).notify(notificationId, builder.build())
        } finally {
            image?.recycle()
        }
    }

    private fun playSound() {
        // Start audible feedback within the runtime completion scope. Releasing
        // an already-started tone later cannot publish a new capture success.
        val tone = ToneGenerator(AudioManager.STREAM_NOTIFICATION, 60)
        try {
            tone.startTone(ToneGenerator.TONE_PROP_ACK, 100)
            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({ tone.release() }, 120)
        } catch (_: RuntimeException) {
            tone.release()
        }
    }
}
