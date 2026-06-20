package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-8r3p: Verify the runtime ABI guard that detects unsupported CPU ABIs.
 *
 * abiFilters = ["arm64-v8a"] means the .so is only packaged for arm64-v8a. On a
 * 32-bit armeabi-v7a device Android cannot load the .so, [isNativeLibraryLoaded]
 * becomes false, and the app silently stubs ALL FFI — including crypto — without
 * any user-visible indication.
 *
 * [isSupportedAbi] (in CopypasteBindings.kt) exposes the list of supported ABIs so
 * [CopyPasteApp] can check it at startup and warn (or fail fast) clearly, rather than
 * silently running in crypto-stub mode on unsupported hardware.
 *
 * These are pure-JVM unit tests; [android.os.Build.SUPPORTED_ABIS] is not available
 * on the JVM, so we test the helper logic via the function's contract.
 */
class UnsupportedAbiGuardTest {

    // ── Supported ABI set ──────────────────────────────────────────────────────

    @Test
    fun supportedAbis_containsArm64() {
        // The compiled .so supports only arm64-v8a (abiFilters in build.gradle.kts).
        // The supported-ABI set must reflect this exactly so the guard is not too permissive.
        assertTrue(
            "SUPPORTED_NATIVE_ABIS must contain 'arm64-v8a'",
            SUPPORTED_NATIVE_ABIS.contains("arm64-v8a"),
        )
    }

    @Test
    fun supportedAbis_doesNotContain32BitArm() {
        // 32-bit armeabi-v7a is NOT supported — the Rust .so is compiled for arm64 only.
        // Including it would make [isSupportedAbi] return true for devices that will stub.
        assertFalse(
            "SUPPORTED_NATIVE_ABIS must NOT contain 'armeabi-v7a' " +
                "(32-bit arm is not built by cargo-ndk; adding it makes the guard a no-op on 32-bit devices)",
            SUPPORTED_NATIVE_ABIS.contains("armeabi-v7a"),
        )
    }

    @Test
    fun supportedAbis_doesNotContainX86() {
        // x86 is a 32-bit target; not packaged.
        assertFalse(
            "SUPPORTED_NATIVE_ABIS must NOT contain 'x86' (32-bit x86 not built)",
            SUPPORTED_NATIVE_ABIS.contains("x86"),
        )
    }

    // ── isSupportedAbi logic ───────────────────────────────────────────────────

    @Test
    fun isSupportedAbi_trueForArm64() {
        assertTrue(
            "isSupportedAbi(\"arm64-v8a\") must return true",
            isSupportedAbi("arm64-v8a"),
        )
    }

    @Test
    fun isSupportedAbi_falseFor32BitArm() {
        assertFalse(
            "isSupportedAbi(\"armeabi-v7a\") must return false — no .so shipped for 32-bit ARM",
            isSupportedAbi("armeabi-v7a"),
        )
    }

    @Test
    fun isSupportedAbi_falseForX86() {
        assertFalse(
            "isSupportedAbi(\"x86\") must return false",
            isSupportedAbi("x86"),
        )
    }

    @Test
    fun isSupportedAbi_falseForEmpty() {
        assertFalse(
            "isSupportedAbi(\"\") must return false",
            isSupportedAbi(""),
        )
    }
}
