package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [TextKind.classify], mirroring the Rust tests in
 * `copypaste-core/src/text_kind.rs` so the Kotlin port stays in sync.
 */
class TextKindTest {

    private fun k(text: String) = TextKind.classifyKind(text)

    // ── TEXT (plain) ──────────────────────────────────────────────────────────

    @Test fun plainEmpty() = assertEquals(TextKind.Kind.TEXT, k(""))
    @Test fun plainWhitespaceOnly() = assertEquals(TextKind.Kind.TEXT, k("   \t\n"))
    @Test fun plainHelloWorld() = assertEquals(TextKind.Kind.TEXT, k("hello world"))
    @Test fun plainSentence() = assertEquals(TextKind.Kind.TEXT,
        k("The quick brown fox jumps over the lazy dog."))

    // ── URL ───────────────────────────────────────────────────────────────────

    @Test fun urlHttps() = assertEquals(TextKind.Kind.URL, k("https://example.com"))
    @Test fun urlHttp() = assertEquals(TextKind.Kind.URL, k("http://example.com/path?q=1"))
    @Test fun urlFtp() = assertEquals(TextKind.Kind.URL, k("ftp://files.example.org"))
    @Test fun urlUppercaseScheme() = assertEquals(TextKind.Kind.URL, k("HTTPS://EXAMPLE.COM"))
    @Test fun urlWithSpaceIsText() = assertEquals(TextKind.Kind.TEXT,
        k("https://example.com/path with spaces"))
    @Test fun mailtoIsEmailNotUrl() = assertEquals(TextKind.Kind.EMAIL,
        k("mailto:user@example.com"))
    @Test fun trimmedUrlWithSpaces() = assertEquals(TextKind.Kind.URL,
        k("  https://example.com  "))

    // ── EMAIL ─────────────────────────────────────────────────────────────────

    @Test fun emailBasic() = assertEquals(TextKind.Kind.EMAIL, k("user@example.com"))
    @Test fun emailWithPlus() = assertEquals(TextKind.Kind.EMAIL, k("user+tag@mail.example.org"))
    @Test fun emailNoTldIsText() = assertEquals(TextKind.Kind.TEXT, k("a@b"))
    @Test fun emailNoAtIsText() = assertEquals(TextKind.Kind.TEXT, k("userexample.com"))
    @Test fun emailTwoAtsIsText() = assertEquals(TextKind.Kind.TEXT, k("a@b@c.com"))
    @Test fun emailWithSpaceIsText() = assertEquals(TextKind.Kind.TEXT, k("user @example.com"))

    // ── COLOR ─────────────────────────────────────────────────────────────────

    @Test fun colorHex3() = assertEquals(TextKind.Kind.COLOR, k("#fff"))
    @Test fun colorHex6() = assertEquals(TextKind.Kind.COLOR, k("#1a2b3c"))
    @Test fun colorHex8() = assertEquals(TextKind.Kind.COLOR, k("#aabbccdd"))
    @Test fun colorHex4() = assertEquals(TextKind.Kind.COLOR, k("#abcd"))
    @Test fun colorHexWrongLengthIsText() = assertEquals(TextKind.Kind.TEXT, k("#12345"))
    @Test fun colorHexNoHashIsText() = assertEquals(TextKind.Kind.TEXT, k("ffffff"))
    @Test fun colorHexInvalidCharsIsText() = assertEquals(TextKind.Kind.TEXT, k("#zzzzzz"))
    @Test fun colorHexUppercase() = assertEquals(TextKind.Kind.COLOR, k("#AABBCC"))

    // ── PHONE ─────────────────────────────────────────────────────────────────

    @Test fun phoneInternational() = assertEquals(TextKind.Kind.PHONE, k("+1 (800) 555-1234"))
    @Test fun phoneDigitsOnly() = assertEquals(TextKind.Kind.PHONE, k("1234567890"))
    @Test fun phoneTooShortIsText() = assertEquals(TextKind.Kind.TEXT, k("12-3456"))
    @Test fun phoneWithAlphaIsText() = assertEquals(TextKind.Kind.TEXT, k("+1abc5551234"))
    @Test fun phoneWithDashes() = assertEquals(TextKind.Kind.PHONE, k("555-867-5309"))

