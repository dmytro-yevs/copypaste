package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-lxo6: Native-stub degradation must be LOUD, not silent.
 *
 * When libcopypaste_android.so fails to load the app silently stubs ALL FFI:
 * isSensitive returns false, sensitiveCaptureDecision returns non-sensitive,
 * openDatabase returns -1, storeClipboardItem returns "". The user sees a
 * functional-looking app with no security guarantees and no indication that
 * anything is wrong.
 *
 * Fix:
 *  1. [nativeLoadError] captures the [UnsatisfiedLinkError] thrown at load time
 *     so callers can log it at ERROR level with a concrete cause.
 *  2. [checkNativeAbiCompatibility] logs at ERROR (not WARN) when the .so is absent.
 *  3. Security-critical stubs ([isSensitive], [sensitiveCaptureDecision]) log at ERROR.
 *  4. [CopyPasteApp.onCreate] calls [NotificationHelper.notifyNativeUnavailable] immediately
 *     at startup when [isNativeLibraryLoaded] is false, not only on the first capture attempt.
 *
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.NativeStubDegradationTest"
 */
class NativeStubDegradationTest {

    // ── Helpers to locate source files ────────────────────────────────────────

    private fun readSourceFile(name: String): String {
        val anchor = NativeStubDegradationTest::class.java
            .protectionDomain?.codeSource?.location?.toURI()
            ?.let { java.io.File(it) }
        var dir: java.io.File? = anchor
        var moduleRoot: java.io.File? = null
        while (dir != null) {
            if (java.io.File(dir, "src/main").exists()) {
                moduleRoot = dir
                break
            }
            dir = dir.parentFile
        }
        requireNotNull(moduleRoot) { "Could not locate module root from $anchor" }
        return java.io.File(moduleRoot, "src/main/java/com/copypaste/android/$name").readText()
    }

    private val bindingsSource: String by lazy { readSourceFile("CopypasteBindings.kt") }
    private val appSource: String by lazy { readSourceFile("CopyPasteApp.kt") }

    // ── Behavioural tests (JVM stub-mode paths) ───────────────────────────────

    /**
     * In the JVM unit-test environment the .so is never on the classpath.
     * [isNativeLibraryLoaded] must be false — the stub state must be detectable.
     */
    @Test
    fun isNativeLibraryLoaded_isFalse_inJvmTests() {
        assertFalse(
            "isNativeLibraryLoaded must be false in the JVM unit test environment " +
                "(no .so on classpath); stub state must be detectable (lxo6)",
            isNativeLibraryLoaded,
        )
    }

    /**
     * [nativeLoadError] must be non-null when the .so is absent — the
     * [UnsatisfiedLinkError] must be captured so callers can log it at ERROR
     * level with the actual cause rather than an opaque message.
     */
    @Test
    fun nativeLoadError_isNonNull_whenSoAbsent() {
        // In the JVM test environment System.loadLibrary always throws UnsatisfiedLinkError.
        assertNotNull(
            "nativeLoadError must capture the UnsatisfiedLinkError thrown at load time " +
                "so it can be logged at ERROR level with a concrete cause (lxo6)",
            nativeLoadError,
        )
        assertTrue(
            "nativeLoadError must be an UnsatisfiedLinkError",
            nativeLoadError is UnsatisfiedLinkError,
        )
    }

    /**
     * [isNativeLibraryLoaded] must be the logical complement of [nativeLoadError]:
     * loaded iff error is null.
     */
    @Test
    fun isNativeLibraryLoaded_isComplement_ofNativeLoadError() {
        assertEquals(
            "isNativeLibraryLoaded must equal (nativeLoadError == null)",
            nativeLoadError == null,
            isNativeLibraryLoaded,
        )
    }

    /**
     * [isSensitive] returns false in stub mode — the return value is detectable,
     * not silently treated as a genuine "not sensitive" verdict.
     * Callers must gate on [isNativeLibraryLoaded] to distinguish stub from real.
     */
    @Test
    fun isSensitive_stubMode_returnsDetectableFalse() {
        assertFalse(
            "isNativeLibraryLoaded is false (stub mode) — cannot be expected to " +
                "detect sensitive content",
            isNativeLibraryLoaded,
        )
        // In stub mode isSensitive must return false, not throw.
        val result = isSensitive("4111 1111 1111 1111 exp 12/25 cvv 123")
        assertFalse(
            "isSensitive must return false in stub mode (no native detector) — " +
                "callers must check isNativeLibraryLoaded to know the verdict is not real (lxo6)",
            result,
        )
    }

    /**
     * [openDatabase] returns -1 in stub mode — a clearly invalid sentinel, not a
     * real handle. The stub-mode state is detectable.
     */
    @Test
    fun openDatabase_stubMode_returnsNegativeOne() {
        assertFalse("Expected stub mode in JVM tests", isNativeLibraryLoaded)
        val handle = openDatabase("/tmp/test.db", ByteArray(32))
        assertEquals(
            "openDatabase must return -1 in stub mode (not a valid handle) (lxo6)",
            -1L,
            handle,
        )
    }

