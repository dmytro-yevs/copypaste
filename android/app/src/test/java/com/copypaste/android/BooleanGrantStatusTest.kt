package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [booleanGrantStatus] — the pure Boolean -> [PermissionStatus]
 * mapping for special-access grants with no rationale/permanent-denial concept
 * (overlay, battery exemption). Extracted in S10 Wave D/E (CopyPaste-myh8.10)
 * from the identical inline `if (x) GRANTED else DENIED` duplicated across
 * BackgroundCaptureSetupActivity and PermissionsSettingsActivity.
 */
class BooleanGrantStatusTest {

    @Test
    fun `granted true maps to GRANTED`() {
        assertEquals(PermissionStatus.GRANTED, booleanGrantStatus(true))
    }

    @Test
    fun `granted false maps to DENIED`() {
        assertEquals(PermissionStatus.DENIED, booleanGrantStatus(false))
    }
}
