package com.copypaste.android

/**
 * Text kind classification — delegates to the canonical Rust FFI
 * [classifyTextKind] (PG-16 / 89ve) so Android and macOS use the SAME
 * classifier and cannot drift.
 *
 * The [Kind] enum and the private helpers below are kept as a Kotlin-side
 * fallback: when the native library is unavailable (stub mode) [classify] and
 * [classifyKind] fall back to the pure-Kotlin path so the UI still labels clips
 * correctly in test/emulator environments.
 *
 * This is a PURE presentation-layer hint derived from decrypted text. It does
 * NOT change the stored content_type ("text"/"image"/"file"). Priority order
 * mirrors the Rust implementation exactly: URL > EMAIL > COLOR > PHONE >
 * NUMBER > JSON > PATH > CODE > TEXT.
 *
 * Callers must not depend on the ordinal values of [Kind] — only the [label].
 */
object TextKind {

    enum class Kind(val label: String) {
        TEXT("TEXT"),
        URL("URL"),
        EMAIL("EMAIL"),
        PHONE("PHONE"),
        COLOR("COLOR"),
        JSON("JSON"),
        CODE("CODE"),
        NUMBER("NUMBER"),
        PATH("PATH"),
    }

    /**
     * Classify [text] and return its [Kind.label] string (e.g. "URL", "EMAIL").
     * Delegates to the canonical Rust FFI [classifyTextKind] when the native
     * library is loaded; falls back to the pure-Kotlin [classifyKind] otherwise.
     * Always returns a non-null, non-blank uppercase label.
     */
    fun classify(text: String): String {
        // PG-16 (89ve): delegate to Rust FFI for parity with macOS classifier.
        // classifyTextKind() returns "TEXT" in stub mode, so the fallback below
        // is only reached when the returned label is blank (should not happen).
        if (isNativeLibraryLoaded) {
            val label = classifyTextKind(text)
            if (label.isNotBlank()) return label
        }
        return classifyKind(text).label
    }

    /** Returns the [Kind] enum value for the given text (for testing / stub fallback). */
    internal fun classifyKind(text: String): Kind {
        val trimmed = text.trim()
        if (trimmed.isEmpty()) return Kind.TEXT
        if (isUrl(trimmed)) return Kind.URL
        if (isEmail(trimmed)) return Kind.EMAIL
        if (isColorHex(trimmed)) return Kind.COLOR
        if (isPhone(trimmed)) return Kind.PHONE
        if (isNumber(trimmed)) return Kind.NUMBER
        if (isJson(trimmed)) return Kind.JSON
        if (isFilePath(trimmed)) return Kind.PATH
        if (isCode(trimmed)) return Kind.CODE
        return Kind.TEXT
    }

    // ── URL ──────────────────────────────────────────────────────────────────

    private fun isUrl(s: String): Boolean {
        // No internal whitespace allowed
        if (s.any { it.isWhitespace() }) return false
        val lower = s.lowercase()
        // mailto: is treated as Email, not URL
        if (lower.startsWith("mailto:")) return false
        return lower.startsWith("http://") ||
            lower.startsWith("https://") ||
            lower.startsWith("ftp://")
    }

    // ── EMAIL ─────────────────────────────────────────────────────────────────

    private fun isEmail(s: String): Boolean {
        // Single line, no whitespace
        if (s.any { it.isWhitespace() }) return false
        val lower = s.lowercase()
        // Handle mailto: prefix
        val addr = if (lower.startsWith("mailto:")) lower.removePrefix("mailto:") else lower

        // Exactly one '@'
        if (addr.count { it == '@' } != 1) return false
        val atIdx = addr.indexOf('@')
        val local = addr.substring(0, atIdx)
        val domain = addr.substring(atIdx + 1)

        if (local.isEmpty() || domain.isEmpty()) return false

        // Domain must contain a '.' with non-empty TLD
        val dotPos = domain.lastIndexOf('.')
        if (dotPos < 0) return false
        val tld = domain.substring(dotPos + 1)
        if (tld.isEmpty() || dotPos == 0) return false

        // Validate character sets using ASCII-only predicates.
        // Mirrors Rust is_ascii_alphanumeric() — rejects non-ASCII letters/digits
        // (e.g. accented chars like 'é', 'ü') so the Kotlin fallback and the Rust
        // FFI agree on all inputs (CopyPaste-7yop).
        val validLocal = local.all { c ->
            c in 'a'..'z' || c in 'A'..'Z' || c in '0'..'9' ||
                c == '.' || c == '_' || c == '+' || c == '-'
        }
        val validDomain = domain.all { c ->
            c in 'a'..'z' || c in 'A'..'Z' || c in '0'..'9' ||
                c == '.' || c == '-'
        }
        return validLocal && validDomain
    }

