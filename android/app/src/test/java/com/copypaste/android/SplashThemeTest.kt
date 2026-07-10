package com.copypaste.android

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.w3c.dom.Element

/**
 * task 0.10/S12 "System-bar + first-paint" part 2 (design.md ~254-263,
 * absorbed S1 deliverable): resource-level characterization for the splash
 * wiring.
 *
 * Robolectric-without-AGP's-`includeAndroidResources` resolves the manifest
 * to a synthetic "org.robolectric.default" package with no activities/styles
 * (verified empirically), so a PackageManager/Resources-based test is not
 * reachable here. Turning `isIncludeAndroidResources` on module-wide was
 * tried and reverted: it makes Robolectric instantiate the REAL
 * [CopyPasteApp] (not the default stub `Application`) for every existing
 * Robolectric test, and `CopyPasteApp.onCreate` crashes with
 * `IllegalStateException: WorkManager is not initialized properly` outside
 * a real Android process — a suite-wide regression risk out of scope for
 * this slice. Instead these tests parse the actual manifest/themes XML
 * source files directly (no Android runtime needed at all), which is a
 * faithful, if lower-level, characterization of the same facts.
 */
class SplashThemeTest {

    private fun projectFile(relative: String): File {
        val file = File(relative)
        assertTrue("expected $relative to exist relative to the :app module dir", file.exists())
        return file
    }

    private fun parseXml(file: File): Element =
        DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file).documentElement

    @Test
    fun `MainActivity manifest theme is Theme_CopyPaste_Splash`() {
        val manifest = parseXml(projectFile("src/main/AndroidManifest.xml"))
        val activities = manifest.getElementsByTagName("activity")

        var mainActivityTheme: String? = null
        for (i in 0 until activities.length) {
            val activity = activities.item(i) as Element
            if (activity.getAttribute("android:name") == ".MainActivity") {
                mainActivityTheme = activity.getAttribute("android:theme")
                break
            }
        }

        assertEquals("@style/Theme.CopyPaste.Splash", mainActivityTheme)
    }

    @Test
    fun `Theme_CopyPaste_Splash postSplashScreenTheme points at Theme_CopyPaste`() {
        val themes = parseXml(projectFile("src/main/res/values/themes.xml"))
        val styles = themes.getElementsByTagName("style")

        var postSplashScreenTheme: String? = null
        for (i in 0 until styles.length) {
            val style = styles.item(i) as Element
            if (style.getAttribute("name") == "Theme.CopyPaste.Splash") {
                assertEquals("Theme.SplashScreen", style.getAttribute("parent"))
                val items = style.getElementsByTagName("item")
                for (j in 0 until items.length) {
                    val item = items.item(j) as Element
                    if (item.getAttribute("name") == "postSplashScreenTheme") {
                        postSplashScreenTheme = item.textContent.trim()
                    }
                }
                break
            }
        }

        assertEquals("@style/Theme.CopyPaste", postSplashScreenTheme)
    }
}
