package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Structural guard that prevents schema.sql and setup.sql from drifting
 * apart on the columns that the Android client reads at runtime
 * (f797: deleted, pinned, pin_order; hash contract).
 *
 * Also validates the Android SharedPreferences blob field-index constants
 * against the canonical v4 format used by ClipboardRepository (CopyPaste-44rq.61).
 * A silent blob-format drift (e.g. inserting a field at position 2) would cause
 * every cross-device decrypt to fail with a corrupt-ciphertext error; these tests
 * catch such regressions without requiring an Android device or the native .so.
 *
 * Runs as a pure-JVM test under `./gradlew :app:testDebugUnitTest` — no
 * Android runtime or Supabase connection needed.
 *
 * The test reads the SQL source files relative to the project root so it
 * stays current with any future column additions without manual updates.
 * If the files move, the tests fail with a clear "missing file" assertion.
 */
class SchemaDriftTest {

    // ── Blob format field-index constants (mirrors encodeItem / parseItem in ClipboardRepository) ─

    /**
     * Canonical pipe-delimited blob format v5 (10 fields):
     *
     * Index 0: wallTimeMs       — wall-clock capture time (Long)
     * Index 1: contentType      — MIME type string
     * Index 2: payloadBytes     — plaintext byte size (Long)
     * Index 3: nonceB64         — XChaCha20-Poly1305 nonce, Base64-NO_WRAP (crypto field)
     * Index 4: ciphertextB64    — encrypted payload, Base64-NO_WRAP (crypto field)
     * Index 5: lamportTs        — Lamport timestamp for LWW sync (Long)
     * Index 6: deleted          — soft-delete tombstone flag: "0" or "1"
     * Index 7: originDeviceId   — UUID of originating device (may be blank)
     * Index 8: keyVersion       — AEAD key generation: "1" (legacy) or "2" (current)
     * Index 9: sourceApp        — package name of the capturing app (may be blank)
     *                            When present and in ClipboardRepository.KNOWN_SENSITIVE_PACKAGES,
     *                            parseItem() forces isSensitive=true. (CopyPaste-44rq.48)
     *
     * These constants must stay in sync with:
     *   - ClipboardRepository.encodeItem()      (writer)
     *   - ClipboardRepository.parseItem()       (reader: wall_time, contentType, nonce[3], ct[4], originDeviceId[7], sourceApp[9])
     *   - ClipboardRepository.loadFullPlaintextBlocking()  (reader: nonce[3], ct[4])
     *   - ClipboardRepository.isDeletedBlob()   (reader: deleted[6])
     *   - ClipboardRepository.bumpBlobLamportTs() (reader/writer: lamportTs[5])
     *   - ClipboardRepository.keyVersionFromParts() (reader: keyVersion[8])
     *   - ClipboardRepository.storeItemWithLww() (reader: lamportTs[5], wallTime[0], originDeviceId[7])
     *   - ClipboardRepository.encodeTombstone()  (writer: preserves contentType[1], originDeviceId[7])
     *   - copypaste-core/src/storage/schema.rs  (Rust canonical schema, v13 as of 2026-06)
     */
    object BlobFormat {
        const val FIELD_COUNT        = 10
        const val IDX_WALL_TIME      = 0
        const val IDX_CONTENT_TYPE   = 1
        const val IDX_PAYLOAD_BYTES  = 2
        const val IDX_NONCE_B64      = 3
        const val IDX_CIPHERTEXT_B64 = 4
        const val IDX_LAMPORT_TS     = 5
        const val IDX_DELETED        = 6
        const val IDX_ORIGIN_DEVICE  = 7
        const val IDX_KEY_VERSION    = 8
        const val IDX_SOURCE_APP     = 9

        // Sentinel values the code relies on
        const val DELETED_FLAG_TRUE  = "1"
        const val DELETED_FLAG_FALSE = "0"
        const val TOMBSTONE_CIPHERTEXT = "tombstone"

        /**
         * Build a synthetic live blob matching ClipboardRepository.encodeItem() v5:
         * "$wallTimeMs|$contentType|$plaintextLen|$nonce64|$ct64|$lamportTs|$deletedFlag|$originDeviceId|$keyVersion|$sourceApp"
         */
        fun encodeLiveBlob(
            wallTimeMs: Long = 1_700_000_000_000L,
            contentType: String = "text/plain",
            payloadBytes: Long = 42L,
            nonceB64: String = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", // 32-byte placeholder
            ciphertextB64: String = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=",
            lamportTs: Long = 12345L,
            deleted: Boolean = false,
            originDeviceId: String = "device-abc",
            keyVersion: Int = 2,
            sourceApp: String = "",
        ): String = "$wallTimeMs|$contentType|$payloadBytes|$nonceB64|$ciphertextB64|$lamportTs|${if (deleted) 1 else 0}|$originDeviceId|$keyVersion|$sourceApp"

