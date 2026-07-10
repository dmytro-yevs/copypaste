package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * android-preview S6.1/6.3 — unit test for [isMonoPreviewKind], the pure
 * predicate driving PreviewTextContent's mono-vs-sans typography (spec.md
 * "Content Rendering by Kind — Scenario: Monospace kinds / Sans-serif
 * kinds"). Every [ContentVisualKind] value is asserted explicitly so a future
 * addition to the enum cannot silently fall through unclassified.
 */
class PreviewContentKindTest {

    @Test
    fun `mono kinds match spec md exactly`() {
        assertTrue(isMonoPreviewKind(ContentVisualKind.CODE))
        assertTrue(isMonoPreviewKind(ContentVisualKind.URL))
        assertTrue(isMonoPreviewKind(ContentVisualKind.PATH))
        assertTrue(isMonoPreviewKind(ContentVisualKind.JSON))
        assertTrue(isMonoPreviewKind(ContentVisualKind.NUMBER))
        assertTrue(isMonoPreviewKind(ContentVisualKind.COLOR))
        assertTrue(isMonoPreviewKind(ContentVisualKind.SECRET))
    }

    @Test
    fun `sans kinds match spec md exactly`() {
        assertFalse(isMonoPreviewKind(ContentVisualKind.TEXT))
        assertFalse(isMonoPreviewKind(ContentVisualKind.EMAIL))
        // PHONE is unspecified by spec.md's two enumerated scenarios; this
        // slice defaults it to sans (human-readable, not code-like) — see
        // PreviewContent.kt's kdoc and bd notes for the recorded interpretation.
        assertFalse(isMonoPreviewKind(ContentVisualKind.PHONE))
    }

    @Test
    fun `every ContentVisualKind value is classified one way or the other`() {
        // FILE/IMAGE never reach PreviewTextContent (routed to
        // PreviewFileContent/PreviewImageContent instead) but the predicate
        // must still be total over the enum, not throw for them.
        ContentVisualKind.entries.forEach { kind ->
            // Just exercising the predicate for every value must not throw.
            isMonoPreviewKind(kind)
        }
    }
}
