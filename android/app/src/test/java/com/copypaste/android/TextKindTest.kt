package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [TextKind] stub-mode behavior.
 *
 * Real classification is performed exclusively via the Rust FFI [classifyTextKind]
 * (tested via integration/instrumented tests with the native lib). In stub mode
 * (no native library), [TextKind.classifyKind] must return [TextKind.Kind.TEXT]
 * for ALL inputs — there is no Kotlin-side classifier.
 */
class TextKindTest {

    private fun k(text: String) = TextKind.classifyKind(text)

    // ── Stub mode: classifyKind always returns TEXT ───────────────────────────

    @Test fun stubEmpty() = assertEquals(TextKind.Kind.TEXT, k(""))
    @Test fun stubWhitespace() = assertEquals(TextKind.Kind.TEXT, k("   \t\n"))
    @Test fun stubPlainText() = assertEquals(TextKind.Kind.TEXT, k("hello world"))
    @Test fun stubSentence() = assertEquals(TextKind.Kind.TEXT,
        k("The quick brown fox jumps over the lazy dog."))
    @Test fun stubUrl() = assertEquals(TextKind.Kind.TEXT, k("https://example.com"))
    @Test fun stubUrlHttp() = assertEquals(TextKind.Kind.TEXT, k("http://example.com/path?q=1"))
    @Test fun stubUrlFtp() = assertEquals(TextKind.Kind.TEXT, k("ftp://files.example.org"))
    @Test fun stubEmail() = assertEquals(TextKind.Kind.TEXT, k("user@example.com"))
    @Test fun stubMailto() = assertEquals(TextKind.Kind.TEXT, k("mailto:user@example.com"))
    @Test fun stubColorHex3() = assertEquals(TextKind.Kind.TEXT, k("#fff"))
    @Test fun stubColorHex6() = assertEquals(TextKind.Kind.TEXT, k("#1a2b3c"))
    @Test fun stubColorHex8() = assertEquals(TextKind.Kind.TEXT, k("#aabbccdd"))
    @Test fun stubPhone() = assertEquals(TextKind.Kind.TEXT, k("+1 (800) 555-1234"))
    @Test fun stubPhoneDigitsOnly() = assertEquals(TextKind.Kind.TEXT, k("1234567890"))
    @Test fun stubNumber() = assertEquals(TextKind.Kind.TEXT, k("42"))
    @Test fun stubNumberDecimal() = assertEquals(TextKind.Kind.TEXT, k("3.14"))
    @Test fun stubNumberNegative() = assertEquals(TextKind.Kind.TEXT, k("-7.5"))
    @Test fun stubJson() = assertEquals(TextKind.Kind.TEXT, k("""{"key": "value"}"""))
    @Test fun stubJsonArray() = assertEquals(TextKind.Kind.TEXT, k("[1, 2, 3]"))
    @Test fun stubPath() = assertEquals(TextKind.Kind.TEXT, k("/usr/local/bin/cargo"))
    @Test fun stubPathHome() = assertEquals(TextKind.Kind.TEXT, k("~/Documents/notes.txt"))
    @Test fun stubCode() = assertEquals(TextKind.Kind.TEXT, k("fn main() {\n    println!(\"hello\");\n}"))
    @Test fun stubCodeArrow() = assertEquals(TextKind.Kind.TEXT, k("const f = x => x * 2"))
    @Test fun stubArabicIndicDigits() = assertEquals(TextKind.Kind.TEXT, k("٤٢"))
    @Test fun stubUnicodeEmail() = assertEquals(TextKind.Kind.TEXT, k("café@example.com"))

    // ── Kind enum label roundtrip ─────────────────────────────────────────────

    @Test fun labelsCorrect() {
        assertEquals("TEXT",   TextKind.Kind.TEXT.label)
        assertEquals("URL",    TextKind.Kind.URL.label)
        assertEquals("EMAIL",  TextKind.Kind.EMAIL.label)
        assertEquals("PHONE",  TextKind.Kind.PHONE.label)
        assertEquals("COLOR",  TextKind.Kind.COLOR.label)
        assertEquals("JSON",   TextKind.Kind.JSON.label)
        assertEquals("CODE",   TextKind.Kind.CODE.label)
        assertEquals("NUMBER", TextKind.Kind.NUMBER.label)
        assertEquals("PATH",   TextKind.Kind.PATH.label)
    }
}