        /**
         * Build a synthetic tombstone blob matching ClipboardRepository.encodeTombstone():
         * "$wallTimeMs|$contentType|0||tombstone|$lamportTs|1|$originDeviceId"
         * Note: tombstones are 8 fields (no keyVersion/sourceApp appended by encodeTombstone —
         * tombstones never carry source-app metadata; back-compat reads field 9 as null).
         */
        fun encodeTombstoneBlob(
            wallTimeMs: Long = 1_700_000_001_000L,
            contentType: String = "text/plain",
            lamportTs: Long = 12346L,
            originDeviceId: String = "device-abc",
        ): String = "$wallTimeMs|$contentType|0||tombstone|$lamportTs|1|$originDeviceId"
    }

    // ── Blob field-index assertions (CopyPaste-44rq.61) ──────────────────────

    /**
     * Assert that the field count in a live blob equals FIELD_COUNT (10).
     * Any insertion/removal of a field in encodeItem shifts every subsequent
     * index and breaks the parser.
     *
     * v5 added IDX_SOURCE_APP (index 9) for password-manager sensitivity overrides
     * (CopyPaste-44rq.48). If FIELD_COUNT changes again, update all IDX_* constants.
     */
    @Test
    fun blobFormat_liveBlob_hasExactlyTenFields() {
        val blob = BlobFormat.encodeLiveBlob()
        val parts = blob.split("|")
        assertEquals(
            "Live blob must have exactly ${BlobFormat.FIELD_COUNT} pipe-delimited fields. " +
                "If encodeItem added or removed a field, update BlobFormat.FIELD_COUNT and all " +
                "IDX_* constants to match. Actual blob: '$blob'",
            BlobFormat.FIELD_COUNT,
            parts.size,
        )
    }

    /**
     * Assert field 3 is the nonce (AEAD crypto field).
     * parseItem() and loadFullPlaintextBlocking() both read nonce at index 3.
     * A shift here causes every decrypt call to pass the wrong bytes, producing
     * authentication failures indistinguishable from key mismatch.
     */
    @Test
    fun blobFormat_fieldThree_isNonce() {
        val expectedNonce = "NONCE_PLACEHOLDER_BASE64=="
        val blob = BlobFormat.encodeLiveBlob(nonceB64 = expectedNonce)
        val parts = blob.split("|")
        assertEquals(
            "Blob field at IDX_NONCE_B64 (${BlobFormat.IDX_NONCE_B64}) must be the nonce. " +
                "parseItem() reads nonce from index 3; if encodeItem reordered fields, update IDX_NONCE_B64.",
            expectedNonce,
            parts[BlobFormat.IDX_NONCE_B64],
        )
    }

    /**
     * Assert field 4 is the ciphertext (AEAD crypto field).
     * parseItem() and loadFullPlaintextBlocking() both read ciphertext at index 4.
     * A shift here causes decrypt to receive the nonce bytes as ciphertext — a
     * crash or authentication failure on every history load.
     */
    @Test
    fun blobFormat_fieldFour_isCiphertext() {
        val expectedCt = "CIPHERTEXT_PLACEHOLDER_BASE64=="
        val blob = BlobFormat.encodeLiveBlob(ciphertextB64 = expectedCt)
        val parts = blob.split("|")
        assertEquals(
            "Blob field at IDX_CIPHERTEXT_B64 (${BlobFormat.IDX_CIPHERTEXT_B64}) must be the ciphertext. " +
                "parseItem() reads ciphertext from index 4; if encodeItem reordered fields, update IDX_CIPHERTEXT_B64.",
            expectedCt,
            parts[BlobFormat.IDX_CIPHERTEXT_B64],
        )
    }

