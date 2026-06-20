package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Tests for CopyPaste-7yop — Kotlin TextKind fallback parity with Rust.
 *
 * The Kotlin fallback in [TextKind.classifyKind] used [Char.isDigit] and
 * [Char.isLetterOrDigit] where the Rust implementation uses `is_ascii_digit`
 * and `is_ascii_alphanumeric`. These Kotlin predicates match UNICODE digits/letters
 * (e.g. Arabic-Indic digits ٠١٢…٩, CJK numerals), causing the Kotlin fallback to
 * classify inputs as NUMBER, PHONE, or EMAIL that Rust correctly classifies as TEXT.
 *
 * Each test below documents a case where the BEFORE (divergent) Kotlin fallback
 * produced a WRONG result and the AFTER (aligned) version must produce TEXT to
 * match Rust.
 *
 * Runs under :app:testDebugUnitTest (no Android runtime, no NDK).
 */
class TextKindRustParityTest {

    private fun k(text: String) = TextKind.classifyKind(text)

    // ── NUMBER: must reject non-ASCII digits (Arabic-Indic: U+0660–U+0669) ──────

    /**
     * Arabic-Indic digit string "٤٢" (4, 2 in Eastern Arabic numerals).
     * Rust: is_ascii_digit → false → falls through to PlainText ("TEXT").
     * Old Kotlin: Char.isDigit() returns true for these → was classified as NUMBER.
     * Fixed Kotlin: must use c in '0'..'9' (ascii-only) → TEXT.
     */
    @Test
    fun number_arabicIndicDigits_shouldBeText() {
        // "٤٢" — Arabic-Indic 4 and 2 (U+0664, U+0662)
        assertEquals(TextKind.Kind.TEXT, k("٤٢"))
    }

    @Test
    fun number_arabicIndicDecimal_shouldBeText() {
        // "٣.١٤" — Arabic-Indic 3.14
        assertEquals(TextKind.Kind.TEXT, k("٣.١٤"))
    }

    @Test
    fun number_persianDigits_shouldBeText() {
        // Persian digits ۱۲۳ (U+06F1, U+06F2, U+06F3)
        assertEquals(TextKind.Kind.TEXT, k("۱۲۳"))
    }

    // ── NUMBER: valid ASCII digits should still work (regression guard) ──────────

    @Test
    fun number_asciiInteger_isNumber() {
        assertEquals(TextKind.Kind.NUMBER, k("42"))
    }

    @Test
    fun number_asciiDecimal_isNumber() {
        assertEquals(TextKind.Kind.NUMBER, k("3.14"))
    }

    @Test
    fun number_asciiNegative_isNumber() {
        assertEquals(TextKind.Kind.NUMBER, k("-7.5"))
    }

    // ── EMAIL: must reject non-ASCII letters in local/domain parts ────────────

    /**
     * An email-looking string where the local part contains a Unicode letter.
     * Rust: is_ascii_alphanumeric → false → not a valid email → TEXT.
     * Old Kotlin: isLetterOrDigit includes non-ASCII → was EMAIL.
     * Fixed Kotlin: must use ASCII-only char predicate → TEXT.
     */
    @Test
    fun email_unicodeLocalPart_shouldBeText() {
        // "café@example.com" — 'é' is U+00E9, non-ASCII
        assertEquals(TextKind.Kind.TEXT, k("café@example.com"))
    }

    @Test
    fun email_unicodeDomainPart_shouldBeText() {
        // "user@münchen.de" — 'ü' is U+00FC, non-ASCII in domain
        assertEquals(TextKind.Kind.TEXT, k("user@münchen.de"))
    }

    // ── EMAIL: valid ASCII emails should still work (regression guard) ──────────

    @Test
    fun email_asciiBasic_isEmail() {
        assertEquals(TextKind.Kind.EMAIL, k("user@example.com"))
    }

    @Test
    fun email_asciiWithPlus_isEmail() {
        assertEquals(TextKind.Kind.EMAIL, k("user+tag@mail.example.org"))
    }

    // ── PHONE: must reject non-ASCII digits ──────────────────────────────────

    /**
     * A string composed of Arabic-Indic digits that looks like a phone number
     * to isDigit() but Rust rejects via is_ascii_digit.
     */
    @Test
    fun phone_arabicIndicDigits_shouldBeText() {
        // ٠١٢٣٤٥٦٧٨ — 9 Arabic-Indic digits (>= 7, so old code classified as PHONE)
        assertEquals(TextKind.Kind.TEXT, k("٠١٢٣٤٥٦٧٨"))
    }

    // ── PHONE: valid ASCII phone numbers should still work (regression guard) ─

    @Test
    fun phone_asciiInternational_isPhone() {
        assertEquals(TextKind.Kind.PHONE, k("+1 (800) 555-1234"))
    }

    @Test
    fun phone_asciiDigitsOnly_isPhone() {
        assertEquals(TextKind.Kind.PHONE, k("1234567890"))
    }
}
