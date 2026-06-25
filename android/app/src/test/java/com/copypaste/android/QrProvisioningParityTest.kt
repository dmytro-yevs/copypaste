package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * #11 — QR pairing JSON schema parity: golden JSON test (Android side).
 *
 * Source of truth for the JSON field names:
 *   `QrProvisioning::encode` in
 *   `crates/copypaste-core/src/crypto/pairing_qr/payload.rs`
 *
 * The provisioning 6th QR field carries compact JSON:
 *   {"ru":<relay_url>,"su":<supabase_url>,"sk":<supabase_anon_key>}
 * base64url-encoded (no padding). Android's `PairProvisioning.kt` / `extractQrProvisioning`
 * must parse EXACTLY these field names.
 *
 * The companion Rust test lives at:
 *   crates/copypaste-core/src/crypto/pairing_qr/mod.rs :: qr_provisioning_json_golden_schema
 *
 * BOTH tests use the SAME golden JSON string. If the field names or structure change,
 * update BOTH the Rust test and this Android test.
 */
class QrProvisioningParityTest {

    /**
     * Golden JSON string — must be byte-for-byte identical to the string
     * asserted in the Rust test `qr_provisioning_json_golden_schema`.
     *
     * This string is what QrProvisioning::encode() produces for:
     *   relay_url = "https://relay.example.com"
     *   supabase_url = "https://abcd.supabase.co"
     *   supabase_anon_key = "anon-key-123"
     *
     * Field order: ru, su, sk (insertion order from QrProvisioning::encode).
     */
    private val GOLDEN_JSON =
        """{"ru":"https://relay.example.com","su":"https://abcd.supabase.co","sk":"anon-key-123"}"""

    /**
     * Assert that PairProvisioning.kt's `extractJsonString` correctly parses
     * the canonical field names "ru", "su", "sk" from the golden JSON string.
     *
     * Source of truth: QrProvisioning::encode in payload.rs uses keys
     *   "ru" → relay_url
     *   "su" → supabase_url
     *   "sk" → supabase_anon_key
     *
     * If these field names change in Rust, update this test AND the Rust golden test.
     */
    @Test
    fun `golden JSON ru field parses as relayUrl`() {
        val result = extractQrProvisioning(buildCppair2PayloadWithProvJson(GOLDEN_JSON))
        assertEquals(
            "Field 'ru' in provisioning JSON must parse as relayUrl — " +
                "if the Rust QrProvisioning field name changed, " +
                "update qr_provisioning_json_golden_schema in mod.rs too",
            "https://relay.example.com",
            result?.relayUrl,
        )
    }

    @Test
    fun `golden JSON su field parses as supabaseUrl`() {
        val result = extractQrProvisioning(buildCppair2PayloadWithProvJson(GOLDEN_JSON))
        assertEquals(
            "Field 'su' in provisioning JSON must parse as supabaseUrl — " +
                "if the Rust QrProvisioning field name changed, " +
                "update qr_provisioning_json_golden_schema in mod.rs too",
            "https://abcd.supabase.co",
            result?.supabaseUrl,
        )
    }

    @Test
    fun `golden JSON sk field parses as supabaseAnonKey`() {
        val result = extractQrProvisioning(buildCppair2PayloadWithProvJson(GOLDEN_JSON))
        assertEquals(
            "Field 'sk' in provisioning JSON must parse as supabaseAnonKey — " +
                "if the Rust QrProvisioning field name changed, " +
                "update qr_provisioning_json_golden_schema in mod.rs too",
            "anon-key-123",
            result?.supabaseAnonKey,
        )
    }

    @Test
    fun `provisioning absent when prov field is missing`() {
        // A 5-field CPPAIR2 payload with no 6th field — provisioning must be null.
        val payload = "CPPAIR2.fp.tok.id.name.addr"
        val result = extractQrProvisioning(payload)
        assertNull("No provisioning field → result must be null", result)
    }

    @Test
    fun `corrupt provisioning field does not crash`() {
        // A CPPAIR2 payload with a corrupt 6th field — must return null, never throw.
        val payload = "CPPAIR2.fp.tok.id.name.addr.!!!notbase64!!!"
        val result = extractQrProvisioning(payload)
        assertNull("Corrupt provisioning field → result must be null (never throw)", result)
    }

    // ─── helpers ───────────────────────────────────────────────────────────────

    /**
     * Build a CPPAIR2 bare payload with the given provisioning JSON base64url-encoded
     * in the 6th field. Used to invoke `extractQrProvisioning` which requires a
     * full CPPAIR2-prefixed string.
     *
     * The QrProvisioning::encode() in Rust base64url-encodes the JSON without padding.
     * We replicate that here using Android's Base64.NO_WRAP | Base64.NO_PADDING
     * with the url-safe alphabet (+ → - and / → _).
     */
    private fun buildCppair2PayloadWithProvJson(json: String): String {
        val jsonBytes = json.toByteArray(Charsets.UTF_8)
        // base64url (url-safe alphabet, no padding) — same as Rust's b64().encode().
        val provB64 = android.util.Base64
            .encodeToString(jsonBytes, android.util.Base64.URL_SAFE or android.util.Base64.NO_WRAP or android.util.Base64.NO_PADDING)
        return "CPPAIR2.fp.tok.id.name.addr.$provB64"
    }
}
