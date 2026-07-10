package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [permissionCardCta] — the pure PermissionStatus -> CTA mapping
 * consumed by the shared `PermissionCard` composable (S10 Wave B, CopyPaste-myh8.10).
 * Pure function, no Context/Activity/Compose, so it is testable in the
 * :app JVM test module.
 */
class PermissionCardCtaTest {

    @Test
    fun `GRANTED maps to SATISFIED`() {
        assertEquals(PermissionCardCta.SATISFIED, permissionCardCta(PermissionStatus.GRANTED))
    }

    @Test
    fun `NOT_APPLICABLE maps to SATISFIED, same bucket as GRANTED`() {
        assertEquals(PermissionCardCta.SATISFIED, permissionCardCta(PermissionStatus.NOT_APPLICABLE))
    }

    @Test
    fun `DENIED maps to REQUEST`() {
        assertEquals(PermissionCardCta.REQUEST, permissionCardCta(PermissionStatus.DENIED))
    }

    @Test
    fun `PERMANENTLY_DENIED maps to OPEN_SETTINGS`() {
        assertEquals(PermissionCardCta.OPEN_SETTINGS, permissionCardCta(PermissionStatus.PERMANENTLY_DENIED))
    }

    @Test
    fun `every PermissionStatus value maps to the documented CTA`() {
        val expected = mapOf(
            PermissionStatus.GRANTED to PermissionCardCta.SATISFIED,
            PermissionStatus.NOT_APPLICABLE to PermissionCardCta.SATISFIED,
            PermissionStatus.DENIED to PermissionCardCta.REQUEST,
            PermissionStatus.PERMANENTLY_DENIED to PermissionCardCta.OPEN_SETTINGS,
        )
        assertEquals(
            "expected map must cover every PermissionStatus value",
            PermissionStatus.values().toSet(),
            expected.keys,
        )
        for (status in PermissionStatus.values()) {
            assertEquals(
                "unexpected CTA for $status",
                expected.getValue(status),
                permissionCardCta(status),
            )
        }
    }
}
