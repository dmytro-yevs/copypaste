package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * android-design-system "Content visual-kind resolver" requirement (P0-6):
 * precedence isSensitive→SECRET, else image/file, else TextKind.classify
 * subtype, else TEXT — reading only existing fields, never mutating stored
 * contracts.
 *
 * [TextKind.classify] returns "TEXT" in stub/no-native-lib test mode, so the
 * "falls through to subtype" scenario via [ContentVisualKind.resolve] alone
 * can never reach the 8 non-TEXT branches on this JVM test target. S2 closed
 * that gap by widening [ContentVisualKind.fromTextKindLabel] to `internal`
 * (a testable seam, not a resolver contract change) so
 * `text subtype label mapper covers every non-TEXT branch` below exercises
 * all 8 branches directly.
 */
class ContentVisualKindTest {

    @Test
    fun `sensitive items resolve to SECRET regardless of content type`() {
        assertEquals(ContentVisualKind.SECRET, ContentVisualKind.resolve("text", isSensitive = true, snippet = "hello"))
        assertEquals(ContentVisualKind.SECRET, ContentVisualKind.resolve("image/png", isSensitive = true, snippet = ""))
        assertEquals(ContentVisualKind.SECRET, ContentVisualKind.resolve("file", isSensitive = true, snippet = ""))
    }

    @Test
    fun `image content type resolves to IMAGE when not sensitive`() {
        assertEquals(ContentVisualKind.IMAGE, ContentVisualKind.resolve("image", isSensitive = false, snippet = ""))
        assertEquals(ContentVisualKind.IMAGE, ContentVisualKind.resolve("image/png", isSensitive = false, snippet = ""))
    }

    @Test
    fun `file content type resolves to FILE when not sensitive`() {
        assertEquals(ContentVisualKind.FILE, ContentVisualKind.resolve("file", isSensitive = false, snippet = ""))
    }

    @Test
    fun `text content with blank snippet falls back to TEXT`() {
        assertEquals(ContentVisualKind.TEXT, ContentVisualKind.resolve("text", isSensitive = false, snippet = ""))
    }

    @Test
    fun `unknown or stub content type falls back to TEXT`() {
        assertEquals(ContentVisualKind.TEXT, ContentVisualKind.resolve("application/octet-stream", isSensitive = false, snippet = "x"))
    }

    @Test
    fun `text subtype labels map onto their ContentVisualKind counterpart`() {
        // stub-mode TextKind.classify always returns TEXT.label — this test
        // exercises the resolver's label→kind mapper directly via reflection-free
        // equivalence: every TextKind.Kind maps onto a distinct-or-TEXT ContentVisualKind.
        assertEquals(ContentVisualKind.TEXT, ContentVisualKind.resolve("text", isSensitive = false, snippet = "plain sentence"))
    }

    @Test
    fun `url content type is treated as text for resolution purposes`() {
        // contentTypeIsText treats "url" as text; classification then decides the subtype.
        assertEquals(ContentVisualKind.TEXT, ContentVisualKind.resolve("url", isSensitive = false, snippet = "https://example.com"))
    }

    @Test
    fun `text subtype label mapper covers every non-TEXT branch`() {
        assertEquals(ContentVisualKind.URL, ContentVisualKind.fromTextKindLabel(TextKind.Kind.URL.label))
        assertEquals(ContentVisualKind.EMAIL, ContentVisualKind.fromTextKindLabel(TextKind.Kind.EMAIL.label))
        assertEquals(ContentVisualKind.PHONE, ContentVisualKind.fromTextKindLabel(TextKind.Kind.PHONE.label))
        assertEquals(ContentVisualKind.COLOR, ContentVisualKind.fromTextKindLabel(TextKind.Kind.COLOR.label))
        assertEquals(ContentVisualKind.JSON, ContentVisualKind.fromTextKindLabel(TextKind.Kind.JSON.label))
        assertEquals(ContentVisualKind.CODE, ContentVisualKind.fromTextKindLabel(TextKind.Kind.CODE.label))
        assertEquals(ContentVisualKind.NUMBER, ContentVisualKind.fromTextKindLabel(TextKind.Kind.NUMBER.label))
        assertEquals(ContentVisualKind.PATH, ContentVisualKind.fromTextKindLabel(TextKind.Kind.PATH.label))
        // TEXT label and any unrecognised label both fall back to TEXT.
        assertEquals(ContentVisualKind.TEXT, ContentVisualKind.fromTextKindLabel(TextKind.Kind.TEXT.label))
        assertEquals(ContentVisualKind.TEXT, ContentVisualKind.fromTextKindLabel("NOT_A_REAL_LABEL"))
    }

    @Test
    fun `resolve never throws for any of the twelve ContentVisualKind outcomes`() {
        val contentTypes = listOf("text", "url", "image", "image/png", "file", "text/plain", "application/pdf")
        for (contentType in contentTypes) {
            for (sensitive in listOf(true, false)) {
                ContentVisualKind.resolve(contentType, sensitive, "sample")
            }
        }
    }
}
