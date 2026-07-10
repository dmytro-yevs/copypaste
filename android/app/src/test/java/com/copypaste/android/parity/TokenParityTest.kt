package com.copypaste.android.parity

import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.CpMotion
import com.copypaste.android.ui.theme.CpShapes
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.DarkColors
import com.copypaste.android.ui.theme.LightColors
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import androidx.compose.ui.graphics.Color as ComposeColor

/**
 * android-material3-redesign task 2.11 "Cross-platform parity gate": asserts
 * [DarkColors]/[LightColors]/[AccentColor] equal the canonical
 * `parity/tokens.json` (generated from `crates/copypaste-ui/src/styles/tokens.css`
 * at the pinned desktop commit 6960539d by `scripts/gen-parity-tokens.mjs` —
 * see cross-platform-parity.md "Canonical machine-readable token source").
 * The web side of this same check is desktop-epic-owned.
 *
 * Radii/spacing/motion durations are asserted too (task 2.11's traceability
 * row "Radii/spacing/elevation/motion/easing ... Exact ... TokenParityTest") —
 * these have a direct 1:1 tokens.css counterpart, unlike typography (see
 * gen-parity-tokens.mjs's header comment for why typography is NOT
 * cross-checked here).
 */
class TokenParityTest {

    private val tokens: JSONObject by lazy { loadParityTokens() }

    private fun loadParityTokens(): JSONObject {
        var dir = File(".").absoluteFile
        repeat(8) {
            val candidate = File(dir, "parity/tokens.json")
            if (candidate.exists()) return JSONObject(candidate.readText())
            dir = dir.parentFile ?: return@repeat
        }
        throw AssertionError(
            "parity/tokens.json not found by walking up from ${File(".").absolutePath} — " +
                "run `node scripts/gen-parity-tokens.mjs` from the repo root first.",
        )
    }

    private fun hex(rgba: String): ComposeColor {
        val clean = rgba.trim().removePrefix("#")
        return when (clean.length) {
            3 -> { // CSS shorthand, e.g. "fff" -> "ffffff"
                val expanded = clean.map { "$it$it" }.joinToString("")
                ComposeColor(0xFF000000 or expanded.toLong(16))
            }
            6 -> ComposeColor(0xFF000000 or clean.toLong(16))
            8 -> ComposeColor(clean.toLong(16))
            else -> error("unexpected hex length: $rgba")
        }
    }

    private fun assertColorField(json: JSONObject, key: String, actual: ComposeColor) {
        assertEquals("token '$key'", hex(json.getString(key)), actual)
    }

    /** Parses CSS `rgba(r,g,b,a)` (0-255 channels, 0-1 alpha) into an ARGB [ComposeColor]. */
    private fun rgba(css: String): ComposeColor {
        val nums = css.trim().removePrefix("rgba(").removeSuffix(")").split(",").map { it.trim().toFloat() }
        val (r, g, b, a) = nums
        val argb = (Math.round(a * 255) shl 24) or (r.toInt() shl 16) or (g.toInt() shl 8) or b.toInt()
        return ComposeColor(argb.toLong() and 0xFFFFFFFFL)
    }

    private fun assertOverlayField(json: JSONObject, key: String, actual: ComposeColor) {
        assertEquals("overlay token '$key'", rgba(json.getString(key)), actual)
    }

