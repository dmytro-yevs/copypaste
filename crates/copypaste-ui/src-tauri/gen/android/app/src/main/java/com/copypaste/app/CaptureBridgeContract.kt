package com.copypaste.app

import app.tauri.plugin.JSObject
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
data class ReadResult(
    val outcome: ReadOutcome,
    val text: String?,
    val atMs: Long,
    val focused: Boolean,
)

@Serializable
class EmptyResult

@Serializable
data class CapturedClip(
    val text: String,
    val source: CaptureSource,
    val atMs: Long,
    val sourceAppBundleId: String?,
    val sourceAppName: String?,
)

@Serializable
data class DrainResult(
    val clips: List<CapturedClip>,
    val dropped: Long,
    val probe: ShizukuProbe,
)

object CaptureBridgeJson {
    val format = Json {
        encodeDefaults = true
        explicitNulls = false
    }

    fun <T> encode(serializer: SerializationStrategy<T>, value: T): String =
        format.encodeToString(serializer, value)

    fun <T> objectOf(serializer: SerializationStrategy<T>, value: T): JSObject =
        JSObject(encode(serializer, value))
}