    // ── COLOR HEX ─────────────────────────────────────────────────────────────

    private fun isColorHex(s: String): Boolean {
        if (!s.startsWith('#')) return false
        val hex = s.substring(1)
        val len = hex.length
        if (len !in setOf(3, 4, 6, 8)) return false
        return hex.all { c -> c in '0'..'9' || c in 'a'..'f' || c in 'A'..'F' }
    }

    // ── PHONE ─────────────────────────────────────────────────────────────────

    private fun isPhone(s: String): Boolean {
        val rest = if (s.startsWith('+')) s.substring(1) else s
        if (rest.isEmpty()) return false
        // All chars must be ASCII digits, spaces, dashes, or parens.
        // Mirrors Rust is_ascii_digit() — rejects non-ASCII digits (Arabic-Indic, etc.)
        // so the Kotlin fallback and the Rust FFI agree on all inputs (CopyPaste-7yop).
        if (!rest.all { c -> c in '0'..'9' || c == ' ' || c == '-' || c == '(' || c == ')' }) return false
        // Must have at least 7 digits
        return s.count { it in '0'..'9' } >= 7
    }

    // ── NUMBER ────────────────────────────────────────────────────────────────

    private fun isNumber(s: String): Boolean {
        val rest = when {
            s.startsWith('-') -> s.substring(1)
            s.startsWith('+') -> s.substring(1)
            else -> s
        }
        if (rest.isEmpty()) return false
        // Remove thousands separators (commas)
        val withoutSep = rest.replace(",", "")
        if (withoutSep.isEmpty()) return false
        // At most one decimal point
        if (withoutSep.count { it == '.' } > 1) return false
        // All remaining chars must be ASCII digits or '.'.
        // Mirrors Rust is_ascii_digit() — rejects non-ASCII digits (Arabic-Indic, etc.)
        // so the Kotlin fallback and the Rust FFI agree on all inputs (CopyPaste-7yop).
        if (!withoutSep.all { c -> c in '0'..'9' || c == '.' }) return false
        // Must start and end with a digit
        return withoutSep.first() in '0'..'9' && withoutSep.last() in '0'..'9'
    }

    // ── JSON ──────────────────────────────────────────────────────────────────

    private fun isJson(s: String): Boolean {
        val startsObj = s.startsWith('{') && s.endsWith('}')
        val startsArr = s.startsWith('[') && s.endsWith(']')
        if (!startsObj && !startsArr) return false
        return try {
            org.json.JSONTokener(s).nextValue()
            true
        } catch (_: org.json.JSONException) {
            false
        }
    }

    // ── FILE PATH ─────────────────────────────────────────────────────────────

    private fun isFilePath(s: String): Boolean {
        // Single line only
        if ('\n' in s || '\r' in s) return false
        if (s.length <= 1) return false
        val isUnix = s.startsWith('/') || s.startsWith("~/")
        val isWindows = s.length >= 3 &&
            s[0].isLetter() &&
            s.substring(1).startsWith(":\\")
        if (!isUnix && !isWindows) return false
        // Must contain a separator (guaranteed by prefix checks, but be explicit)
        return '/' in s || '\\' in s
    }

    // ── CODE ─────────────────────────────────────────────────────────────────

    private fun isCode(s: String): Boolean {
        val isMultiline = '\n' in s
        val codeSignals = listOf(";", "{", "}", "=>", "fn ", "def ", "function ",
            "import ", "class ", "#include", "</")
        val hasSignal = codeSignals.any { sig -> s.contains(sig) }

        if (isMultiline && hasSignal) return true

        // Single-line with strong code indicators only
        if (!isMultiline && (s.contains("=>") || (s.contains(';') && s.contains('{')))) return true

        return false
    }
}