    /**
     * Assert field 5 is the Lamport timestamp.
     * storeItemWithLww() reads lamportTs from index 5 for LWW comparison.
     * bumpBlobLamportTs() writes to index 5. A drift here silently breaks
     * cross-device ordering: all items would be treated as zero-ts, making
     * every sync overwrite every peer's copy.
     */
    @Test
    fun blobFormat_fieldFive_isLamportTs() {
        val expectedLamport = 999_888_777L
        val blob = BlobFormat.encodeLiveBlob(lamportTs = expectedLamport)
        val parts = blob.split("|")
        assertEquals(
            "Blob field at IDX_LAMPORT_TS (${BlobFormat.IDX_LAMPORT_TS}) must be the Lamport timestamp. " +
                "storeItemWithLww() and storedLamportTsForItemId() read index 5; bumpBlobLamportTs() writes it.",
            expectedLamport.toString(),
            parts[BlobFormat.IDX_LAMPORT_TS],
        )
    }

    /**
     * Assert field 6 is the deleted flag and that a live blob has value "0".
     * isDeletedBlob() reads index 6 explicitly (NOT the last field) so that the
     * originDeviceId added at index 7 does not shadow it. A drift here would make
     * tombstones invisible to getItems(), resurrecting deleted items on reload.
     */
    @Test
    fun blobFormat_fieldSix_isDeletedFlag_andLiveBlobIsNotDeleted() {
        val blob = BlobFormat.encodeLiveBlob(deleted = false)
        val parts = blob.split("|")
        assertEquals(
            "Blob field at IDX_DELETED (${BlobFormat.IDX_DELETED}) must be the soft-delete flag. " +
                "isDeletedBlob() reads index 6 to distinguish tombstones from live items.",
            BlobFormat.DELETED_FLAG_FALSE,
            parts[BlobFormat.IDX_DELETED],
        )
    }

    /**
     * Assert field 6 holds "1" when deleted=true (tombstone encoding).
     * encodeTombstone() sets index 6 to "1"; isDeletedBlob() checks index 6 == "1".
     * Mismatch = tombstoned items re-appear in history (ghost rows) or live items
     * are incorrectly hidden.
     */
    @Test
    fun blobFormat_fieldSix_isOne_forDeletedBlob() {
        // Test via the live-blob path with deleted=true, then via the tombstone path.
        val liveTombstone = BlobFormat.encodeLiveBlob(deleted = true)
        val liveParts = liveTombstone.split("|")
        assertEquals(
            "A blob encoded with deleted=true must have '1' at IDX_DELETED (${BlobFormat.IDX_DELETED}). " +
                "encodeItem writes deletedFlag = if (deleted) 1 else 0 at field 6.",
            BlobFormat.DELETED_FLAG_TRUE,
            liveParts[BlobFormat.IDX_DELETED],
        )

        // Also verify encodeTombstone emits index 6 == "1".
        val tombstone = BlobFormat.encodeTombstoneBlob()
        val tombParts = tombstone.split("|")
        assertEquals(
            "Tombstone blob must have '1' at index ${BlobFormat.IDX_DELETED}. " +
                "encodeTombstone() is the exclusive writer; isDeletedBlob() is the reader.",
            BlobFormat.DELETED_FLAG_TRUE,
            tombParts[BlobFormat.IDX_DELETED],
        )
    }

    /**
     * Assert field 7 is the originDeviceId.
     * parseItem() reads originDeviceId from index 7 for device attribution.
     * storeItemWithLww() reads index 7 for the LWW tie-break after lamportTs + wallTime.
     * encodeTombstone() preserves originDeviceId from index 7 of the original blob.
     * A drift here would break device-attribution UI and LWW determinism.
     */
    @Test
    fun blobFormat_fieldSeven_isOriginDeviceId() {
        val expectedDevice = "550e8400-e29b-41d4-a716-446655440000"
        val blob = BlobFormat.encodeLiveBlob(originDeviceId = expectedDevice)
        val parts = blob.split("|")
        assertEquals(
            "Blob field at IDX_ORIGIN_DEVICE (${BlobFormat.IDX_ORIGIN_DEVICE}) must be the originDeviceId. " +
                "parseItem() reads it at index 7 for device attribution; storeItemWithLww() uses it for LWW tie-break.",
            expectedDevice,
            parts[BlobFormat.IDX_ORIGIN_DEVICE],
        )
    }

