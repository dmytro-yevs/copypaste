package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Pure-JVM structural test for CopyPaste-xi8h.
 *
 * Verifies that PermissionsSettingsActivity.kt:
 *  - Does NOT use bare hardcoded Ide* constants (IdeText, IdeDim, IdeSuccess,
 *    IdeBorder, IdeDanger) as color arguments in composable call sites.
 *  - DOES use LocalIdeColors.current to look up those tokens.
 *
 * This is a text-scan test; it catches regressions where a developer
 * accidentally re-introduces a hardcoded constant instead of the themed token.
 *
 * The scan ignores:
 *  - Import lines (import com.copypaste.android.ui.theme.Ide*)
 *  - The constants' own declarations in Color.kt (not in scope here)
 */
class PermissionsLocalIdeColorsTest {

    private val sourceFile: File = run {
        // Locate PermissionsSettingsActivity.kt relative to test working dir.
        // Works both locally (project root) and in CI (gradle test task sets
        // user.dir to the module root).
        val candidates = listOf(
            File("src/main/java/com/copypaste/android/PermissionsSettingsActivity.kt"),
            File("app/src/main/java/com/copypaste/android/PermissionsSettingsActivity.kt"),
        )
        candidates.firstOrNull { it.exists() }
            ?: error("Cannot locate PermissionsSettingsActivity.kt; searched: $candidates")
    }

    /** Lines that are not imports and not blank */
    private val codeLines: List<String> by lazy {
        sourceFile.readLines()
            .filter { it.isNotBlank() && !it.trimStart().startsWith("import ") }
    }

    @Test
    fun `PermissionsSettingsActivity does not use bare IdeText in code`() {
        val violators = codeLines.filter { it.contains("IdeText") }
        assertTrue(
            "Found bare IdeText usage in non-import lines (use c.text from LocalIdeColors.current): $violators",
            violators.isEmpty(),
        )
    }

    @Test
    fun `PermissionsSettingsActivity does not use bare IdeDim in code`() {
        val violators = codeLines.filter { it.contains("IdeDim") }
        assertTrue(
            "Found bare IdeDim usage in non-import lines (use c.dim from LocalIdeColors.current): $violators",
            violators.isEmpty(),
        )
    }

    @Test
    fun `PermissionsSettingsActivity does not use bare IdeSuccess in code`() {
        val violators = codeLines.filter { it.contains("IdeSuccess") }
        assertTrue(
            "Found bare IdeSuccess usage in non-import lines (use c.success from LocalIdeColors.current): $violators",
            violators.isEmpty(),
        )
    }

    @Test
    fun `PermissionsSettingsActivity does not use bare IdeBorder in code`() {
        val violators = codeLines.filter { it.contains("IdeBorder") }
        assertTrue(
            "Found bare IdeBorder usage in non-import lines (use c.border from LocalIdeColors.current): $violators",
            violators.isEmpty(),
        )
    }

    @Test
    fun `PermissionsSettingsActivity does not use bare IdeDanger in code`() {
        val violators = codeLines.filter { it.contains("IdeDanger") }
        assertTrue(
            "Found bare IdeDanger usage in non-import lines (use c.danger from LocalIdeColors.current): $violators",
            violators.isEmpty(),
        )
    }

    @Test
    fun `PermissionsSettingsActivity references LocalIdeColors`() {
        val hasLocalIdeColors = sourceFile.readLines().any { it.contains("LocalIdeColors") }
        assertTrue(
            "PermissionsSettingsActivity.kt must reference LocalIdeColors.current for themed colors",
            hasLocalIdeColors,
        )
    }
}
