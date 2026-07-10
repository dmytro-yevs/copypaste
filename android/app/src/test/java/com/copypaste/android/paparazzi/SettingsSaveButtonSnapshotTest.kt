package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.R
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// S9 Wave 5 — SettingsActivity's header Save button (CopyPaste-65x6 "sole Save
// affordance"), covering disabled(clean)/enabled(dirty)/saved-transient
// (Wave 3 justSaved). The button lives inline inside SettingsScreenContent's
// Scaffold.topBar { CopyPasteTopBar(actions = { ... }) } block, which reads
// `dirty`/`justSaved` off SettingsActivity's own remember-ed state and is not
// extractable as a standalone function without instantiating the whole
// activity/Context-bound composable (needs Settings/AppearanceStore, native
// FFI, etc — unavailable in this Paparazzi JVM host). So this fixture calls
// the SAME shared building blocks ([CopyPasteTopBar]/[CopyPasteButton]) with
// literal dirty/justSaved fixture values instead, reproducing the button's
// EXACT code path from SettingsActivity.kt's topBar block verbatim — mirroring
// DevicesCardSnapshotTest's DevicesDialogFixture "representative golden"
// convention. Smell flag: this is an inline copy of SettingsActivity's Save
// button branch (enabled = dirty; text = if (justSaved && !dirty) "Saved" else
// "Save"), not a call into production code — if that branch changes, this
// fixture must be updated in lockstep.
// ---------------------------------------------------------------------------
class SettingsSaveButtonSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, save button disabled clean`() {
        paparazzi.snapshot {
            SaveButtonFixture(dirty = false, justSaved = false)
        }
    }

    @Test
    fun `dark theme, save button enabled dirty`() {
        paparazzi.snapshot {
            SaveButtonFixture(dirty = true, justSaved = false)
        }
    }

    @Test
    fun `dark theme, save button saved transient`() {
        paparazzi.snapshot {
            SaveButtonFixture(dirty = false, justSaved = true)
        }
    }
}

@Composable
private fun SaveButtonFixture(dirty: Boolean, justSaved: Boolean) {
    CopyPasteTheme(isDark = true) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(LocalCpColors.current.bg),
        ) {
            CopyPasteTopBar(
                title = stringResource(R.string.title_settings),
                actions = {
                    CopyPasteButton(
                        onClick = {},
                        variant = ButtonVariant.PRIMARY,
                        enabled = dirty,
                    ) {
                        Text(
                            text = if (justSaved && !dirty)
                                stringResource(R.string.btn_save_saved)
                            else
                                stringResource(R.string.btn_save),
                        )
                    }
                },
            )
        }
    }
}
