package com.copypaste.app

import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject

/**
 * A module's Quick Paste surface (module contract v7): on Android a keyboard
 * panel tab and a widget filter, both labelled [title].
 */
internal data class QuickPasteSurface(
    val moduleId: String,
    val title: String,
    /** Takes `query` and returns `results`; an empty query lists the tab. */
    val searchCommand: String,
    /** Takes `id` and returns `deliver`, or a `view` whose form submit does. */
    val activateCommand: String,
)

/** One `ResultItem` of a Quick Paste surface. */
internal data class ModuleResult(
    val id: String,
    val title: String,
    val preview: String,
    val badges: List<String>,
    /** The preview is masked. */
    val secret: Boolean,
    val preselect: Boolean,
    val actions: List<ModuleAction>,
)

/** A `ViewAction`: a command of the same module with its arguments. */
internal data class ModuleAction(
    val title: String,
    val command: String,
    val arguments: JSONObject,
    val destructive: Boolean,
    val requireAuth: Boolean,
)

/** A `Delivery`: text the host inserts, copies or queues. */
internal data class ModuleDelivery(
    val text: String,
    /** Written as a secret and never recorded. */
    val secret: Boolean,
    val requireAuth: Boolean,
    val recordHistory: Boolean,
    val label: String,
    /** Where the caret goes after insertion, in graphemes from the end. */
    val cursorGraphemesFromEnd: Int,
)

/** The `clips deliver_text` op for [delivery], recorded with [moduleId] when the module asked. */
internal fun deliverTextOp(delivery: ModuleDelivery, moduleId: String, mode: String): JSONObject =
    com.copypaste.app.quickaccess.op("deliver_text") {
        put("text", delivery.text)
        put("secret", delivery.secret)
        put("delivery", mode)
        if (delivery.recordHistory && !delivery.secret) {
            put("record", JSONObject().put("module_id", moduleId).put("label", delivery.label))
        }
    }

/** One input of a view form (`ModuleField`). */
internal data class ModuleFormField(
    val id: String,
    val title: String,
    val kind: Kind,
    val required: Boolean,
    val secret: Boolean,
    /** A String, Boolean or List<String>, by [kind]. */
    val default: Any,
    val options: List<Pair<String, String>>,
) {
    enum class Kind { TEXT, MULTILINE, CHOICES, BOOLEAN, UNSUPPORTED }

    fun isMissing(value: Any?): Boolean = required && when (kind) {
        Kind.TEXT, Kind.MULTILINE -> (value as? String).isNullOrBlank()
        Kind.CHOICES -> (value as? List<*>).isNullOrEmpty()
        else -> false
    }
}

/** A view `form` item: [submit] runs with the field values as `values`. */
internal data class ModuleForm(
    val title: String?,
    val fields: List<ModuleFormField>,
    val values: Map<String, Any>,
    val errors: Map<String, String>,
    val submit: ModuleAction,
    val preview: ModuleAction?,
) {
    /** File fields need the app's file picker. */
    val supported: Boolean get() = fields.none { it.kind == ModuleFormField.Kind.UNSUPPORTED }

    fun initialValues(): LinkedHashMap<String, Any> =
        fields.associateTo(LinkedHashMap()) { field -> field.id to (values[field.id] ?: field.default) }
}

/** What one module command returned, as the native surfaces use it. */
internal sealed interface ModuleOutput {
    data class Results(val items: List<ModuleResult>) : ModuleOutput

    data class Deliver(val delivery: ModuleDelivery) : ModuleOutput

    /** A view; [form] is its first form, the only view item the native surfaces render. */
    data class View(val form: ModuleForm?) : ModuleOutput

    /** A `text` or `message` output. */
    data class Message(val text: String) : ModuleOutput

    /** An output kind the native surfaces do not show (`data`, `save_file`, …). */
    data object Other : ModuleOutput

    data class Failed(val reply: RuntimeReply) : ModuleOutput
}

/**
 * The `modules` runtime method as the keyboard panel and widget use it:
 * `list` for the Quick Paste surfaces, `invoke` for their commands.
 */
internal object ModuleSurfaces {
    const val METHOD = "modules"
    private const val MAX_RESULTS = 200
    private const val MAX_ACTIONS = 4
    private const val MAX_BADGES = 4

    fun listParams(): JSONObject = JSONObject().put("operation", JSONObject().put("operation", "list"))

    fun invokeParams(moduleId: String, command: String, arguments: JSONObject): JSONObject = JSONObject().put(
        "operation",
        JSONObject()
            .put("operation", "invoke")
            .put("id", moduleId)
            .put("command", command)
            .put("arguments_json", arguments.toString()),
    )

    /** Surfaces of installed modules that are enabled and loaded; null when the list could not be read. */
    fun quickPaste(access: RuntimeAccess): List<QuickPasteSurface>? = quickPaste(access.request(METHOD, listParams()))

    fun invoke(access: RuntimeAccess, moduleId: String, command: String, arguments: JSONObject): ModuleOutput =
        output(access.request(METHOD, invokeParams(moduleId, command, arguments)))

    fun quickPaste(reply: RuntimeReply): List<QuickPasteSurface>? {
        val modules = payload(reply)?.let { runCatching { JSONArray(it) }.getOrNull() } ?: return null
        return modules.objects().mapNotNull { module ->
            if (!module.optBoolean("enabled") || module.nullable("error") != null) return@mapNotNull null
            val surface = module.optJSONObject("surfaces")?.optJSONObject("quick_paste") ?: return@mapNotNull null
            QuickPasteSurface(
                moduleId = module.nullable("id") ?: return@mapNotNull null,
                title = surface.nullable("title") ?: return@mapNotNull null,
                searchCommand = surface.nullable("search_command") ?: return@mapNotNull null,
                activateCommand = surface.nullable("activate_command") ?: return@mapNotNull null,
            )
        }
    }

