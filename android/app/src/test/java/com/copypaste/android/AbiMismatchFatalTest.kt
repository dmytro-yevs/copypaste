package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.lang.reflect.Method

/**
 * CopyPaste-fkx7: ABI mismatch must be FATAL (throw IllegalStateException),
 * not silently degrade into a broken stub state that corrupts crypto data.
 *
 * These are pure-JVM unit tests that verify the contract of
 * [checkNativeAbiCompatibility] without needing the native .so present:
 *
 * 1. The function now returns Unit (was Boolean) — a mismatch can no longer be
 *    ignored by ignoring the return value.
 * 2. When the native .so is not loaded [isNativeLibraryLoaded] == false, the
 *    function returns normally (stub mode is safe; no FFI surface to mismatch).
 * 3. The function is a top-level fun in the com.copypaste.android package — it
 *    can be called from Application.onCreate.
 *
 * Full mismatch path (VersionException → IllegalStateException) is verified by
 * inspection: the source was changed; the instrumented test on a real device
 * with a mismatched .so would exercise the throw path.
 */
class AbiMismatchFatalTest {

    /**
     * [checkNativeAbiCompatibility] must return Unit, not Boolean.
     * Returning Boolean lets callers silently ignore a mismatch.
     */
    @Test
    fun checkNativeAbiCompatibility_returnsUnit() {
        // Locate the top-level function via reflection on the generated Kt class.
        // Top-level Kotlin functions in a file compile to static methods on
        // <FileName>Kt class — CopypasteBindings.kt → CopypasteBindingsKt.
        val clazz = Class.forName("com.copypaste.android.CopypasteBindingsKt")
        val methods = clazz.declaredMethods.filter { it.name == "checkNativeAbiCompatibility" }
        assertTrue(
            "checkNativeAbiCompatibility must exist as a top-level function",
            methods.isNotEmpty(),
        )
        val method = methods.first()
        assertEquals(
            "checkNativeAbiCompatibility must return Unit/void (CopyPaste-fkx7: " +
                "a Boolean return lets callers silently ignore ABI mismatches)",
            Void.TYPE,
            method.returnType,
        )
    }

    /**
     * When the native library is not loaded (isNativeLibraryLoaded == false),
     * [checkNativeAbiCompatibility] must return normally — stub mode is safe.
     *
     * In the JVM unit test environment [isNativeLibraryLoaded] is always false
     * because libcopypaste_android.so is not present, so this test exercises the
     * stub-mode path directly.
     */
    @Test
    fun checkNativeAbiCompatibility_stubMode_doesNotThrow() {
        // In JVM unit tests the .so is never present → isNativeLibraryLoaded == false.
        // The function must return without throwing.
        assertFalse(
            "Expected isNativeLibraryLoaded=false in JVM unit test environment",
            isNativeLibraryLoaded,
        )
        // Must not throw — stub mode is always safe (no ABI surface to mismatch).
        checkNativeAbiCompatibility()
    }

    /**
     * [APP_ABI_VERSION] must be a UInt constant > 0 — it is the version that will
     * be passed to the native check_compatibility function.
     */
    @Test
    fun appAbiVersion_isPositive() {
        assertTrue(
            "APP_ABI_VERSION must be > 0 (a zero version means the constant was reset or deleted)",
            APP_ABI_VERSION > 0u,
        )
    }
}
