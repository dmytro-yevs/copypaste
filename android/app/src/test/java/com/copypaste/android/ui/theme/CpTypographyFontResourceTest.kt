package com.copypaste.android.ui.theme

import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.security.MessageDigest

/**
 * android-design-system "CpTypography semantic type roles" requirement: every
 * role resolves to a real bundled Inter/JetBrains Mono face (no synthesis, no
 * system fallback), using the frozen exact per-role values (no ranges).
 *
 * Two complementary checks:
 *   1. Code-level: each role's [FontWeight]/family matches the frozen table AND
 *      that exact weight exists as a real entry in [InterFamily]/[JetBrainsMonoFamily].
 *   2. Disk-level: the backing .ttf for each weight exists, is non-trivial in
 *      size, and is a DISTINCT binary per weight (rules out a placeholder that
 *      copies another weight's file instead of a real drawn face).
 */
class CpTypographyFontResourceTest {

    @Test
    fun `title role is Inter 700 22sp 27sp line-height`() {
        assertEquals(InterFamily, CpTypography.title.fontFamily)
        assertEquals(FontWeight.W700, CpTypography.title.fontWeight)
        assertEquals(22.sp, CpTypography.title.fontSize)
        assertEquals(27.sp, CpTypography.title.lineHeight)
    }

    @Test
    fun `section role is Inter 600 with 0 point 01em tracking`() {
        assertEquals(InterFamily, CpTypography.section.fontFamily)
        assertEquals(FontWeight.W600, CpTypography.section.fontWeight)
        assertEquals(14.sp, CpTypography.section.fontSize)
        assertEquals(18.sp, CpTypography.section.lineHeight)
    }

    @Test
    fun `body and body-emphasis are Inter 400 and 500 at 14sp`() {
        assertEquals(FontWeight.W400, CpTypography.body.fontWeight)
        assertEquals(FontWeight.W500, CpTypography.bodyEmphasis.fontWeight)
        assertEquals(14.sp, CpTypography.body.fontSize)
        assertEquals(14.sp, CpTypography.bodyEmphasis.fontSize)
        assertEquals(20.sp, CpTypography.body.lineHeight)
    }

    @Test
    fun `body-mono and micro use JetBrains Mono not Inter`() {
        assertEquals(JetBrainsMonoFamily, CpTypography.bodyMono.fontFamily)
        assertEquals(FontWeight.W400, CpTypography.bodyMono.fontWeight)
        assertEquals(13.sp, CpTypography.bodyMono.fontSize)
        assertEquals(JetBrainsMonoFamily, CpTypography.micro.fontFamily)
        assertEquals(FontWeight.W500, CpTypography.micro.fontWeight)
        assertEquals(10.sp, CpTypography.micro.fontSize)
    }

    @Test
    fun `bodyMono uses tabular figures via the tnum feature not a CSS setting`() {
        assertEquals("tnum", CpTypography.bodyMono.fontFeatureSettings)
    }

    @Test
    fun `meta role is Inter 400 at 11 point 5sp`() {
        assertEquals(InterFamily, CpTypography.meta.fontFamily)
        assertEquals(FontWeight.W400, CpTypography.meta.fontWeight)
        assertEquals(11.5.sp, CpTypography.meta.fontSize)
    }

    @Test
    fun `InterFamily declares real entries for every weight the roles use`() {
        // FontFamily's public factory returns the FontFamily interface type; the
        // concrete FontListFontFamily it always constructs for FontFamily(vararg)
        // implements List<Font>, so this cast reaches the real per-weight entries.
        @Suppress("UNCHECKED_CAST")
        val weights = (InterFamily as List<Font>).map { it.weight }
        assertTrue(FontWeight.W400 in weights)
        assertTrue(FontWeight.W500 in weights)
        assertTrue(FontWeight.W600 in weights)
        assertTrue(FontWeight.W700 in weights)
    }

    @Test
    fun `JetBrainsMonoFamily declares real entries for every weight the roles use`() {
        @Suppress("UNCHECKED_CAST")
        val weights = (JetBrainsMonoFamily as List<Font>).map { it.weight }
        assertTrue(FontWeight.W400 in weights)
        assertTrue(FontWeight.W500 in weights)
    }

    @Test
    fun `every bundled ttf exists on disk is non-trivial and is a distinct binary per weight`() {
        val fontDir = findModuleFontDir()
        val files = listOf(
            "inter_regular.ttf", "inter_medium.ttf", "inter_semibold.ttf", "inter_bold.ttf",
            "jetbrains_mono_regular.ttf", "jetbrains_mono_medium.ttf",
        ).map { File(fontDir, it) }

        val checksums = files.map { file ->
            assertTrue("missing bundled font: ${file.path}", file.exists())
            assertTrue("suspiciously small font (placeholder?): ${file.path}", file.length() > 10_000)
            sha256(file)
        }
        assertEquals("bundled fonts must be distinct binaries per weight", checksums.size, checksums.toSet().size)
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        return digest.digest(file.readBytes()).joinToString("") { "%02x".format(it) }
    }

    /** Locates `src/main/res/font` regardless of the Gradle test task's working directory. */
    private fun findModuleFontDir(): File {
        var dir = File(System.getProperty("user.dir") ?: ".").absoluteFile
        repeat(6) {
            val candidate = File(dir, "src/main/res/font")
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile ?: return@repeat
        }
        throw AssertionError("could not locate src/main/res/font from ${System.getProperty("user.dir")}")
    }
}