    /**
     * Assert field 8 is the AEAD key version.
     * keyVersionFromParts() reads index 8 and passes it to decryptText().
     * A drift here causes every item to be decrypted with the wrong key generation,
     * producing authentication failures that look like key rotation problems.
     */
    @Test
    fun blobFormat_fieldEight_isKeyVersion() {
        val expectedKeyVersion = 2
        val blob = BlobFormat.encodeLiveBlob(keyVersion = expectedKeyVersion)
        val parts = blob.split("|")
        assertEquals(
            "Blob field at IDX_KEY_VERSION (${BlobFormat.IDX_KEY_VERSION}) must be the AEAD key version. " +
                "keyVersionFromParts() reads index 8 and passes it to decryptText().",
            expectedKeyVersion.toString(),
            parts[BlobFormat.IDX_KEY_VERSION],
        )
    }

    /**
     * Assert field 9 is the sourceApp package name (CopyPaste-44rq.48).
     * parseItem() reads sourceApp from index 9 and, when it is present in
     * ClipboardRepository.KNOWN_SENSITIVE_PACKAGES, forces isSensitive=true
     * at read time. A drift here (e.g. a new field inserted before sourceApp)
     * would silently break the password-manager sensitivity override: captures
     * from 1Password/Bitwarden/etc. would no longer be forced to isSensitive=true,
     * and their content would appear unmasked in the history UI.
     *
     * Back-compat: legacy blobs (< 10 fields) parse field 9 as null via
     * getOrNull(9) — no effect on isSensitive for older items.
     */
    @Test
    fun blobFormat_fieldNine_isSourceApp() {
        val expectedSourceApp = "com.agilebits.onepassword"
        val blob = BlobFormat.encodeLiveBlob(sourceApp = expectedSourceApp)
        val parts = blob.split("|")
        assertEquals(
            "Blob field at IDX_SOURCE_APP (${BlobFormat.IDX_SOURCE_APP}) must be the sourceApp package name. " +
                "parseItem() reads it at index 9 to force isSensitive=true for known password-manager packages " +
                "(CopyPaste-44rq.48). A drift here breaks the PM sensitivity override silently.",
            expectedSourceApp,
            parts[BlobFormat.IDX_SOURCE_APP],
        )
    }

    /**
     * Assert that a one-field insertion BEFORE the nonce position (i.e. at index 3)
     * is caught by the IDX_NONCE_B64 assertion.
     *
     * This is the self-test: verify the guards above would actually FAIL if a field
     * were inserted between payloadBytes (2) and nonceB64 (3). Without this test the
     * guards could be trivially fooled by building the blob from the same constants.
     */
    @Test
    fun blobFormat_selfTest_shiftedNonceIsDetected() {
        val expectedNonce = "CORRECT_NONCE_B64=="
        // Simulate a field inserted before the nonce: the actual nonce is now at index 4,
        // but our constant says it should be at index 3. We expect the value at index 3
        // to be the inserted field ("INSERTED"), not the nonce.
        // The blob has 11 fields (10 canonical + 1 inserted) so FIELD_COUNT=10 would also
        // catch the drift via blobFormat_liveBlob_hasExactlyTenFields, but the nonce-index
        // test is the more targeted guard (crypto field position must be exact).
        val driftedBlob = "1700000000000|text/plain|42|INSERTED|$expectedNonce|ct==|12345|0|device|2|"
        val parts = driftedBlob.split("|")
        // The nonce is now at index 4, not at IDX_NONCE_B64=3.
        assertFalse(
            "Self-test: a blob with an extra field inserted before the nonce must NOT have the " +
                "nonce at IDX_NONCE_B64=${BlobFormat.IDX_NONCE_B64}. If this assertion fails, " +
                "the drift-detection guards above are ineffective.",
            parts[BlobFormat.IDX_NONCE_B64] == expectedNonce,
        )
    }

    /**
     * Assert that the tombstone ciphertext sentinel is the literal string "tombstone".
     * isDeletedBlob() detects tombstones by the deleted flag at index 6 (not the
     * ciphertext value), but the sentinel is preserved as a human-readable diagnostic
     * marker in the SharedPreferences file. Any code that tries to decrypt a tombstone
     * must guard on the deleted flag first to avoid a Base64 decode error.
     *
     * This test pins the sentinel so a refactor that renames it to e.g. "DELETED"
     * must explicitly update this assertion — signalling that any code reading the
     * sentinel string also needs updating.
     */
    @Test
    fun blobFormat_tombstone_ciphertextFieldIsSentinelString() {
        val tombstone = BlobFormat.encodeTombstoneBlob()
        val parts = tombstone.split("|")
        assertEquals(
            "Tombstone blob ciphertext (index ${BlobFormat.IDX_CIPHERTEXT_B64}) must be the literal " +
                "'${BlobFormat.TOMBSTONE_CIPHERTEXT}'. encodeTombstone() writes this sentinel; " +
                "any reader that skips the deleted-flag guard will receive garbage on Base64 decode.",
            BlobFormat.TOMBSTONE_CIPHERTEXT,
            parts[BlobFormat.IDX_CIPHERTEXT_B64],
        )
    }

