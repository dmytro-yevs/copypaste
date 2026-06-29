package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Pure-JVM structural test for CopyPaste-xi8h (updated for CopyPaste-7rxb).
 *
 * Verifies that PermissionsSettingsActivity.kt:
 *  - Does NOT use bare hardcoded Ide* colour constants (IdeText, IdeDim,
 *    IdeSuccess, IdeBorder, IdeDanger) as colour arguments.
 *  - DOES read themed colours from `LocalCpColors.current` (the two-axis token
 *    contract that replaced the deleted `LocalIdeColors` adapter).
 *
 * Text-scan test: catches regressions where a developer re-introduces a
 * hardcoded constant instead of the themed token. Import lines are ignored.
 */
class PermissionsThemedColorsTest {

    private val sourceFile: File = run {
        val candidates = listOf(
            File("src/main/java/com/copypaste/android/PermissionsSettingsActivity.kt"),
            File("app/src/main/java/com/copypaste/android/PermissionsSettingsActivity.kt"),
        )
        candidates.firstOrNull { it.exists() }
            ?: error("Cannot locate PermissionsSettingsActivity.kt; searched: $candidates")
    }

    /** Lines that are not imports and not blank. */
    private val codeLines: List<String> by lazy {
        sourceFile.readLines()
            .filter { it.isNotBlank() && !it.trimStart().startsWith("import ") }
    }

    private fun assertNoBareConstant(constant: String) {
        val violators = codeLines.filter { it.contains(constant) }
        assertTrue(
            "Found bare $constant usage in non-import lines " +
                "(use the matching LocalCpColors.current token instead): $violators",
            violators.isEmpty(),
        )
    }

    @Test
    fun `does not use bare IdeText constant`() = assertNoBareConstant("IdeText")

    @Test
    fun `does not use bare IdeDim constant`() = assertNoBareConstant("IdeDim")

    @Test
    fun `does not use bare IdeSuccess constant`() = assertNoBareConstant("IdeSuccess")

    @Test
    fun `does not use bare IdeBorder constant`() = assertNoBareConstant("IdeBorder")

    @Test
    fun `does not use bare IdeDanger constant`() = assertNoBareConstant("IdeDanger")

    @Test
    fun `PermissionsSettingsActivity reads themed colours from LocalCpColors`() {
        val hasLocalCpColors = sourceFile.readLines().any { it.contains("LocalCpColors") }
        assertTrue(
            "PermissionsSettingsActivity.kt must reference LocalCpColors.current for themed colours",
            hasLocalCpColors,
        )
    }
}
