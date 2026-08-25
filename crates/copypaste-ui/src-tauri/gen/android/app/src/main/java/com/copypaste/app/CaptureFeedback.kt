package com.copypaste.app

import android.content.Context
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Handler
import android.os.Looper
import android.util.Log

internal object CaptureFeedback {
    private const val TAG = "CopyPasteFeedback"
    private const val RELEASE_DELAY_MS = 500L

    fun play(context: Context) {
        val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        playIfAllowed(
            audio.ringerMode,
            audio.isStreamMute(AudioManager.STREAM_NOTIFICATION),
            audio.getStreamVolume(AudioManager.STREAM_NOTIFICATION),
        ) {
            queueTone()
        }
    }

    internal fun playIfAllowed(
        ringerMode: Int,
        muted: Boolean,
        volume: Int,
        play: () -> Unit,
    ): Boolean {
        if (ringerMode != AudioManager.RINGER_MODE_NORMAL || muted || volume <= 0) return false
        play()
        return true
    }

    private fun queueTone() {
        Handler(Looper.getMainLooper()).post {
            val tone = try {
                ToneGenerator(AudioManager.STREAM_NOTIFICATION, ToneGenerator.MAX_VOLUME)
            } catch (_: RuntimeException) {
                Log.d(TAG, "copy feedback is unavailable")
                return@post
            }
            if (!tone.startTone(ToneGenerator.TONE_PROP_ACK)) {
                tone.release()
                return@post
            }
            Handler(Looper.getMainLooper()).postDelayed(tone::release, RELEASE_DELAY_MS)
        }
    }
}
