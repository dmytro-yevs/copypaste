package com.copypaste.app.quickaccess

import com.copypaste.app.R
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

/** A parameter the sheet can change with a tap: a choice or an on/off value. */
internal data class PaletteParam(
    val key: String,
    val label: String,
    val kind: String,
    val defaultJson: String,
    val options: List<Pair<String, String>>,
) {
    /** The JSON value after [current], wrapping around. */
    fun next(currentJson: String): String = when (kind) {
        KIND_BOOL -> (currentJson != "true").toString()
        else -> {
            val values = options.map { JSONObject.quote(it.first) }
            values.getOrNull((values.indexOf(currentJson) + 1) % values.size.coerceAtLeast(1)) ?: defaultJson
        }
    }

    fun labelOf(valueJson: String): String = when (kind) {
        KIND_BOOL -> label
        else -> options.firstOrNull { JSONObject.quote(it.first) == valueJson }?.second ?: label
    }

    companion object {
        const val KIND_BOOL = "bool"
        const val KIND_CHOICE = "choice"
    }
}

/** One runnable row: a built-in transform or a saved pipeline. */
internal data class PaletteEntry(
    val id: String,
    val pipeline: Boolean,
    val name: String,
    val description: String,
    val version: Int = 0,
    val params: List<PaletteParam> = emptyList(),
    /** Stored parameter defaults merged over the catalogue defaults. */
    val preset: JSONObject = JSONObject(),
    /** A pipeline's steps exactly as stored. */
    val steps: JSONArray? = null,
) {
    /** `TransformStep` list for a preview or run with the chip values in [chosen]. */
    fun steps(chosen: Map<String, String>): JSONArray {
        if (pipeline) return steps ?: JSONArray()
        val values = JSONObject(preset.toString())
        for (param in params) {
            val json = chosen[param.key] ?: valueOf(param)
            runCatching { JSONTokener(json).nextValue() }.getOrNull()?.let { values.put(param.key, it) }
        }
        return JSONArray().put(
            JSONObject().put("transform_id", id).put("version", version)
                .put("params_json", values.toString()).put("enabled", true),
        )
    }

    /** `TransformSteps` for a run: pipelines run by id so provenance names them. */
    fun runSteps(chosen: Map<String, String>): JSONObject =
        if (pipeline) JSONObject().put("kind", "pipeline").put("id", id)
        else JSONObject().put("kind", "steps").put("steps", steps(chosen))

    /** The JSON text of [param]'s starting value: the stored default, else the catalogue's. */
    fun valueOf(param: PaletteParam): String {
        val value = preset.opt(param.key)
        return when {
            value == null || value == JSONObject.NULL -> param.defaultJson
            value is String -> JSONObject.quote(value)
            else -> value.toString()
        }
    }
}

internal data class PaletteSection(val title: Int?, val entries: List<PaletteEntry>)

/**
 * Builds the palette from the runtime's catalogue, preferences and pipelines:
 * favorites first, then pipelines, then the transforms by category. Hidden
 * transforms are left out, and transforms that need free text run only
 * through saved pipelines, because the sheet offers no text input.
 */
internal object TransformPalette {
    fun build(catalogue: JSONArray, preferences: JSONObject?, pipelines: JSONArray?): List<PaletteSection> {
        val hidden = preferences?.optJSONArray("hidden_transforms").strings().toSet()
        val presets = preferences?.optJSONArray("param_defaults").objects()
            .associate { it.optString("transform_id") to parseObject(it.optString("params_json")) }
        val transforms = catalogue.objects().filter { info ->
            info.optString("id") !in hidden &&
                info.optJSONObject("flags")?.optBoolean("needs_free_text", false) != true
        }.map { transform(it, presets[it.optString("id")]) }
        val saved = pipelines.objects().filter { it.isNull("deleted_at") || !it.has("deleted_at") }.map(::pipeline)
        val favorites = preferences?.optJSONArray("favorites").objects().mapNotNull { favorite ->
            val id = favorite.optString("id")
            if (favorite.optString("kind") == "pipeline") saved.firstOrNull { it.id == id }
            else transforms.firstOrNull { it.id == id }
        }
        val categories = linkedMapOf<String, MutableList<PaletteEntry>>()
        catalogue.objects().forEach { info ->
            val entry = transforms.firstOrNull { it.id == info.optString("id") } ?: return@forEach
            categories.getOrPut(info.optString("category")) { mutableListOf() } += entry
        }
        return buildList {
            if (favorites.isNotEmpty()) add(PaletteSection(R.string.quick_access_transform_favorites, favorites))
            if (saved.isNotEmpty()) add(PaletteSection(R.string.quick_access_transform_pipelines, saved))
            categories.forEach { (key, entries) -> add(PaletteSection(categoryTitle(key), entries)) }
        }
    }

    fun categoryTitle(key: String): Int? = when (key) {
        "structure" -> R.string.quick_access_transform_category_structure
        "encode" -> R.string.quick_access_transform_category_encode
        "case" -> R.string.quick_access_transform_category_case
        "hash" -> R.string.quick_access_transform_category_hash
        "lines" -> R.string.quick_access_transform_category_lines
        "clean" -> R.string.quick_access_transform_category_clean
        "url" -> R.string.quick_access_transform_category_url
        "markup" -> R.string.quick_access_transform_category_markup
        "dev" -> R.string.quick_access_transform_category_dev
        else -> null
    }

    private fun transform(info: JSONObject, preset: JSONObject?): PaletteEntry = PaletteEntry(
        id = info.optString("id"),
        pipeline = false,
        name = info.optString("name"),
        description = info.optString("description"),
        version = info.optInt("version"),
        params = info.optJSONArray("params").objects().mapNotNull(::param),
        preset = preset ?: JSONObject(),
    )

    private fun pipeline(record: JSONObject): PaletteEntry = PaletteEntry(
        id = record.optString("id"),
        pipeline = true,
        name = record.optString("name"),
        description = record.optString("description").takeUnless { record.isNull("description") }.orEmpty(),
        steps = record.optJSONArray("steps") ?: JSONArray(),
    )

    private fun param(spec: JSONObject): PaletteParam? {
        val kind = spec.optString("kind")
        val options = spec.optJSONArray("options").objects().map { it.optString("value") to it.optString("label") }
        if (kind != PaletteParam.KIND_BOOL && (kind != PaletteParam.KIND_CHOICE || options.size < 2)) return null
        return PaletteParam(spec.optString("key"), spec.optString("label"), kind, spec.optString("default_json"), options)
    }

    private fun parseObject(raw: String): JSONObject = runCatching { JSONObject(raw) }.getOrDefault(JSONObject())

    private fun JSONArray?.objects(): List<JSONObject> =
        if (this == null) emptyList() else (0 until length()).mapNotNull(::optJSONObject)

    private fun JSONArray?.strings(): List<String> =
        if (this == null) emptyList() else (0 until length()).map(::optString)
}

/** What the sheet may do with a result. */
internal enum class OutputAction(val label: Int) {
    REPLACE(R.string.quick_access_transform_replace),
    COPY(R.string.quick_access_transform_copy),
    SAVE(R.string.quick_access_transform_save),
    ;

    companion object {
        /**
         * Replace exists only for an editable selection. A sensitive source is
         * never saved as an ordinary clip; Copy marks the clipboard sensitive.
         */
        fun available(editable: Boolean, sensitive: Boolean): List<OutputAction> = buildList {
            if (editable) add(REPLACE)
            add(COPY)
            if (!sensitive) add(SAVE)
        }
    }
}
