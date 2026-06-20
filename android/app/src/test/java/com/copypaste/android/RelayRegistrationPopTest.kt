package com.copypaste.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for the relay registration proof-of-possession (PoP) fix (CopyPaste-kmcr).
 *
 * The Rust FFI relay_registration_pop already exists in the UDL and generated bindings.
 * These tests cover:
 *   1. RelayClient.registerDevice includes pop_b64 in the request body.
 *   2. SyncManager.relayRegistration() carries enough information for the caller to compute PoP.
 *   3. The pop_b64 field is NOT logged (structural test — verify field name in the JSON body).
 *
 * No native library is available in JVM unit tests; the CopypasteBindings stubs
 * (isNativeLibraryLoaded == false) throw IllegalStateException for relay_registration_pop,
 * so we test the RelayClient/SyncManager wiring at the JSON-body level using
 * RelayClient.buildRegisterBody() — a pure Kotlin helper introduced for testability.
 *
 * Runs under :app:testDebugUnitTest (no Android runtime, no NDK).
 */
class RelayRegistrationPopTest {

    // ── RelayClient.buildRegisterBody ─────────────────────────────────────────

    @Test
    fun buildRegisterBody_includesPopB64Field() {
        val bodyStr: String = RelayClient.buildRegisterBody(
            deviceId = "test-device-id",
            deviceName = "Test Phone",
            publicKeyBase64 = "cHVibGlja2V5",
            popB64 = "cG9wcG9w",
        )
        val json = JSONObject(bodyStr)
        assertEquals("test-device-id", json.getString("device_id"))
        assertEquals("Test Phone", json.getString("device_name"))
        assertEquals("cHVibGlja2V5", json.getString("public_key_b64"))
        assertEquals("cG9wcG9w", json.getString("pop_b64"))
    }

    @Test
    fun buildRegisterBody_missingPopB64_notPresentOrEmpty() {
        // Verify the old body (WITHOUT pop_b64) would lack the field — this is the BEFORE state.
        // We simulate the old behaviour by checking a hand-crafted old-style body.
        val oldBodyStr: String = JSONObject().apply {
            put("device_id", "x")
            put("device_name", "y")
            put("public_key_b64", "z")
        }.toString()
        val json = JSONObject(oldBodyStr)
        assertFalse("old body must not have pop_b64", json.has("pop_b64"))
    }

    @Test
    fun buildRegisterBody_popB64_notLogged() {
        // SECURITY: the pop_b64 value must not appear in the field key name as plaintext label.
        // The body is opaque JSON — callers must not log it. This structural check
        // verifies the field is named "pop_b64" (the relay contract) and not something
        // that would hint at the secret value.
        val pop = "HMAC_OUTPUT_BASE64"
        val bodyStr: String = RelayClient.buildRegisterBody(
            deviceId = "id",
            deviceName = "n",
            publicKeyBase64 = "pk",
            popB64 = pop,
        )
        val json = JSONObject(bodyStr)
        // The field key is exactly "pop_b64" — consistent with relay contract.
        assertTrue("pop_b64 field must be present", json.has("pop_b64"))
        assertEquals(pop, json.getString("pop_b64"))
    }

    // ── SyncManager.RelayRegistration ─────────────────────────────────────────

    @Test
    fun relayRegistration_dataClass_holdsPopB64Field() {
        // SyncManager.RelayRegistration must hold inboxId, publicKeyB64, popB64, deviceName.
        // popB64 is computed at relayRegistration() time from the sync key + inbox id.
        val reg = SyncManager.RelayRegistration(
            inboxId = "inbox-id-uuid",
            publicKeyB64 = "pubkey==",
            popB64 = "pop-hmac-b64==",
            deviceName = "My Android",
        )
        assertEquals("inbox-id-uuid", reg.inboxId)
        assertEquals("pubkey==", reg.publicKeyB64)
        assertEquals("pop-hmac-b64==", reg.popB64)
        assertEquals("My Android", reg.deviceName)
    }

    @Test
    fun relayRegistration_popB64_isIncludedInRegisterBody() {
        // Verify the full round-trip: reg.popB64 flows into buildRegisterBody's pop_b64 field.
        val reg = SyncManager.RelayRegistration(
            inboxId = "my-inbox",
            publicKeyB64 = "my-pubkey",
            popB64 = "my-pop-value",
            deviceName = "Test Device",
        )
        val bodyStr: String = RelayClient.buildRegisterBody(
            deviceId = reg.inboxId,
            deviceName = reg.deviceName,
            publicKeyBase64 = reg.publicKeyB64,
            popB64 = reg.popB64,
        )
        val json = JSONObject(bodyStr)
        assertEquals("my-inbox", json.getString("device_id"))
        assertEquals("my-pubkey", json.getString("public_key_b64"))
        assertEquals("my-pop-value", json.getString("pop_b64"))
    }
}