    /**
     * [storeClipboardItem] returns empty string in stub mode — a clearly invalid
     * sentinel, not a real item id.
     */
    @Test
    fun storeClipboardItem_stubMode_returnsEmptyString() {
        assertFalse("Expected stub mode in JVM tests", isNativeLibraryLoaded)
        val id = storeClipboardItem("/tmp/test.db", ByteArray(32), "hello world", 30L)
        assertEquals(
            "storeClipboardItem must return \"\" in stub mode (not a real row id) (lxo6)",
            "",
            id,
        )
    }

    // ── Source-scan tests: surfacing & log-level contract ─────────────────────

    /**
     * [CopyPasteApp.onCreate] must call [NotificationHelper.notifyNativeUnavailable]
     * when [isNativeLibraryLoaded] is false — at startup, before any capture attempt.
     *
     * Previously the notification was posted only lazily (on the first store failure in
     * ClipboardRepository), so a user who never captured anything would never see it.
     */
    @Test
    fun CopyPasteApp_notifiesNativeUnavailable_atStartup_whenNativeAbsent() {
        assertTrue(
            "CopyPasteApp.onCreate must call NotificationHelper.notifyNativeUnavailable " +
                "when isNativeLibraryLoaded is false — surface the failure at startup, " +
                "not only on the first capture attempt (lxo6)",
            appSource.contains("NotificationHelper.notifyNativeUnavailable") &&
                appSource.contains("isNativeLibraryLoaded"),
        )
        // The notification call must be inside an `if (!isNativeLibraryLoaded)` guard
        // (or equivalent) so it only fires in stub mode.
        val notifyBlock = appSource
            .substringAfter("checkNativeAbiCompatibility()")
            .substringBefore("NotificationHelper.createChannels")
        assertTrue(
            "CopyPasteApp.onCreate must call notifyNativeUnavailable AFTER " +
                "checkNativeAbiCompatibility() and before createChannels(), " +
                "guarded by !isNativeLibraryLoaded (lxo6)",
            notifyBlock.contains("isNativeLibraryLoaded") &&
                notifyBlock.contains("notifyNativeUnavailable"),
        )
    }

    /**
     * [checkNativeAbiCompatibility] must log at ERROR (not WARN) when the native
     * library is absent — the absence is a security-critical degradation.
     */
    @Test
    fun checkNativeAbiCompatibility_logsAtError_inStubMode() {
        val fnBody = bindingsSource
            .substringAfter("fun checkNativeAbiCompatibility()")
            .substringBefore("\nfun ")
        assertTrue(
            "checkNativeAbiCompatibility must log at ERROR (Log.e) when " +
                "isNativeLibraryLoaded is false — native absence is a security degradation, " +
                "not a benign warning (lxo6)",
            fnBody.contains("Log.e(") &&
                fnBody.contains("!isNativeLibraryLoaded"),
        )
    }

    /**
     * [isSensitive] must log at ERROR (not WARN) in stub mode to make the security
     * degradation visible in log aggregators.
     */
    @Test
    fun isSensitive_logsAtError_inStubMode() {
        val fnBody = bindingsSource
            .substringAfter("fun isSensitive(text: String): Boolean {")
            .substringBefore("\nfun ")
        assertTrue(
            "isSensitive must log at ERROR when native library is unavailable — " +
                "the stub returns false for ALL content, defeating sensitive detection (lxo6)",
            fnBody.contains("Log.e("),
        )
    }

    /**
     * [sensitiveCaptureDecision] must log at ERROR (not WARN) in stub mode.
     */
    @Test
    fun sensitiveCaptureDecision_logsAtError_inStubMode() {
        val fnBody = bindingsSource
            .substringAfter("fun sensitiveCaptureDecision(")
            .substringBefore("\nfun ")
        assertTrue(
            "sensitiveCaptureDecision must log at ERROR when native library is unavailable — " +
                "all captures appear non-sensitive, defeating TTL-based auto-wipe (lxo6)",
            fnBody.contains("Log.e("),
        )
    }

    /**
     * [nativeLoadError] must be a public top-level val so [CopyPasteApp] and
     * [checkNativeAbiCompatibility] can reference it for detailed error reporting.
     */
    @Test
    fun nativeLoadError_isPublicTopLevelVal() {
        val clazz = Class.forName("com.copypaste.android.CopypasteBindingsKt")
        val field = runCatching {
            clazz.getDeclaredField("nativeLoadError")
        }.getOrNull()
        assertNotNull(
            "nativeLoadError must exist as a public top-level val (field on CopypasteBindingsKt) " +
                "so CopyPasteApp and checkNativeAbiCompatibility can reference it for diagnostics (lxo6)",
            field,
        )
    }
}