    fun output(reply: RuntimeReply): ModuleOutput {
        val output = payload(reply)?.let {
            try {
                JSONObject(it)
            } catch (_: JSONException) {
                null
            }
        } ?: return ModuleOutput.Failed(reply)
        return when (output.optString("kind")) {
            "results" -> ModuleOutput.Results(output.optJSONArray("items").objects().take(MAX_RESULTS).mapNotNull(::result))
            "deliver" -> output.optJSONObject("delivery")?.let { delivery ->
                ModuleOutput.Deliver(
                    ModuleDelivery(
                        text = delivery.optString("text"),
                        secret = delivery.optBoolean("secret"),
                        requireAuth = delivery.optBoolean("require_auth"),
                        recordHistory = delivery.optBoolean("record_history"),
                        label = delivery.optString("label"),
                        cursorGraphemesFromEnd = delivery.optInt("cursor_graphemes_from_end", 0),
                    ),
                )
            } ?: ModuleOutput.Other
            "view" -> ModuleOutput.View(output.optJSONObject("view")?.let(::firstForm))
            "text" -> ModuleOutput.Message(output.optString("text"))
            "message" -> ModuleOutput.Message(output.optString("message"))
            else -> ModuleOutput.Other
        }
    }

    /** The `json` string of a `modules` reply. */
    private fun payload(reply: RuntimeReply): String? =
        (reply as? RuntimeReply.Ok)?.takeIf { it.kind == METHOD }?.objectData()?.nullable("json")

    private fun result(item: JSONObject): ModuleResult? = ModuleResult(
        id = item.nullable("id") ?: return null,
        title = item.optString("title"),
        preview = item.optString("preview"),
        badges = item.optJSONArray("badges").strings().take(MAX_BADGES),
        secret = item.optBoolean("secret"),
        preselect = item.optBoolean("preselect"),
        actions = item.optJSONArray("actions").objects().take(MAX_ACTIONS).mapNotNull(::action),
    )

    private fun action(action: JSONObject): ModuleAction? = ModuleAction(
        title = action.optString("title"),
        command = action.nullable("command") ?: return null,
        arguments = action.optJSONObject("arguments") ?: JSONObject(),
        destructive = action.optBoolean("destructive"),
        requireAuth = action.optBoolean("require_auth"),
    )

    private fun firstForm(view: JSONObject): ModuleForm? {
        for (section in view.optJSONArray("sections").objects()) {
            val form = section.optJSONArray("items").objects().firstOrNull { it.optString("kind") == "form" } ?: continue
            val fields = form.optJSONArray("fields").objects().mapNotNull(::field)
            val values = form.optJSONObject("values")
            val errors = form.optJSONObject("errors")
            return ModuleForm(
                title = section.nullable("title"),
                fields = fields,
                values = values?.let { stored ->
                    stored.keys().asSequence().mapNotNull { key -> value(stored.opt(key))?.let { key to it } }.toMap()
                }.orEmpty(),
                errors = errors?.let { messages -> messages.keys().asSequence().associateWith { messages.optString(it) } }.orEmpty(),
                submit = form.optJSONObject("submit")?.let(::action) ?: return null,
                preview = form.optJSONObject("preview")?.let(::action),
            )
        }
        return null
    }

    private fun field(field: JSONObject): ModuleFormField? {
        val id = field.nullable("id") ?: return null
        val kind = when (field.optString("kind")) {
            "text" -> if (field.optBoolean("multiline")) ModuleFormField.Kind.MULTILINE else ModuleFormField.Kind.TEXT
            "choices" -> ModuleFormField.Kind.CHOICES
            "boolean" -> ModuleFormField.Kind.BOOLEAN
            else -> ModuleFormField.Kind.UNSUPPORTED
        }
        return ModuleFormField(
            id = id,
            title = field.optString("title").ifEmpty { id },
            kind = kind,
            required = field.optBoolean("required"),
            secret = field.optBoolean("secret"),
            default = when (kind) {
                ModuleFormField.Kind.CHOICES -> field.optJSONArray("default").strings()
                ModuleFormField.Kind.BOOLEAN -> field.optBoolean("default")
                else -> field.optString("default")
            },
            options = field.optJSONArray("options").objects().mapNotNull { option ->
                option.nullable("id")?.let { it to option.optString("title").ifEmpty { it } }
            },
        )
    }

    /** A stored form value: a string, a boolean or a list of strings. */
    private fun value(raw: Any?): Any? = when (raw) {
        is String, is Boolean -> raw
        is JSONArray -> raw.strings()
        else -> null
    }

    /** The `values` argument of a form submit or preview. */
    fun valuesJson(values: Map<String, Any>): JSONObject = JSONObject().apply {
        values.forEach { (key, value) -> put(key, if (value is List<*>) JSONArray(value) else value) }
    }

    private fun JSONObject.nullable(key: String): String? =
        if (has(key) && !isNull(key)) optString(key).ifEmpty { null } else null

    private fun JSONArray?.objects(): List<JSONObject> =
        if (this == null) emptyList() else (0 until length()).mapNotNull(::optJSONObject)

    private fun JSONArray?.strings(): List<String> =
        if (this == null) emptyList() else (0 until length()).mapNotNull { index -> opt(index) as? String }
}

/** Runs one module command on the worker thread; [onOutput] gets its parsed output on the delivery thread. */
internal fun RuntimeAccessWorker.invokeModule(
    moduleId: String,
    command: String,
    arguments: JSONObject,
    onOutput: (ModuleOutput) -> Unit,
) = perform({ access -> ModuleSurfaces.invoke(access, moduleId, command, arguments) }, onOutput)
