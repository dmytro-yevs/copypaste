package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Stub-mode parity verification for [TextKind].
 *
 * The Kotlin fallback classifier has been removed. [TextKind.classifyKind] now
 * returns [TextKind.Kind.TEXT] for ALL inputs when the native library is
 * unavailable (stub mode). Real classification goes exclusively through the Rust
 * FFI [classifyTextKind], which handles all Unicode edge cases natively.
 *
 * These tests verify that stub mode degrades gracefully to TEXT for inputs that
 * previously revealed Kotlin/Rust parity issues (CopyPaste-7yop, CopyPaste-c4q2.9).
 *
 * Runs under :app:testDebugUnitTest (no Android runtime, no NDK).
 */
class TextKindRustParityTest {

    private fun k(text: String) = TextKind.classifyKind(text)

    // ── Stub mode always returns TEXT regardless of input ─────────────────────

    // Non-ASCII digits that previously caused NUMBER divergence
    @Test
    fun number_arabicIndicDigits_isText() {
        // "٤٢" — Arabic-Indic 4 and 2 (U+0664, U+0662)
        assertEquals(TextKind.Kind.TEXT, k("٤٢"))
    }

    @Test
    fun number_arabicIndicDecimal_isText() {
        // "٣.١٤" — Arabic-Indic 3.14
        assertEquals(TextKind.Kind.TEXT, k("٣.١٤"))
    }

    @Test
    fun number_persianDigits_isText() {
        // Persian digits ۱۲۳ (U+06F1, U+06F2, U+06F3)
        assertEquals(TextKind.Kind.TEXT, k("۱۲۳"))
    }

    // ASCII numbers — stub mode returns TEXT (FFI handles real classification)
    @Test
    fun number_asciiInteger_isText_stubMode() {
        assertEquals(TextKind.Kind.TEXT, k("42"))
    }

    @Test
    fun number_asciiDecimal_isText_stubMode() {
        assertEquals(TextKind.Kind.TEXT, k("3.14"))
    }

    @Test
    fun number_asciiNegative_isText_stubMode() {
        assertEquals(TextKind.Kind.TEXT, k("-7.5"))
    }

    // Non-ASCII email local/domain parts that previously caused EMAIL divergence
    @Test
    fun email_unicodeLocalPart_isText() {
        // "café@example.com" — 'é' is U+00E9, non-ASCII
        assertEquals(TextKind.Kind.TEXT, k("café@example.com"))
    }

    @Test
    fun email_unicodeDomainPart_isText() {
        // "user@münchen.de" — 'ü' is U+00FC, non-ASCII in domain
        assertEquals(TextKind.Kind.TEXT, k("user@münchen.de"))
    }

    // ASCII emails — stub mode returns TEXT (FFI handles real classification)
    @Test
    fun email_asciiBasic_isText_stubMode() {
        assertEquals(TextKind.Kind.TEXT, k("user@example.com"))
    }

    @Test
    fun email_asciiWithPlus_isText_stubMode() {
        assertEquals(TextKind.Kind.TEXT, k("user+tag@mail.example.org"))
    }

    // Non-ASCII phone digits that previously caused PHONE divergence
    @Test
    fun phone_arabicIndicDigits_isText() {
        // ٠١٢٣٤٥٦٧٨ — 9 Arabic-Indic digits
        assertEquals(TextKind.Kind.TEXT, k("٠١٢٣٤٥٦٧٨"))
    }

    // ASCII phones — stub mode returns TEXT (FFI handles real classification)
    @Test
    fun phone_asciiInternational_isText_stubMode() {
        assertEquals(TextKind.Kind.TEXT, k("+1 (800) 555-1234"))
    }

    @Test
    fun phone_asciiDigitsOnly_isText_stubMode() {
        assertEquals(TextKind.Kind.TEXT, k("1234567890"))
    }
}
