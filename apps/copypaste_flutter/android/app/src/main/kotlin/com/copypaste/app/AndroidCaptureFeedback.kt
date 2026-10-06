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

internal data class CaptureFeedbackPreview(val text: String, val imagePng: ByteArray? = null)

internal object AndroidCaptureFeedback {
    private const val channelId = "clipboard-capture-events-silent"
    private const val notificationId = 2300

    fun textPreview(text: String): String {
        val normalized = text.replace("\r\n", "\n").replace("\r", "\n")
            .replace("\n", "⏎").replace("\t", "⇥")
        val count = normalized.codePointCount(0, normalized.length)
        return if (count <= 1000) normalized
        else normalized.substring(0, normalized.offsetByCodePoints(0, 1000)) + "…"
    }

    fun imagePreview(image: Bitmap): CaptureFeedbackPreview {
        val text = "Image · ${image.width} × ${image.height}"
        if (!NativeRuntimeCapture.notifyOnCopyEnabled() || !NativeRuntimeCapture.notificationPreviewEnabled()) {
            return CaptureFeedbackPreview(text)
        }
        val scale = minOf(1.0, 256.0 / maxOf(image.width, image.height))
        val thumbnail = Bitmap.createScaledBitmap(
            image,
            maxOf(1, (image.width * scale).toInt()),
            maxOf(1, (image.height * scale).toInt()),
            true,
        )
        return try {
            val output = ByteArrayOutputStream()
            if (thumbnail.compress(Bitmap.CompressFormat.PNG, 100, output)) {
                CaptureFeedbackPreview(text, output.toByteArray())
            } else CaptureFeedbackPreview(text)
        } finally {
            if (thumbnail !== image) thumbnail.recycle()
        }
    }

    fun onCaptured(context: Context, preview: CaptureFeedbackPreview? = null) {
        if (NativeRuntimeCapture.notifyOnCopyEnabled() &&
            AndroidCaptureState.notificationGranted(context)
        ) {
            notify(context, if (NativeRuntimeCapture.notificationPreviewEnabled()) preview else null)
        }
        if (NativeRuntimeCapture.soundOnCopyEnabled()) playSound()
    }

    private fun notify(context: Context, preview: CaptureFeedbackPreview?) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(
                    channelId,
                    "Clipboard saved",
                    NotificationManager.IMPORTANCE_DEFAULT,
                ).apply {
                    description = "Confirms that CopyPaste saved a clipboard change."
                    setSound(null, null)
                },
            )
        }
        val body = preview?.text ?: "A new item was added to CopyPaste."
        val builder = NotificationCompat.Builder(context, channelId)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentTitle("Clipboard saved")
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
