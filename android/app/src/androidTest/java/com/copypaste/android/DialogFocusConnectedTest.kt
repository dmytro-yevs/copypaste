package com.copypaste.android

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isFocusable
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Rule
import org.junit.Test

/**
 * CopyPaste-myh8.15 — S15 focus-management check for DevicesDialogs'
 * unpair confirm dialog (`UnpairConfirmDialog` in DevicesDialogs.kt, gated by
 * `controller.revoke.unpairTarget`): on open, focus must land inside the
 * dialog's own semantics subtree (`isDialog()` ancestor); on dismiss the
 * trigger button must remain a focus target (`isFocusable()`) and displayed
 * so the user's next Tab/D-pad/TalkBack step lands back on it.
 *
 * Verified hermetic-enough for a connected test by reading DevicesDialogs.kt
 * + DevicesController.kt first: [DevicesController]'s constructor only wires
 * `Settings(context)`/`DeviceKeyStore(context)` (both plain SharedPreferences
 * reads at construction — see DeviceKeyStore.kt kdoc, the Rust FFI
 * `generateDeviceCert` call is lazy, inside `getOrCreate()`, never invoked
 * here) and this test never calls a method that reaches native/FFI code —
 * only `controller.revoke.unpairTarget` (a `mutableStateOf` on
 * [DevicesRevokeActions]) is mutated to open [UnpairConfirmDialog].
 *
 * FIXED (CopyPaste-myh8.15): [com.copypaste.android.ui.theme.GlassAlertDialog]
 * now requests focus onto its title on first composition (FocusRequester +
 * Modifier.focusRequester/.focusable + LaunchedEffect(Unit)), so opening
 * [UnpairConfirmDialog] moves accessibility focus onto a node inside the
 * dialog's semantics subtree — every `GlassAlertDialog`-based confirm dialog
 * benefits, not just this one.
 */
class DialogFocusConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val triggerLabel = "Open unpair dialog"

    private val fixturePeer = PairedPeer(
        fingerprint = "0123456789abcdef".repeat(4),
        syncAddr = "",
        name = "Test Mac",
        sessionKeyWrappedB64 = "",
        sessionKeyIvB64 = "",
        peerModel = "MacBook Air (M3)",
        peerOs = "macOS 15.3",
        peerAppVersion = "0.5.3",
        peerLocalIp = "10.0.0.5",
        latencyMs = null,
        sasVerified = true,
    )

    private fun setContentWithTriggerAndDialog(onController: (DevicesController) -> Unit) {
        composeRule.setContent {
            val activity = composeRule.activity
            // CopyPaste-myh8 gate: lint's RememberReturnType check misresolves this
            // identical, already-widespread `remember { MainSourceSetClass(ctx) }`
            // pattern (see HistoryList.kt/PairScreen.kt/etc.) as Unit-returning ONLY
            // when the call site lives in the androidTest source set — a lint
            // cross-sourceSet UAST resolution false positive, not a real Unit remember.
            @Suppress("RememberReturnType")
            val settings: Settings = remember { Settings(activity) }
            @Suppress("RememberReturnType")
            val deviceKeyStore: DeviceKeyStore = remember { DeviceKeyStore(activity) }
            val scope = rememberCoroutineScope()
            @Suppress("RememberReturnType")
            val controller: DevicesController =
                remember { DevicesController(activity, settings, deviceKeyStore, scope) }
            onController(controller)

            CopyPasteTheme(isDark = true) {
                Column {
                    Button(onClick = { controller.revoke.unpairTarget = fixturePeer }) {
                        Text(triggerLabel)
                    }
                    DevicesDialogs(controller = controller, settings = settings)
                }
            }
        }
    }

    @Test
    fun openingTheUnpairConfirmDialogMovesFocusInsideIt() {
        setContentWithTriggerAndDialog {}

        composeRule.onNodeWithText(triggerLabel).performClick()
        composeRule.waitForIdle()

        composeRule.onNode(isFocused() and hasAnyAncestor(isDialog()))
            .assertExists("expected a focused node inside the dialog's semantics subtree on open")
    }

    @Test
    fun dismissingTheDialogLeavesTheTriggerDisplayedAndFocusable() {
        setContentWithTriggerAndDialog {}

        composeRule.onNodeWithText(triggerLabel).performClick()
        composeRule.waitForIdle()

        composeRule.onNodeWithText(composeRule.activity.getString(R.string.dialog_cancel))
            .performClick()
        composeRule.waitForIdle()

        composeRule.onNodeWithText(triggerLabel)
            .assertIsDisplayed()
            .assert(isFocusable())
    }
}
