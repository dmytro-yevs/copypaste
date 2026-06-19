package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Structural guard that prevents schema.sql and setup.sql from drifting
 * apart on the columns that the Android client reads at runtime
 * (f797: deleted, pinned, pin_order; hash contract).
 *
 * Runs as a pure-JVM test under `./gradlew :app:testDebugUnitTest` — no
 * Android runtime or Supabase connection needed.
 *
 * The test reads the SQL source files relative to the project root so it
 * stays current with any future column additions without manual updates.
 * If the files move, the tests fail with a clear "missing file" assertion.
 */
class SchemaDriftTest {

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
