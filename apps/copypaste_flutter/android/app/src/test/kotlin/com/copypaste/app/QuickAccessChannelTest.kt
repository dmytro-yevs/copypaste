package com.copypaste.app

import android.app.Application
import android.app.StatusBarManager
import android.content.ComponentName
import android.provider.Settings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class QuickAccessChannelTest {
    private lateinit var app: Application

    @Before fun setup() {
        app = RuntimeEnvironment.getApplication()
        app.getSharedPreferences("copypaste_quick_access", 0).edit().clear().commit()
    }

    @Test fun panelOptionsDefaultToAutoReturnAndPersistTheUserChoice() {
        assertEquals(mapOf("autoReturn" to true, "focusSearchOnOpen" to false), QuickAccessPreferences.asMap(app))
        assertTrue(QuickAccessPreferences.setEnabled(app, QuickAccessPreferences.PanelOption.AUTO_RETURN, false))
        assertEquals(
            mapOf("autoReturn" to false, "focusSearchOnOpen" to false),
            QuickAccessStatus.read(app)["panelOptions"],
        )
    }

    @Test fun reportsTheClipsKeyboardAsCurrentWhenItIsTheDefaultInputMethod() {
        val keyboard = ComponentName(app, ClipboardInputMethodService::class.java)
        Settings.Secure.putString(app.contentResolver, Settings.Secure.DEFAULT_INPUT_METHOD, keyboard.flattenToShortString())

        val state = QuickAccessStatus.read(app)

        assertEquals(true, state["keyboardSelected"])
        assertEquals(false, state["backgroundCapture"])
        assertEquals(true, state["tileRequestSupported"])
        assertEquals(0, state["widgetCount"])
    }

    @Test fun tileRequestErrorsMeanThePromptWasUnavailable() {
        assertEquals(TILE_ADDED, tileRequestOutcome(StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ADDED))
        assertEquals(TILE_NOT_ADDED, tileRequestOutcome(StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_NOT_ADDED))
        assertEquals(
            TILE_UNAVAILABLE,
            tileRequestOutcome(StatusBarManager.TILE_ADD_REQUEST_ERROR_APP_NOT_IN_FOREGROUND),
        )
    }
}
