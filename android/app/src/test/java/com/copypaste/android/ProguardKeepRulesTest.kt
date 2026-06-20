package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * CopyPaste-hh3w: Verify that proguard-rules.pro contains all keep rules
 * required to prevent R8 from stripping UniFFI + JNA classes in the release build.
 *
 * Without these rules R8 renames/strips JNA interop and UniFFI-generated bindings
 * (which rely on RUNTIME REFLECTION), causing UnsatisfiedLinkError and silent
 * fallback to the crypto STUB instead of real XChaCha20-Poly1305.
 *
 * This test is a pure-JVM structural guard — it reads the proguard file from disk
 * and asserts the required rules are present. Runs under `./gradlew :app:testDebugUnitTest`.
 */
class ProguardKeepRulesTest {

    private fun proguardText(): String {
        // Working directory during unit tests is android/app/
        val f = File("proguard-rules.pro")
        assertTrue("proguard-rules.pro not found at ${f.absolutePath}", f.exists())
        return f.readText()
    }

    // ── JNA core keep rules ────────────────────────────────────────────────────

    @Test
    fun proguard_keeps_com_sun_jna() {
        val rules = proguardText()
        assertTrue(
            "proguard-rules.pro must keep 'com.sun.jna.**' (JNA binding reflection " +
                "uses com.sun.jna.* class/method names at runtime)",
            rules.contains("-keep class com.sun.jna.**"),
        )
    }

    @Test
    fun proguard_keeps_net_java_dev_jna() {
        val rules = proguardText()
        assertTrue(
            "proguard-rules.pro must keep 'net.java.dev.jna.**' (AAR maven group — " +
                "R8 may resolve the JNA classes under this namespace on some toolchains)",
            rules.contains("-keep class net.java.dev.jna.**"),
        )
    }

    @Test
    fun proguard_keeps_JNA_Structure_subclasses() {
        val rules = proguardText()
        // JNA maps native struct layout by FIELD NAME via @Structure.FieldOrder.
        // If Structure subclasses are renamed the struct layout is corrupted.
        assertTrue(
            "proguard-rules.pro must keep all Structure subclasses with members " +
                "(RustBuffer, ForeignBytes, UniffiRustCallStatus use @Structure.FieldOrder)",
            rules.contains("extends com.sun.jna.Structure"),
        )
    }

    @Test
    fun proguard_keeps_JNA_Callback_implementors() {
        val rules = proguardText()
        assertTrue(
            "proguard-rules.pro must keep JNA Callback implementors " +
                "(UniFFI async/future continuations are com.sun.jna.Callback interfaces)",
            rules.contains("implements com.sun.jna.Callback"),
        )
    }

    @Test
    fun proguard_keeps_JNA_Library_interfaces() {
        val rules = proguardText()
        assertTrue(
            "proguard-rules.pro must keep JNA Library interfaces " +
                "(UniffiLib extends com.sun.jna.Library; Native.load binds methods by name)",
            rules.contains("extends com.sun.jna.Library"),
        )
    }

    // ── Annotation preservation ────────────────────────────────────────────────

    @Test
    fun proguard_keepattributes_annotations() {
        val rules = proguardText()
        // JNA reads @Structure.FieldOrder at runtime. R8 may strip annotation
        // attributes unless explicitly preserved. The rule must cover at minimum
        // RuntimeVisibleAnnotations (used by JNA to discover @FieldOrder).
        assertTrue(
            "proguard-rules.pro must include '-keepattributes *Annotation*' (or an explicit " +
                "RuntimeVisibleAnnotations keepattributes) so R8 does not strip @Structure.FieldOrder " +
                "from RustBuffer/ForeignBytes/UniffiRustCallStatus — JNA reads these at runtime",
            rules.contains("-keepattributes") && rules.contains("Annotation"),
        )
    }

    // ── UniFFI generated bindings ──────────────────────────────────────────────

    @Test
    fun proguard_keeps_uniffi_copypaste_android_package() {
        val rules = proguardText()
        assertTrue(
            "proguard-rules.pro must keep 'uniffi.copypaste_android.**' — the generated " +
                "Kotlin bindings live in this package (package declaration in copypaste_android.kt)",
            rules.contains("uniffi.copypaste_android"),
        )
    }

    @Test
    fun proguard_keeps_com_copypaste_generated_uniffi_package() {
        val rules = proguardText()
        // The on-disk path differs from the Kotlin package (com/copypaste/generated/uniffi/…)
        // so keep that namespace too in case any tooling-generated class lands under it.
        assertTrue(
            "proguard-rules.pro must keep 'com.copypaste.generated.uniffi.**' " +
                "(on-disk path of the generated bindings, may diverge from the Kotlin package)",
            rules.contains("com.copypaste.generated.uniffi"),
        )
    }

    // ── Dontwarn: suppress missing class warnings from JNA desktop paths ───────

    @Test
    fun proguard_dontwarn_java_awt() {
        val rules = proguardText()
        // JNA references java.awt.* (desktop) which is absent on Android; silence
        // the warning so R8 does not fail the build with missing-class errors.
        assertTrue(
            "proguard-rules.pro must include '-dontwarn java.awt.**' to suppress R8 " +
                "missing-class warnings for JNA's desktop code paths",
            rules.contains("-dontwarn java.awt.**"),
        )
    }
}
