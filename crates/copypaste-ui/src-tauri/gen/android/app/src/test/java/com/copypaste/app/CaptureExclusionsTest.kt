package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class CaptureExclusionsTest {
    @Test
    fun knownExcludedAndUnknownExternalSourcesAreRejectedAtTheReadBoundary() {
        try {
            CaptureExclusions.replace(true, listOf("com.example.private"))

            assertEquals(
                ExternalReadDecision.SKIP_EXCLUDED,
                CaptureExclusions.decide("COM.EXAMPLE.PRIVATE"),
            )
            assertEquals(
                ExternalReadDecision.SKIP_UNKNOWN,
                CaptureExclusions.decide(null),
            )
            assertEquals(
                ExternalReadDecision.READ,
                CaptureExclusions.decide("com.example.writer"),
            )
        } finally {
            CaptureExclusions.replace(false, emptyList())
        }
    }
}
