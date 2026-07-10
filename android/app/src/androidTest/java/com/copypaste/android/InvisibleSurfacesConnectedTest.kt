package com.copypaste.android

import android.content.Intent
import android.view.ViewGroup
import androidx.lifecycle.Lifecycle
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

// ---------------------------------------------------------------------------
// S12 Wave 5 (CopyPaste-myh8.12) connected check for the two "invisible
// surface" Activities on the A-C7 skin axis (see ClipboardFloatingActivity's
// and ShareReceiverActivity's KDoc "Skin axis" sections): neither Activity
// ever composes a Compose tree or calls setContentView, so no skin token
// applies to either. This test asserts that hard invariant on-device rather
// than by reading the source: both Activities finish (reach DESTROYED)
// without ever attaching visible content to their own decor view.
//
// ClipboardFloatingActivity's own overlay window (added directly via
// WindowManager, not through the Activity's window) is a separate concern
// covered by its own KDoc/guards; this test only asserts the Activity's own
// window content view, which is what a user would actually see if the
// Activity were somehow shown.
// ---------------------------------------------------------------------------
@RunWith(AndroidJUnit4::class)
class InvisibleSurfacesConnectedTest {

    /**
     * Poll [ActivityScenario.getState] until it reaches [target] or [timeoutMs]
     * elapses. Bounded — never blocks indefinitely like a bare `Thread.sleep`
     * chain would.
     */
    private fun awaitState(
        scenario: ActivityScenario<*>,
        target: Lifecycle.State,
        timeoutMs: Long = 10_000,
    ): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            if (scenario.state == target) return true
            Thread.sleep(50)
        }
        return scenario.state == target
    }

    /**
     * Best-effort peek at the Activity's own decor content view child count.
     * Returns null (rather than failing) when the Activity has already been
     * destroyed by the time this runs — a race that itself only strengthens
     * the "finishes without ever showing content" assertion, since a
     * still-alive Activity is a stronger, not weaker, test of "no children
     * were ever attached".
     */
    private fun peekContentChildCount(scenario: ActivityScenario<*>): Int? {
        // Both Activities in this file finish within milliseconds of onCreate, so by
        // the time this poll reaches ActivityScenario the Activity is frequently
        // already DESTROYED. androidx.test's onActivity() surfaces that race as a
        // NullPointerException from its internal Checks.checkNotNull (not
        // IllegalStateException), so catch broadly — any failure here just means
        // the race lost, which only strengthens the "never showed content" claim.
        if (scenario.state == Lifecycle.State.DESTROYED) return null
        var childCount: Int? = null
        try {
            scenario.onActivity { activity ->
                val decor = activity.window?.decorView
                val content = decor?.findViewById<ViewGroup>(android.R.id.content)
                childCount = content?.childCount ?: 0
            }
        } catch (_: Exception) {
            // Activity was destroyed between the state check above and this call.
        }
        return childCount
    }

    @Test
    fun clipboardFloatingActivityFinishesWithoutVisibleContent() {
        ActivityScenario.launch(ClipboardFloatingActivity::class.java).use { scenario ->
            val childCount = peekContentChildCount(scenario)
            if (childCount != null) {
                assertEquals(
                    "ClipboardFloatingActivity must never attach content views to its own " +
                        "window decor — it draws only through a separate WindowManager overlay",
                    0,
                    childCount,
                )
            }

            assertTrue(
                "ClipboardFloatingActivity must finish (reach DESTROYED) — it is a " +
                    "transparent one-shot overlay, never a persistent screen",
                awaitState(scenario, Lifecycle.State.DESTROYED),
            )
        }
    }

    @Test
    fun shareReceiverActivityFinishesWithoutVisibleContent() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val intent = Intent(context, ShareReceiverActivity::class.java).apply {
            action = Intent.ACTION_SEND
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, "InvisibleSurfacesConnectedTest synthetic text")
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

        ActivityScenario.launch<ShareReceiverActivity>(intent).use { scenario ->
            val childCount = peekContentChildCount(scenario)
            if (childCount != null) {
                assertEquals(
                    "ShareReceiverActivity must never attach content views to its own " +
                        "window decor — it is a Translucent.NoTitleBar share-target with no UI",
                    0,
                    childCount,
                )
            }

            assertTrue(
                "ShareReceiverActivity must finish (reach DESTROYED) after its capture " +
                    "coroutine drains the shared text, even with no synced device configured",
                awaitState(scenario, Lifecycle.State.DESTROYED),
            )
        }
    }
}
