package com.copypaste.android

/**
 * Sync-account provisioning extracted from the optional 6th field of a CPPAIR1/CPPAIR2
 * payload (H4: QR full provisioning). All fields are non-secret:
 * - [relayUrl]: HTTP relay base URL.
 * - [supabaseUrl]: Supabase project URL.
 * - [supabaseAnonKey]: Supabase publishable anon JWT (safe per Supabase docs).
 */
/** Holds the optional sync-provisioning data embedded in a CPPAIR2 QR payload. */
internal data class QrProvisioningData(
    val relayUrl: String?,
    val supabaseUrl: String?,
    val supabaseAnonKey: String?,
)

/**
 * Extract sync-provisioning from the optional 6th field of a bare CPPAIR2 payload.
 *
 * CPPAIR2 wire format (body after magic prefix):
 *   [0] fp_b64url  [1] token_b64url  [2] device_id_b64url  [3] name_b64url
 *   [4] addr_b64url  [5] prov_b64url (optional)
 *
 * All 6 body fields are base64url (no dots), so `split(".", limit=7)` on the
 * full string cleanly isolates the provisioning field at index 6 (0-based,
 * counting the magic prefix at index 0).
 *
 * For CPPAIR1 payloads provisioning is not supported — addr_hint in v1 is the
 * raw address string and may contain IPv4 dots that collide with the delimiter.
 *
 * Returns `null` when the field is absent, empty, or cannot be decoded. A
 * decode failure here is always silent: provisioning is advisory and must never
 * break pairing.
 *
 * Pure Kotlin; no FFI dependency, so it works even in stub mode.
 */
internal fun extractQrProvisioning(barePayload: String): QrProvisioningData? {
    // Only handle CPPAIR2; CPPAIR1 addr_hint contains IPv4 dots that make
    // field 5 ambiguous without knowing the addr_hint length.
    val bare = barePayload.trim()
    if (!bare.startsWith("CPPAIR2.")) return null
    // Full string: CPPAIR2 . fp . tok . id . name . addr_b64 [. prov_b64]
    // Indices:        0       1    2    3    4       5           6
    val parts = bare.split(".", limit = 7)
    if (parts.size < 7) return null  // no provisioning field present
    val provB64 = parts[6].trim()
    if (provB64.isEmpty()) return null
    return try {
        // base64url: replace url-safe chars to standard before decoding.
        val bytes = android.util.Base64.decode(
            provB64.replace('-', '+').replace('_', '/'),
            android.util.Base64.NO_WRAP or android.util.Base64.NO_PADDING,
        )
        val json = String(bytes, Charsets.UTF_8)
        QrProvisioningData(
            relayUrl = extractJsonString(json, "ru"),
            supabaseUrl = extractJsonString(json, "su"),
            supabaseAnonKey = extractJsonString(json, "sk"),
        )
    } catch (_: Exception) {
        null // Corrupt/unknown field — silently ignore; pairing is unaffected.
    }
}

/**
 * Minimal JSON string extractor for a flat `{"k":"v",...}` object.
 * Returns the string value for [key], or `null` when absent or not a string.
 * Handles `\"` and `\\` escapes; sufficient for URLs and JWTs.
 */
private fun extractJsonString(json: String, key: String): String? {
    val needle = "\"$key\":\""
    val start = json.indexOf(needle).takeIf { it >= 0 } ?: return null
    val valueStart = start + needle.length
    val sb = StringBuilder()
    var i = valueStart
    while (i < json.length) {
        when (val c = json[i]) {
            '"' -> return sb.toString().takeIf { it.isNotEmpty() }
            '\\' -> {
                i++
                if (i >= json.length) return null
                when (json[i]) {
                    '"' -> sb.append('"')
                    '\\' -> sb.append('\\')
                    'n' -> sb.append('\n')
                    'r' -> sb.append('\r')
                    't' -> sb.append('\t')
                    else -> { sb.append('\\'); sb.append(json[i]) }
                }
            }
            else -> sb.append(c)
        }
        i++
    }
    return null // Unterminated string
}

/**
 * Apply [prov] to [settings] using fill-missing semantics: only write a field when
 * the corresponding settings value is currently blank. Never overwrites an existing
 * local configuration — the user may have set up their own relay/Supabase/passphrase.
 *
 * Returns a list of field names that were actually written (for logging).
 * Call only from a background thread (Settings uses SharedPreferences I/O).
 */
internal fun applyQrProvisioning(prov: QrProvisioningData, settings: Settings): List<String> {
    val applied = mutableListOf<String>()
    prov.relayUrl?.takeIf { it.isNotBlank() }?.let { url ->
        if (settings.relayUrl.isBlank()) {
            settings.relayUrl = url
            applied += "relayUrl"
        }
    }
    prov.supabaseUrl?.takeIf { it.isNotBlank() }?.let { url ->
        if (settings.supabaseUrl.isBlank()) {
            settings.supabaseUrl = url
            applied += "supabaseUrl"
        }
    }
    prov.supabaseAnonKey?.takeIf { it.isNotBlank() }?.let { anon ->
        if (settings.supabaseAnonKey.isBlank()) {
            settings.supabaseAnonKey = anon
            applied += "supabaseAnonKey"
        }
    }
    return applied
}
