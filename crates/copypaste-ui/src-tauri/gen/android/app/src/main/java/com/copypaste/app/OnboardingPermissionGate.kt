package com.copypaste.app

internal enum class NotificationAsk {
    GRANTED,
    PROMPT,
    DENIED,
    NOT_REQUIRED,
}

internal object OnboardingPermissionGate {
    fun notifications(
        apiLevel: Int,
        tiramisu: Int,
        granted: Boolean,
        everAsked: Boolean,
        showRationale: Boolean,
    ): NotificationAsk {
        if (apiLevel < tiramisu) return NotificationAsk.NOT_REQUIRED
        if (granted) return NotificationAsk.GRANTED
        return if (!everAsked || showRationale) NotificationAsk.PROMPT else NotificationAsk.DENIED
    }
}
