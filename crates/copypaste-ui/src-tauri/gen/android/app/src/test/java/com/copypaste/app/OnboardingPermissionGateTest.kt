package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class OnboardingPermissionGateTest {
    @Test
    fun firstAskOnApi33IsAPromptEvenWithoutRationale() {
        assertEquals(
            NotificationAsk.PROMPT,
            OnboardingPermissionGate.notifications(
                apiLevel = 33,
                tiramisu = 33,
                granted = false,
                everAsked = false,
                showRationale = false,
            ),
        )
    }

    @Test
    fun aPermanentDenialOpensSettingsInsteadOfThePrompt() {
        assertEquals(
            NotificationAsk.DENIED,
            OnboardingPermissionGate.notifications(
                apiLevel = 33,
                tiramisu = 33,
                granted = false,
                everAsked = true,
                showRationale = false,
            ),
        )
    }

    @Test
    fun preTiramisuHasNoRuntimeNotificationPrompt() {
        assertEquals(
            NotificationAsk.NOT_REQUIRED,
            OnboardingPermissionGate.notifications(
                apiLevel = 32,
                tiramisu = 33,
                granted = false,
                everAsked = false,
                showRationale = false,
            ),
        )
    }
}
