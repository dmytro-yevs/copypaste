package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShizukuSettingsTest {
    @Test
    fun everyLifecycleCompletesOnceAndReleasesItsResources() {
        ShizukuSettingsResult.entries.forEach { firstResult ->
            var timeoutCancellations = 0
            var serviceReleases = 0
            val completions = mutableListOf<Boolean>()
            val gate = ShizukuSettingsCompletion(
                cancelTimeout = { timeoutCancellations++ },
                releaseService = { serviceReleases++ },
                completion = completions::add,
            )

            gate.complete(firstResult)
            ShizukuSettingsResult.entries.forEach(gate::complete)

            assertEquals(firstResult.name, listOf(firstResult.changed), completions)
            assertEquals(firstResult.name, 1, timeoutCancellations)
            assertEquals(firstResult.name, 1, serviceReleases)
        }
    }

    @Test
    fun onlyConnectedChangedSucceeds() {
        assertTrue(ShizukuSettingsResult.CONNECTED_CHANGED.changed)
        ShizukuSettingsResult.entries
            .filterNot { it == ShizukuSettingsResult.CONNECTED_CHANGED }
            .forEach { assertFalse(it.name, it.changed) }
    }

    @Test
    fun teardownFailureStillCompletesOnce() {
        val completions = mutableListOf<Boolean>()
        val gate = ShizukuSettingsCompletion(
            cancelTimeout = {},
            releaseService = { throw IllegalStateException("Shizuku stopped") },
            completion = completions::add,
        )

        gate.complete(ShizukuSettingsResult.DISCONNECTED)
        gate.complete(ShizukuSettingsResult.CONNECTED_CHANGED)

        assertEquals(listOf(false), completions)
    }

    @Test
    fun timeoutIsShortAndExplicit() {
        assertEquals(3_000L, SHIZUKU_SETTINGS_TIMEOUT_MILLIS)
    }
}
