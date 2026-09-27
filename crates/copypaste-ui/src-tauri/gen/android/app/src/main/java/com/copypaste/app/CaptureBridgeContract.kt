package com.copypaste.app

import android.app.StatusBarManager
import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.util.Base64
import androidx.core.content.FileProvider
import app.tauri.plugin.JSObject
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationStrategy
import kotlinx.serialization.json.Json

@Serializable
enum class CaptureSource {
    @SerialName("in_app")
    IN_APP,

    @SerialName("share")
    SHARE,

    @SerialName("process_text")
    PROCESS_TEXT,

    @SerialName("tile")
    TILE,

    @SerialName("background")
    BACKGROUND,
}

@Serializable
enum class ReadOutcome {
    @SerialName("succeeded")
    SUCCEEDED,

    @SerialName("empty")
    EMPTY,

    @SerialName("refused")
    REFUSED,
}

@Serializable
data class ShizukuProbe(
    val supported: Boolean,
    val installed: Boolean,
    val running: Boolean,
    val permission: Boolean,
    val enabled: Boolean,
    val toastSuppressed: Boolean,
    val rearmRequested: Boolean,
)

@Serializable
data class ProbeResult(
    val probe: ShizukuProbe,
    val enabled: Boolean,
    val listening: Boolean,
)

@Serializable
data class ArmResult(
    val probe: ShizukuProbe,
    val enabled: Boolean,
    val listening: Boolean,
    val outcome: ReadOutcome,
    val focused: Boolean,
    val notificationPermission: Boolean,
)

@Serializable
data class CaptureArmRequest(
    val ongoingText: String,
    val lostTitle: String,
    val lostBody: String,
)

@Serializable
data class NotificationPermissionFacts(
    val apiLevel: Int,
    val granted: Boolean,
    val everAsked: Boolean,
    val showRationale: Boolean,
)

@Serializable
data class SetupInstructions(
    val packageName: String,
    val shizukuCommands: List<List<String>>,
    val adbCommands: List<List<String>>,
    val requiresRestart: Boolean,
)

@Serializable
data class TileAddResultConstants(
    val notAdded: Int,
    val alreadyAdded: Int,
    val added: Int,
) {
    companion object {
        fun platform(): TileAddResultConstants = TileAddResultConstants(
            StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_NOT_ADDED,
            StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ALREADY_ADDED,
            StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ADDED,
        )
    }
}

@Serializable
data class TilePermissionFacts(
    val apiLevel: Int,
    val lastAddResult: Int?,
    val resultConstants: TileAddResultConstants,
)

@Serializable
data class ReadResult(
    val outcome: ReadOutcome,
    val clip: CapturedClip?,
    val atMs: Long,
    val focused: Boolean,
    val sourceAppBundleId: String?,
    val sourceAppName: String?,
)

@Serializable
class EmptyResult

@Serializable
data class ClipboardWriteRequest(
    val bytesBase64: String,
    val contentType: String,
    val filename: String,
)

internal fun writeBinaryClipboard(context: Context, request: ClipboardWriteRequest): Boolean {
    val maximumBase64 = ((ClipQueue.MAX_BINARY_BYTES + 2) / 3) * 4
    if (request.bytesBase64.length > maximumBase64 ||
        !Regex("^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$").matches(request.contentType)
    ) return false
    val filename = request.filename
        .substringAfterLast('/')
        .substringAfterLast('\\')
        .takeIf { it.isNotBlank() && it.length <= 255 }
        ?: return false
    val bytes = try {
        Base64.decode(request.bytesBase64, Base64.DEFAULT)
    } catch (_: IllegalArgumentException) {
        return false
    }
    if (bytes.isEmpty() || bytes.size > ClipQueue.MAX_BINARY_BYTES) return false
    return try {
        // Each clipboard URI names immutable bytes for its recipient. Reusing
        // a display filename could let a later copy replace a URI an app has
        // not opened yet.
        val file = java.io.File.createTempFile("clipboard-", ".bin", context.cacheDir)
        file.outputStream().use { it.write(bytes) }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        val clip = ClipData(
            ClipDescription(filename, arrayOf(request.contentType)),
            ClipData.Item(uri),
        )
        (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(clip)
        true
    } catch (_: java.io.IOException) {
        false
    } catch (_: IllegalArgumentException) {
        false
    }
}

@Serializable
data class CapturedClip(
    val text: String?,
    val bytesBase64: String?,
    val contentType: String?,
    val filename: String?,
    val source: CaptureSource,
    val atMs: Long,
    val sourceAppBundleId: String?,
    val sourceAppName: String?,
)

@Serializable
data class DrainResult(
    val clips: List<CapturedClip>,
    val dropped: Long,
    val stateDirty: Boolean,
    val probe: ShizukuProbe,
    val listening: Boolean,
)

object CaptureBridgeJson {
    val format = Json {
        encodeDefaults = true
        explicitNulls = false
    }

    fun <T> encode(serializer: SerializationStrategy<T>, value: T): String =
        format.encodeToString(serializer, value)

    fun <T> decode(serializer: DeserializationStrategy<T>, value: JSObject): T =
        format.decodeFromString(serializer, value.toString())

    fun <T> objectOf(serializer: SerializationStrategy<T>, value: T): JSObject =
        JSObject(encode(serializer, value))
}
