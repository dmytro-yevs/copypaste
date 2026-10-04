package com.copypaste.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import java.util.concurrent.atomic.AtomicInteger

internal object AndroidCaptureFeedback {
    private const val channelId = "clipboard-capture-events"
    private val nextNotificationId = AtomicInteger(2300)

    fun onCaptured(context: Context) {
        if (NativeRuntimeCapture.notifyOnCopyEnabled() &&
            AndroidCaptureState.notificationGranted(context)
        ) {
            notify(context)
        }
        if (NativeRuntimeCapture.soundOnCopyEnabled()) playSound()
    }

    private fun notify(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(
                    channelId,
                    "Clipboard saved",
                    NotificationManager.IMPORTANCE_DEFAULT,
                ).apply {
                    description = "Confirms that CopyPaste saved a clipboard change."
                },
            )
        }
        NotificationManagerCompat.from(context).notify(
            nextNotificationId.updateAndGet { current ->
                if (current == Int.MAX_VALUE) 2300 else current + 1
            },
            NotificationCompat.Builder(context, channelId)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentTitle("Clipboard saved")
                .setContentText("A new item was added to CopyPaste.")
                .setAutoCancel(true)
                .build(),
        )
    }

    private fun playSound() {
        Thread {
            val tone = ToneGenerator(AudioManager.STREAM_NOTIFICATION, 60)
            try {
                tone.startTone(ToneGenerator.TONE_PROP_ACK, 100)
                Thread.sleep(120)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
            } finally {
                tone.release()
            }
        }.start()
    }
}