    // ── NUMBER ────────────────────────────────────────────────────────────────

    @Test fun numberInteger() = assertEquals(TextKind.Kind.NUMBER, k("42"))
    @Test fun numberDecimal() = assertEquals(TextKind.Kind.NUMBER, k("3.14"))
    @Test fun numberNegative() = assertEquals(TextKind.Kind.NUMBER, k("-7.5"))
    @Test fun numberThousandsSep() = assertEquals(TextKind.Kind.NUMBER, k("1,234.56"))
    @Test fun numberLargeIntWithCommas() = assertEquals(TextKind.Kind.NUMBER, k("1,000,000"))
    @Test fun numberWithAlphaIsText() = assertEquals(TextKind.Kind.TEXT, k("42px"))
    @Test fun numberJustDotIsText() = assertEquals(TextKind.Kind.TEXT, k("."))
    @Test fun numberPositiveSign() = assertEquals(TextKind.Kind.NUMBER, k("+42"))

    // ── JSON ──────────────────────────────────────────────────────────────────

    @Test fun jsonObject() = assertEquals(TextKind.Kind.JSON, k("""{"key": "value"}"""))
    @Test fun jsonArray() = assertEquals(TextKind.Kind.JSON, k("[1, 2, 3]"))
    @Test fun jsonNested() = assertEquals(TextKind.Kind.JSON, k("""{"a": {"b": [1, 2]}}"""))
    @Test fun jsonInvalidBracesIsText() = assertEquals(TextKind.Kind.TEXT, k("{not json}"))
    @Test fun jsonEmptyObject() = assertEquals(TextKind.Kind.JSON, k("{}"))
    @Test fun jsonEmptyArray() = assertEquals(TextKind.Kind.JSON, k("[]"))
    @Test fun jsonStringOnlyNoBracesIsText() = assertEquals(TextKind.Kind.TEXT, k("\"just a string\""))

    // ── PATH ─────────────────────────────────────────────────────────────────

    @Test fun pathAbsoluteUnix() = assertEquals(TextKind.Kind.PATH, k("/usr/local/bin/cargo"))
    @Test fun pathHomeRelative() = assertEquals(TextKind.Kind.PATH, k("~/Documents/notes.txt"))
    @Test fun pathWindows() = assertEquals(TextKind.Kind.PATH, k("C:\\Users\\Alice\\file.txt"))
    @Test fun pathNoPrefixIsText() = assertEquals(TextKind.Kind.TEXT, k("relative/path/to/file"))
    @Test fun pathSingleSlashIsText() = assertEquals(TextKind.Kind.TEXT, k("/"))
    @Test fun pathMultilineIsText() = assertEquals(TextKind.Kind.TEXT, k("/usr/bin\n/etc/passwd"))
    @Test fun pathDeep() = assertEquals(TextKind.Kind.PATH,
        k("/home/user/.config/copypaste/settings.json"))

    // ── CODE ─────────────────────────────────────────────────────────────────

    @Test fun codeRustMultiline() = assertEquals(TextKind.Kind.CODE,
        k("fn main() {\n    println!(\"hello\");\n}"))
    @Test fun codePythonMultiline() = assertEquals(TextKind.Kind.CODE,
        k("def foo(x):\n    return x + 1"))
    @Test fun codeJsArrowSingleLine() = assertEquals(TextKind.Kind.CODE,
        k("const f = x => x * 2"))
    @Test fun codeImportMultiline() = assertEquals(TextKind.Kind.CODE,
        k("import React from 'react';\nimport { useState } from 'react';"))
    @Test fun codeHtmlTag() = assertEquals(TextKind.Kind.CODE,
        k("<div>\n  <p>hello</p>\n</div>"))
    @Test fun codeCIncludeMultiline() = assertEquals(TextKind.Kind.CODE,
        k("#include <stdio.h>\nint main() { return 0; }"))

    // ── Label roundtrip ───────────────────────────────────────────────────────

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