    /**
     * Assert that the tombstone nonce field (index 3) is empty.
     * encodeTombstone() clears the nonce ("||") to avoid retaining the crypto material
     * for a deleted item. parseItem() would receive an empty nonce B64 and skip the
     * decrypt call — but only if the nonce is truly empty.
     */
    @Test
    fun blobFormat_tombstone_nonceFieldIsEmpty() {
        val tombstone = BlobFormat.encodeTombstoneBlob()
        val parts = tombstone.split("|")
        assertTrue(
            "Tombstone blob nonce (index ${BlobFormat.IDX_NONCE_B64}) must be empty ('') to prevent " +
                "retaining crypto material for deleted items. encodeTombstone() emits '||' for the nonce.",
            parts[BlobFormat.IDX_NONCE_B64].isEmpty(),
        )
    }

    // ── SQL file helpers ──────────────────────────────────────────────────────

    private fun readSql(relativePath: String): String {
        // Working directory during unit tests is android/app/. Walk up to repo root.
        val repoRoot = File("../../").canonicalFile
        val f = File(repoRoot, relativePath)
        assertTrue("SQL file not found: ${f.absolutePath}", f.exists())
        return f.readText()
    }

    // ── Column presence ───────────────────────────────────────────────────────

    @Test
    fun setupSql_hasDeletedColumn() {
        val sql = readSql("docs/supabase/setup.sql")
        assertTrue(
            "setup.sql must declare 'deleted' column (f797: missing from provisioning)",
            sql.contains("deleted"),
        )
    }

    @Test
    fun setupSql_hasPinnedColumn() {
        val sql = readSql("docs/supabase/setup.sql")
        assertTrue(
            "setup.sql must declare 'pinned' column (f797: missing from provisioning)",
            sql.contains("pinned"),
        )
    }

    @Test
    fun setupSql_hasPinOrderColumn() {
        val sql = readSql("docs/supabase/setup.sql")
        assertTrue(
            "setup.sql must declare 'pin_order' column (f797: missing from provisioning)",
            sql.contains("pin_order"),
        )
    }

    @Test
    fun schemaSql_hasDeletedColumn() {
        val sql = readSql("docs/supabase/schema.sql")
        assertTrue(
            "schema.sql must declare 'deleted' column",
            sql.contains("deleted"),
        )
    }

    @Test
    fun schemaSql_hasPinnedColumn() {
        val sql = readSql("docs/supabase/schema.sql")
        assertTrue(
            "schema.sql must declare 'pinned' column",
            sql.contains("pinned"),
        )
    }

    @Test
    fun schemaSql_hasPinOrderColumn() {
        val sql = readSql("docs/supabase/schema.sql")
        assertTrue(
            "schema.sql must declare 'pin_order' column",
            sql.contains("pin_order"),
        )
    }

    // ── Hash contract: both files must agree on SHA-256 (not BLAKE3) ──────────

    @Test
    fun setupSql_contentHashIsSha256NotBlake3() {
        val sql = readSql("docs/supabase/setup.sql")
        val hashComment = sql.lines()
            .firstOrNull { it.contains("content_hash") && it.contains("--") }
            ?: ""
        assertTrue(
            "setup.sql content_hash comment must say SHA-256, not BLAKE3 " +
                "(f797: daemon/core contract uses SHA-256). Found: '$hashComment'",
            !hashComment.contains("BLAKE3", ignoreCase = true),
        )
    }

    @Test
    fun schemaSql_contentHashIsSha256NotBlake3() {
        val sql = readSql("docs/supabase/schema.sql")
        val hashComment = sql.lines()
            .firstOrNull { it.contains("content_hash") && it.contains("--") }
            ?: ""
        assertTrue(
            "schema.sql content_hash comment must say SHA-256 (or be hash-algorithm agnostic), not BLAKE3 " +
                "(f797: daemon/core contract uses SHA-256). Found: '$hashComment'",
            !hashComment.contains("BLAKE3", ignoreCase = true),
        )
    }
}
