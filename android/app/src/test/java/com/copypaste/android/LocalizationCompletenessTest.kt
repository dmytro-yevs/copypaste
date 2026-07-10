package com.copypaste.android

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertTrue
import org.junit.Test
import org.w3c.dom.Element

/**
 * CopyPaste-myh8.13 S13 Wave (e): JVM gate mirroring
 * scripts/check-l10n-completeness.mjs's EN -> UK coverage check (now
 * blocking, see that script's header), plus two checks the Node script does
 * not do: plural-category completeness and placeholder-multiset parity per
 * key. Follows SplashThemeTest's XML-parse pattern (no Android runtime):
 * Robolectric here cannot resolve merged resources without
 * `isIncludeAndroidResources`, and turning that on module-wide crashes
 * CopyPasteApp via WorkManager (see SplashThemeTest kdoc) — so this parses
 * the actual res/values*.xml source files directly.
 */
class LocalizationCompletenessTest {

    private fun projectFile(relative: String): File {
        val file = File(relative)
        assertTrue("expected $relative to exist relative to the :app module dir", file.exists())
        return file
    }

    private fun parseXml(file: File): Element =
        DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file).documentElement

    /** EN <string> keys that ship as translatable (translatable="false" excluded), across
     * strings.xml + the per-slice strings_s5..s8.xml files (S6/S7/S8 file-ownership
     * partition — Android merges every res/values XML file into one pool at build time). */
    private fun enStringKeys(): Set<String> {
        val keys = mutableSetOf<String>()
        for (file in enStringsFiles()) {
            val root = parseXml(file)
            val strings = root.getElementsByTagName("string")
            for (i in 0 until strings.length) {
                val el = strings.item(i) as Element
                if (el.getAttribute("translatable") == "false") continue
                keys.add(el.getAttribute("name"))
            }
        }
        return keys
    }

    private fun enStringsFiles(): List<File> = listOf(
        projectFile("src/main/res/values/strings.xml"),
        projectFile("src/main/res/values/strings_s5.xml"),
        projectFile("src/main/res/values/strings_s6.xml"),
        projectFile("src/main/res/values/strings_s7.xml"),
        projectFile("src/main/res/values/strings_s8.xml"),
    )

    private fun ukStringsFile(): File = projectFile("src/main/res/values-uk/strings.xml")

    private fun ukStringKeys(): Set<String> {
        val keys = mutableSetOf<String>()
        val root = parseXml(ukStringsFile())
        val strings = root.getElementsByTagName("string")
        for (i in 0 until strings.length) {
            keys.add((strings.item(i) as Element).getAttribute("name"))
        }
        return keys
    }

    /** name -> quantity -> item text, for every <plurals> in the given file. */
    private fun pluralsByName(file: File): Map<String, Map<String, String>> {
        val root = parseXml(file)
        val plurals = root.getElementsByTagName("plurals")
        val result = mutableMapOf<String, Map<String, String>>()
        for (i in 0 until plurals.length) {
            val el = plurals.item(i) as Element
            val name = el.getAttribute("name")
            val items = el.getElementsByTagName("item")
            val byQuantity = mutableMapOf<String, String>()
            for (j in 0 until items.length) {
                val item = items.item(j) as Element
                byQuantity[item.getAttribute("quantity")] = item.textContent
            }
            result[name] = byQuantity
        }
        return result
    }

    private fun enPlurals(): Map<String, Map<String, String>> {
        val merged = mutableMapOf<String, Map<String, String>>()
        for (file in enStringsFiles()) merged.putAll(pluralsByName(file))
        return merged
    }

    private fun ukPlurals(): Map<String, Map<String, String>> = pluralsByName(ukStringsFile())

    /** Android format-string placeholders, e.g. %1$s, %2$d, %s — order-insensitive multiset. */
    private fun placeholders(text: String): List<String> =
        Regex("%(\\d+\\$)?[sdf]").findAll(text).map { it.value }.toList().sorted()

    @Test
    fun `every translatable EN string key has a UK counterpart`() {
        val en = enStringKeys()
        val uk = ukStringKeys()
        val missing = (en - uk).sorted()
        assertTrue(
            "values-uk/strings.xml is missing ${missing.size} translatable EN key(s): " +
                missing.joinToString(", "),
            missing.isEmpty(),
        )
    }

    @Test
    fun `every EN plurals block has a UK counterpart with one, few, many, other`() {
        val required = setOf("one", "few", "many", "other")
        val en = enPlurals()
        val uk = ukPlurals()

        val missingBlocks = (en.keys - uk.keys).sorted()
        assertTrue(
            "values-uk/strings.xml is missing ${missingBlocks.size} <plurals> block(s): " +
                missingBlocks.joinToString(", "),
            missingBlocks.isEmpty(),
        )

        val incomplete = en.keys.filter { name ->
            val ukQuantities = uk[name]?.keys ?: emptySet()
            !ukQuantities.containsAll(required)
        }.sorted()
        assertTrue(
            "these UK <plurals> blocks are missing one/few/many/other quantities: " +
                incomplete.joinToString(", ") { name ->
                    val have = uk[name]?.keys?.sorted() ?: emptyList()
                    "$name (has: $have)"
                },
            incomplete.isEmpty(),
        )
    }

    @Test
    fun `placeholder multiset matches between EN and UK for every string key`() {
        val enMap = mutableMapOf<String, String>()
        for (file in enStringsFiles()) {
            val root = parseXml(file)
            val strings = root.getElementsByTagName("string")
            for (i in 0 until strings.length) {
                val el = strings.item(i) as Element
                if (el.getAttribute("translatable") == "false") continue
                enMap[el.getAttribute("name")] = el.textContent
            }
        }
        val ukMap = mutableMapOf<String, String>()
        run {
            val root = parseXml(ukStringsFile())
            val strings = root.getElementsByTagName("string")
            for (i in 0 until strings.length) {
                val el = strings.item(i) as Element
                ukMap[el.getAttribute("name")] = el.textContent
            }
        }

        val mismatches = mutableListOf<String>()
        for ((name, enText) in enMap) {
            val ukText = ukMap[name] ?: continue // missing-key case is covered by another test
            val enPh = placeholders(enText)
            val ukPh = placeholders(ukText)
            if (enPh != ukPh) {
                mismatches.add("$name: EN=$enPh UK=$ukPh")
            }
        }
        assertTrue(
            "${mismatches.size} string key(s) have mismatched placeholders between EN and UK:\n" +
                mismatches.joinToString("\n"),
            mismatches.isEmpty(),
        )
    }

    @Test
    fun `placeholder multiset matches between EN and UK for every plurals quantity`() {
        val en = enPlurals()
        val uk = ukPlurals()

        val mismatches = mutableListOf<String>()
        for ((name, enQuantities) in en) {
            val ukQuantities = uk[name] ?: continue // missing-block case covered by another test
            for ((quantity, enText) in enQuantities) {
                val ukText = ukQuantities[quantity] ?: continue // missing-quantity covered above
                val enPh = placeholders(enText)
                val ukPh = placeholders(ukText)
                if (enPh != ukPh) {
                    mismatches.add("$name[$quantity]: EN=$enPh UK=$ukPh")
                }
            }
        }
        assertTrue(
            "${mismatches.size} plurals quantity/quantities have mismatched placeholders between EN and UK:\n" +
                mismatches.joinToString("\n"),
            mismatches.isEmpty(),
        )
    }
}
