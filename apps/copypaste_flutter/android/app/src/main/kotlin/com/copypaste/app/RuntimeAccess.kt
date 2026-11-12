package com.copypaste.app

import android.content.Context
import android.os.Handler
import android.os.Looper
import java.util.concurrent.Executor
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject

/** Exchanges one IPC method JSON for one IPC response JSON. */
internal fun interface RuntimeTransport {
    fun exchange(method: String): String
}

/** IPC error codes; a code this build does not know maps to [UNKNOWN]. */
internal enum class RuntimeErrorCode(val wire: String) {
    NOT_FOUND("not_found"),
    INVALID_REQUEST("invalid_request"),
    PROTOCOL_MISMATCH("protocol_mismatch"),
    NOT_READY("not_ready"),
    AUTH_FAILED("auth_failed"),
    KEY_LOCKED("key_locked"),
    KEY_UNUSABLE("key_unusable"),
    UNSUPPORTED_CONTENT("unsupported_content"),
    CONTENT_TOO_LARGE("content_too_large"),
    PAIRING_CODE("pairing_code"),
    PAIRING_ADDRESS("pairing_address"),
    PAIRING_ALREADY_EXISTS("pairing_already_exists"),
    RATE_LIMITED("rate_limited"),
    PEER_UNREACHABLE("peer_unreachable"),
    PAIRING_LIMIT("pairing_limit"),
    PEER_FAILED("peer_failed"),
    PEER_VERSION("peer_version"),
    PEER_NOT_FOUND("peer_not_found"),
    FORBIDDEN("forbidden"),
    UNSUPPORTED("unsupported"),
    INTERNAL("internal"),
    UNKNOWN("");

    companion object {
        fun fromWire(code: String?): RuntimeErrorCode =
            entries.firstOrNull { it != UNKNOWN && it.wire == code } ?: UNKNOWN
    }
}

internal sealed interface RuntimeReply {
    /** [kind] names the response data variant, such as `items` or `empty`. */
    data class Ok(val kind: String, val data: Any?) : RuntimeReply {
        fun objectData(): JSONObject? = data as? JSONObject
        fun arrayData(): JSONArray? = data as? JSONArray
    }

    data class Failure(val code: RuntimeErrorCode, val message: String) : RuntimeReply
}

/** Blocking runtime access; callers run it on [RuntimeAccessWorker], never the main thread. */
internal interface RuntimeAccess {
    fun request(method: String, params: JSONObject? = null): RuntimeReply
}

internal class JsonRuntimeAccess(private val transport: RuntimeTransport) : RuntimeAccess {
    override fun request(method: String, params: JSONObject?): RuntimeReply {
        val envelope = JSONObject().put("method", method)
        if (params != null) envelope.put("params", params)
        val raw = try {
            transport.exchange(envelope.toString())
        } catch (_: RuntimeException) {
            return transportFailure
        } catch (_: LinkageError) {
            return transportFailure
        }
        return decode(raw)
    }

    companion object {
        private val transportFailure =
            RuntimeReply.Failure(RuntimeErrorCode.INTERNAL, "The runtime could not be reached.")
        private val malformed =
            RuntimeReply.Failure(RuntimeErrorCode.INTERNAL, "The runtime reply could not be read.")

        fun decode(raw: String): RuntimeReply {
            val response = try {
                JSONObject(raw)
            } catch (_: JSONException) {
                return malformed
            }
            if (!response.optBoolean("ok", false)) {
                val message = response.optString("error").takeIf { it.isNotEmpty() } ?: return malformed
                return RuntimeReply.Failure(
                    RuntimeErrorCode.fromWire(response.optString("error_code").ifEmpty { null }),
                    message,
                )
            }
            val data: Any? = response.opt("data")
            if (data is String) return RuntimeReply.Ok(data, null)
            if (data !is JSONObject) return malformed
            val kind = data.keys().asSequence().singleOrNull() ?: return malformed
            return RuntimeReply.Ok(kind, data.opt(kind).takeUnless { it == JSONObject.NULL })
        }
    }
}

/**
 * Runs runtime requests on one background thread and delivers each reply on
 * [deliver]. The thread exits after a short idle period.
 */
internal class RuntimeAccessWorker(
    private val access: RuntimeAccess,
    private val executor: Executor,
    private val deliver: Executor,
) {
    fun request(method: String, params: JSONObject? = null, onReply: (RuntimeReply) -> Unit) {
        executor.execute {
            val reply = access.request(method, params)
            deliver.execute { onReply(reply) }
        }
    }

    companion object {
        private const val idleSeconds = 30L
        private val thread: Executor by lazy {
            ThreadPoolExecutor(0, 1, idleSeconds, TimeUnit.SECONDS, LinkedBlockingQueue()) { task ->
                Thread(task, "copypaste-runtime-access")
            }
        }

        fun native(context: Context): RuntimeAccessWorker {
            val main = Handler(Looper.getMainLooper())
            return RuntimeAccessWorker(
                JsonRuntimeAccess(NativeRuntimeTransport(context.applicationContext)),
                thread,
                Executor { main.post(it) },
            )
        }
    }
}

private class NativeRuntimeTransport(private val context: Context) : RuntimeTransport {
    override fun exchange(method: String): String {
        check(Looper.myLooper() != Looper.getMainLooper()) { "Runtime access blocks; use a worker thread." }
        MainActivity.ensureRuntime(context)
        return NativeRuntimeAccess.request(method)
    }
}
