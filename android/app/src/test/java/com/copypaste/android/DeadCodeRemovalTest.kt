package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Regression tests for CopyPaste-loyk.3 — dead `sensitiveKind()` wrapper removal.
 *
 * The `sensitiveKind(text: String): String?` function in CopypasteBindings.kt was an
 * unused FFI wrapper (zero callers in the entire Android codebase). It was removed
 * because [SensitiveCaptureDecision.kind] already surfaces the sensitive-kind label
 * at capture time via [sensitiveCaptureDecision]; a separate call-after-the-fact is
 * redundant.
 *
 * These tests verify:
 *  1. The `sensitiveKind` symbol is no longer present as a public wrapper (checked
 *     via reflection on CopypasteBindingsKt).
 *  2. [detectSensitiveSpans] (a peer function in the same file) still compiles and
 *     runs in stub mode — exercising the surrounding code path to confirm the
 *     deletion did not cause a compilation regression.
 *
 * All tests run on the JVM — no NDK, no Android runtime required.
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.DeadCodeRemovalTest"
 */
class DeadCodeRemovalTest {

    // ── 1. sensitiveKind is absent ────────────────────────────────────────────────

    /**
     * CopyPaste-loyk.3: the top-level `sensitiveKind` wrapper must not be present.
     * If it is accidentally re-introduced, this test fails and the auditor sees it
     * before the code ships.
     *
     * Top-level Kotlin functions in CopypasteBindings.kt compile to static methods
     * on the synthetic `CopypasteBindingsKt` class; we probe that class.
     */
    @Test
    fun sensitiveKindWrapper_notPresentInCopypasteBindings() {
        val bindingsClass = Class.forName("com.copypaste.android.CopypasteBindingsKt")
        val methodNames = bindingsClass.declaredMethods.map { it.name }
        assertFalse(
            "sensitiveKind() must have been deleted (CopyPaste-loyk.3: zero callers; " +
                "SensitiveCaptureDecision.kind is the replacement)",
            "sensitiveKind" in methodNames,
        )
    }

    // ── 2. Surrounding code is unharmed ──────────────────────────────────────────

    /**
     * Confirms that deleting `sensitiveKind` did not break adjacent code.
     * [detectSensitiveSpans] is the function immediately before the deleted block;
     * in stub mode it must still return an empty list without throwing.
     */
    @Test
    fun detectSensitiveSpans_afterDeletion_stillWorksInStubMode() {
        val result = detectSensitiveSpans("4111 1111 1111 1111")
        assertNotNull("detectSensitiveSpans must not be null after deletion", result)
        assertTrue("Stub mode must return empty list", result.isEmpty())
    }

    /**
     * [applySpanMasking] is the function immediately after the deleted block;
     * in stub mode it must still return the original text unchanged.
     */
    @Test
    fun applySpanMasking_afterDeletion_noSpansReturnsOriginal() {
        val text = "hello world"
        val result = applySpanMasking(text, emptyList())
        assertTrue(
            "applySpanMasking with no spans must return original text unchanged after deletion",
            result == text,
        )
    }
}
