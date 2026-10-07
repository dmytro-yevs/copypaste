package com.copypaste.app

import android.content.Context
import android.database.ContentObserver
import android.os.Handler
import android.os.HandlerThread
import android.provider.Telephony

/** Observes the inbox, including hash-addressed SMS that older Android versions
 * deliver only to the intended app. No historical message body is imported. */
internal class SmsInboxMonitor(private val context: Context) {
    private val thread = HandlerThread("copypaste-sms-inbox").apply { start() }
    private val worker = Handler(thread.looper)
    private val preferences = context.getSharedPreferences("sms-module-inbox", Context.MODE_PRIVATE)
    @Volatile private var closed = false
    @Volatile private var host = 0L
    private val observer = object : ContentObserver(worker) {
        override fun onChange(selfChange: Boolean) = scan()
    }

    fun start(completion: (Boolean) -> Unit) = worker.post {
        MainActivity.ensureRuntime(context)
        if (closed || !NativeSmsModules.hasHandler() || !AndroidSmsAccess.granted(context)) {
            completion(false); return@post
        }
        host = NativeSmsModules.openHost()
        if (host == 0L) { completion(false); return@post }
        try {
            if (closed) { completion(false); return@post }
            context.contentResolver.registerContentObserver(Telephony.Sms.CONTENT_URI, true, observer)
            completion(true)
            scan()
        } catch (_: Exception) { completion(false) }
    }

    fun changed() = worker.post { scan() }

    fun close() {
        closed = true
        if (host != 0L) NativeRuntimeCapture.revokeHost(host)
        worker.post {
            context.contentResolver.unregisterContentObserver(observer)
            if (host != 0L) {
                NativeRuntimeCapture.revokeHost(host)
                NativeRuntimeCapture.drainHost(host)
            }
            thread.quitSafely()
        }
    }

    private fun scan() {
        if (closed || host == 0L || !NativeSmsModules.hasHandler() || !AndroidSmsAccess.granted(context)) return
        val activated = maxOf(preferences.getLong("activated-at", 0L), preferences.getLong("skip-before", 0L))
        if (activated == 0L) return
        val token = NativeRuntimeCapture.begin(host, Runnable {})
        if (token == 0L) {
            preferences.edit().putLong("skip-before", System.currentTimeMillis() + 1L).commit()
            return
        }
        try {
            val messages = mutableListOf<Pair<Long, String>>()
            val read = NativeRuntimeCapture.scoped(token, false, "text/plain", CaptureCallback { limit ->
                val lastId = preferences.getLong("last-id", 0L)
                context.contentResolver.query(
                    Telephony.Sms.Inbox.CONTENT_URI,
                    arrayOf(Telephony.Sms._ID, Telephony.Sms.BODY),
                    "${Telephony.Sms._ID} > ? AND ${Telephony.Sms.DATE} >= ?",
                    arrayOf(lastId.toString(), activated.toString()),
                    "${Telephony.Sms._ID} ASC LIMIT 32",
                )?.use { cursor ->
                    while (!closed && cursor.moveToNext()) {
                        val id = cursor.getLong(0)
                        val text = cursor.getString(1).orEmpty()
                        messages.add(id to if (text.toByteArray().size <= minOf(limit, 65_536L)) text else "")
                    }
                }
            })
            if (!read || closed) return
            for ((id, text) in messages) {
                if (closed) return
                // Metadata only. Mark each row once, even when paused or no code matches.
                preferences.edit().putLong("last-id", id).commit()
                val operation = NativeRuntimeCapture.begin(host, Runnable {})
                if (operation == 0L) continue
                try { NativeSmsModules.ingest(operation, text) }
                finally { NativeRuntimeCapture.abandon(operation) }
            }
            if (messages.size == 32) worker.post { scan() }
        } catch (_: Exception) {
            // SMS bodies and provider failures must not enter logs or UI errors.
        } finally { NativeRuntimeCapture.abandon(token) }
    }

    companion object {
        fun setEnabled(context: Context, enabled: Boolean) {
            val preferences = context.getSharedPreferences("sms-module-inbox", Context.MODE_PRIVATE)
            if (!enabled) preferences.edit().clear().commit()
            else if (preferences.getLong("activated-at", 0L) == 0L) {
                preferences.edit().putLong("activated-at", System.currentTimeMillis()).putLong("last-id", 0L).commit()
            }
        }
    }
}