    @Test
    fun `dark theme surfaces lines text overlays status content match tokens json`() {
        val dark = tokens.getJSONObject("theme").getJSONObject("dark")
        val surfaces = dark.getJSONObject("surfaces")
        assertColorField(surfaces, "bg", DarkColors.bg)
        assertColorField(surfaces, "panel", DarkColors.panel)
        assertColorField(surfaces, "elevated", DarkColors.elevated)
        assertColorField(surfaces, "card", DarkColors.card)
        assertColorField(surfaces, "raised", DarkColors.raised)
        assertColorField(surfaces, "raised2", DarkColors.raised2)

        val lines = dark.getJSONObject("lines")
        assertColorField(lines, "border", DarkColors.border)
        assertColorField(lines, "divider", DarkColors.divider)

        val text = dark.getJSONObject("text")
        assertColorField(text, "text", DarkColors.text)
        assertColorField(text, "dim", DarkColors.dim)
        assertColorField(text, "faint", DarkColors.faint)
        assertColorField(text, "mute", DarkColors.mute)

        val overlays = dark.getJSONObject("overlays")
        assertOverlayField(overlays, "hover", DarkColors.hover)
        assertOverlayField(overlays, "pressed", DarkColors.pressed)
        assertOverlayField(overlays, "scrim", DarkColors.scrim)

        val status = dark.getJSONObject("status")
        assertColorField(status, "ok", DarkColors.ok)
        assertColorField(status, "warn", DarkColors.warn)
        assertColorField(status, "err", DarkColors.err)
        assertColorField(status, "info", DarkColors.info)

        val statusStrong = dark.getJSONObject("statusStrong")
        assertColorField(statusStrong, "okStrong", DarkColors.okStrong)
        assertColorField(statusStrong, "errStrong", DarkColors.errStrong)
        assertColorField(statusStrong, "infoStrong", DarkColors.infoStrong)

        val content = dark.getJSONObject("content")
        assertColorField(content, "cText", DarkColors.cText)
        assertColorField(content, "cUrl", DarkColors.cUrl)
        assertColorField(content, "cCode", DarkColors.cCode)
        assertColorField(content, "cImage", DarkColors.cImage)
        assertColorField(content, "cMail", DarkColors.cMail)
        assertColorField(content, "cColor", DarkColors.cColor)
        assertColorField(content, "cNum", DarkColors.cNum)
        // PATH aliases to cFile on Android (D1 override #1 — no distinct cPath field);
        // tokens.css's cPath and cFile are the same literal value in both themes, so
        // this stays a genuine equality, not a divergence.
        assertColorField(content, "cFile", DarkColors.cFile)
        assertColorField(content, "cJson", DarkColors.cJson)
        assertColorField(content, "cSecret", DarkColors.cSecret)
    }

    @Test
    fun `light theme surfaces lines text overlays status content match tokens json`() {
        val light = tokens.getJSONObject("theme").getJSONObject("light")
        val surfaces = light.getJSONObject("surfaces")
        assertColorField(surfaces, "bg", LightColors.bg)
        assertColorField(surfaces, "panel", LightColors.panel)
        assertColorField(surfaces, "elevated", LightColors.elevated)
        assertColorField(surfaces, "card", LightColors.card)
        assertColorField(surfaces, "raised", LightColors.raised)
        assertColorField(surfaces, "raised2", LightColors.raised2)

        val lines = light.getJSONObject("lines")
        assertColorField(lines, "border", LightColors.border)
        assertColorField(lines, "divider", LightColors.divider)

        val text = light.getJSONObject("text")
        assertColorField(text, "text", LightColors.text)
        assertColorField(text, "dim", LightColors.dim)
        assertColorField(text, "faint", LightColors.faint)
        assertColorField(text, "mute", LightColors.mute)

        val overlays = light.getJSONObject("overlays")
        assertOverlayField(overlays, "hover", LightColors.hover)
        assertOverlayField(overlays, "pressed", LightColors.pressed)
        assertOverlayField(overlays, "scrim", LightColors.scrim)

        val status = light.getJSONObject("status")
        assertColorField(status, "ok", LightColors.ok)
        assertColorField(status, "warn", LightColors.warn)
        assertColorField(status, "err", LightColors.err)
        assertColorField(status, "info", LightColors.info)

        val statusStrong = light.getJSONObject("statusStrong")
        assertColorField(statusStrong, "okStrong", LightColors.okStrong)
        assertColorField(statusStrong, "errStrong", LightColors.errStrong)
        assertColorField(statusStrong, "infoStrong", LightColors.infoStrong)

        val content = light.getJSONObject("content")
        assertColorField(content, "cText", LightColors.cText)
        assertColorField(content, "cUrl", LightColors.cUrl)
        assertColorField(content, "cCode", LightColors.cCode)
        assertColorField(content, "cImage", LightColors.cImage)
        assertColorField(content, "cMail", LightColors.cMail)
        assertColorField(content, "cColor", LightColors.cColor)
        assertColorField(content, "cNum", LightColors.cNum)
        assertColorField(content, "cFile", LightColors.cFile)
        assertColorField(content, "cJson", LightColors.cJson)
        assertColorField(content, "cSecret", LightColors.cSecret)
    }

