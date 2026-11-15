package com.copypaste.app

import org.json.JSONObject

/** How a clip may be presented on a native surface. */
internal enum class ClipPresentation {
    /** Content may be shown. */
    Full,

    /** Only the type may be shown, never the content. */
    Masked,

    /** The clip must not be listed: it expired and awaits its purge. */
    Hidden,
}

/**
 * The Kotlin twin of the Dart `ClipVisibility`: whether a clip is secret or
 * expired, from its privacy flag and runtime annotations. Every native surface
 * (widget, keyboard panel) uses it so the rules stay identical everywhere.
 */
internal data class ClipVisibility(
    /** The authenticated secret flag captured with the clip. */
    val secret: Boolean,
    /** A local display mask applied by a content rule. */
    val ruleSecret: Boolean = false,
    val expiresAtMs: Long? = null,
) {
    val masked: Boolean get() = secret || ruleSecret

    fun isExpired(nowMs: Long): Boolean = expiresAtMs != null && expiresAtMs <= nowMs

    /** [maskedMode] masks every clip, for example in a password field. */
    fun presentationAt(nowMs: Long, maskedMode: Boolean = false): ClipPresentation = when {
        isExpired(nowMs) -> ClipPresentation.Hidden
        masked || maskedMode -> ClipPresentation.Masked
        else -> ClipPresentation.Full
    }

    companion object {
        /** Reads an IPC `Item` object. */
        fun of(item: JSONObject): ClipVisibility {
            val annotations = item.optJSONObject("annotations")
            return ClipVisibility(
                secret = item.optJSONObject("privacy")?.optBoolean("secret") == true,
                ruleSecret = annotations?.optBoolean("rule_secret") == true,
                expiresAtMs = annotations
                    ?.takeUnless { it.isNull("expires_at") }
                    ?.optLong("expires_at"),
            )
        }
    }
}