    /**
     * Accent base (dark/light)/variant are Exact per cross-platform-parity.md;
     * on-accent text is Exact for 7 of 12 cells and a documented, approved AA
     * divergence for the other 5 (Color.kt's kdoc: dark blue/rose and light
     * teal/green/amber white on-accent measured below AA — darkened locally).
     * This test asserts BOTH: the 7 matching cells equal tokens.json, and the
     * divergence is EXACTLY those 5 documented cells (not a silent drift).
     */
    @Test
    fun `accent base and variant match tokens json — on-accent matches except the 5 documented AA overrides`() {
        val accents = tokens.getJSONArray("accents")
        val documentedOnAccentOverrides = setOf(
            "blue" to false, // dark blue
            "rose" to false, // dark rose
            "teal" to true, // light teal
            "green" to true, // light green
            "amber" to true, // light amber
        )
        var overrideCount = 0
        for (i in 0 until accents.length()) {
            val row = accents.getJSONObject(i)
            val name = row.getString("name")
            val accent = AccentColor.valueOf(name.uppercase())

            assertColorField(row, "dark", accent.dark)
            assertColorField(row, "light", accent.light)
            assertColorField(row, "variant", accent.variant)

            val jsonOnAccentDark = hex(row.getString("onAccent"))
            val jsonOnAccentLight = hex(row.getString("onAccentLight"))
            val isDarkOverride = (name to false) in documentedOnAccentOverrides
            val isLightOverride = (name to true) in documentedOnAccentOverrides

            if (isDarkOverride) {
                assertTrue(
                    "$name dark onAccent expected to diverge (documented AA override) but matched",
                    accent.onDark != jsonOnAccentDark,
                )
                overrideCount++
            } else {
                assertEquals("$name dark onAccent", jsonOnAccentDark, accent.onDark)
            }
            if (isLightOverride) {
                assertTrue(
                    "$name light onAccent expected to diverge (documented AA override) but matched",
                    accent.onLight != jsonOnAccentLight,
                )
                overrideCount++
            } else {
                assertEquals("$name light onAccent", jsonOnAccentLight, accent.onLight)
            }
        }
        assertEquals("expected exactly 5 documented on-accent AA overrides", 5, overrideCount)
    }

    @Test
    fun `radii match tokens json`() {
        val radii = tokens.getJSONObject("radii")
        assertEquals(radii.getString("chip"), "${CpShapes.chip.value.toInt()}px")
        assertEquals(radii.getString("ctl"), "${CpShapes.ctl.value.toInt()}px")
        assertEquals(radii.getString("input"), "${CpShapes.input.value.toInt()}px")
        assertEquals(radii.getString("card"), "${CpShapes.card.value.toInt()}px")
        assertEquals(radii.getString("pill"), "${CpShapes.pill.value.toInt()}px")
    }

    @Test
    fun `spacing scale matches tokens json`() {
        val spacing = tokens.getJSONObject("spacing")
        assertEquals(spacing.getString("s1"), "${CpSpacing.s1.value.toInt()}px")
        assertEquals(spacing.getString("s2"), "${CpSpacing.s2.value.toInt()}px")
        assertEquals(spacing.getString("s3"), "${CpSpacing.s3.value.toInt()}px")
        assertEquals(spacing.getString("s4"), "${CpSpacing.s4.value.toInt()}px")
        assertEquals(spacing.getString("s5"), "${CpSpacing.s5.value.toInt()}px")
        assertEquals(spacing.getString("s6"), "${CpSpacing.s6.value.toInt()}px")
        assertEquals(spacing.getString("s7"), "${CpSpacing.s7.value.toInt()}px")
        assertEquals(spacing.getString("s8"), "${CpSpacing.s8.value.toInt()}px")
        assertEquals(spacing.getString("s9"), "${CpSpacing.s9.value.toInt()}px")
    }

    @Test
    fun `motion durations match tokens json`() {
        val durations = tokens.getJSONObject("motion").getJSONObject("durations")
        assertEquals(durations.getString("fast"), "${CpMotion.FAST_MS}ms")
        assertEquals(durations.getString("default"), "${CpMotion.DEFAULT_MS}ms")
        assertEquals(durations.getString("theme"), "${CpMotion.THEME_MS}ms")
    }

    @Test
    fun `parity json is non-empty and pinned to the frozen desktop commit`() {
        assertNotNull(tokens)
        assertEquals("6960539d", tokens.getString("sourceCommit"))
        assertEquals(6, tokens.getJSONArray("accents").length())
    }
}
