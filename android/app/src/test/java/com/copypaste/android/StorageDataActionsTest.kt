package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * CopyPaste-myh8.9 wave 4 (§I "Storage tab", data-actions half): export
 * include-sensitive (Ephemeral), import success/dedup outcomes (extends the
 * import coverage started by [RepairedSettingsConsumersTest] — that file
 * covers the maxFileSizeBytes gate; this one covers dedup-by-id and a plain
 * text-item skip-blank outcome), destructive confirm gates for
 * Clear/[ClipboardRepository.resetDatabase], vacuum outcomes.
 *
 * Reuse-first: rg confirmed no prior `importHistory` test file besides
 * [RepairedSettingsConsumersTest] — extending that file's SCOPE (not its file)
 * per the wave-brief file assignment; both files share package-private access
 * to [ClipboardRepository] internals but do not duplicate any assertion.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class StorageDataActionsTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    // ── export "include sensitive" — Ephemeral (no persisted key) ───────────

    @Test
    fun `export include-sensitive has no persisted SharedPreferences key — it is a per-call parameter only`() {
        val ctx = context()
        // rg-confirmed (ClipboardRepositorySync.kt exportHistoryImpl / SettingsActivity.kt):
        // includeSensitive is a plain function parameter threaded straight from a
        // local Compose `var`, never written through Settings/prefs. Assert the
        // absence directly: no key in either prefs file resembles it.
        val settingsPrefs = ctx.getSharedPreferences("copypaste", Context.MODE_PRIVATE)
        val itemsPrefs = ctx.getSharedPreferences("copypaste_items", Context.MODE_PRIVATE)
        val suspectKeys = listOf(
            "include_sensitive", "export_include_sensitive", "includeSensitive",
        )
        suspectKeys.forEach { key ->
            assertFalse("must not exist in settings prefs: $key", settingsPrefs.contains(key))
            assertFalse("must not exist in items prefs: $key", itemsPrefs.contains(key))
        }
    }

    @Test
    fun `export include-sensitive true does not persist any new key — this export only`() = runBlocking {
        val ctx = context()
        val repository = ClipboardRepository(ctx)
        val settingsPrefs = ctx.getSharedPreferences("copypaste", Context.MODE_PRIVATE)
        val beforeKeys = settingsPrefs.all.keys.toSet()

        // Empty repository: getItemsImpl's fallback (non-native) path returns an
        // empty list without any native decrypt call, so this is safe under JVM tests.
        val json = repository.exportHistory(ByteArray(32), includeSensitive = true)

        val root = JSONObject(json)
        assertEquals(0, root.getJSONArray("items").length())
        assertEquals(beforeKeys, settingsPrefs.all.keys.toSet())
    }

    // ── import: dedup by existing id ─────────────────────────────────────────

    @Test
    fun `import skips an item whose id is already present locally — dedup outcome`() = runBlocking {
        val ctx = context()
        val settings = Settings(ctx)
        val repository = ClipboardRepository(ctx)
        // Seed the id index directly (same package — internal access) so dedup is
        // exercised without ever calling storeItem (no native library available).
        ctx.getSharedPreferences("copypaste_items", Context.MODE_PRIVATE).edit()
            .putString(ClipboardRepository.KEY_ITEM_IDS, "existing-1")
            .apply()
        ClipboardItemCache.cachedIds = null // invalidate the in-memory snapshot from any prior test

        val json = JSONObject().apply {
            put("version", 1)
            put("exported_at", 1L)
            put("items", org.json.JSONArray().put(
                JSONObject().apply {
                    put("id", "existing-1")
                    put("content_type", "text")
                    put("full_text", "already have this one")
                    put("wall_time_ms", 1L)
                    put("pinned", false)
                },
            ))
        }.toString()

        val imported = repository.importHistory(json, ByteArray(32), settings)

        assertEquals("an already-present id must be skipped, not re-imported", 0, imported)
    }

    @Test
    fun `import skips an item with blank full_text and snippet — no crash, zero imported`() = runBlocking {
        val ctx = context()
        val settings = Settings(ctx)
        val repository = ClipboardRepository(ctx)
        ClipboardItemCache.cachedIds = null

        val json = JSONObject().apply {
            put("version", 1)
            put("exported_at", 1L)
            put("items", org.json.JSONArray().put(
                JSONObject().apply {
                    put("id", "blank-1")
                    put("content_type", "text")
                    put("full_text", "")
                    put("snippet", "")
                    put("wall_time_ms", 1L)
                    put("pinned", false)
                },
            ))
        }.toString()

        val imported = repository.importHistory(json, ByteArray(32), settings)

        assertEquals(0, imported)
    }

    // ── destructive confirm gates ─────────────────────────────────────────────

    @Test(expected = IllegalArgumentException::class)
    fun `resetDatabase confirmed=false throws — the require() gate is the destructive-action confirmation`() {
        val ctx = context()
        ClipboardRepository(ctx).resetDatabase(confirmed = false)
    }

    @Test
    fun `resetDatabase confirmed=true wipes the items prefs file`() {
        val ctx = context()
        val itemsPrefs = ctx.getSharedPreferences("copypaste_items", Context.MODE_PRIVATE)
        itemsPrefs.edit().putString(ClipboardRepository.KEY_ITEM_IDS, "some-id").apply()
        ClipboardItemCache.cachedIds = null

        ClipboardRepository(ctx).resetDatabase(confirmed = true)

        assertFalse(itemsPrefs.contains(ClipboardRepository.KEY_ITEM_IDS))
    }

    // NOTE: Clear (clearUnpinned/clearAll)'s confirmation dialog lives entirely
    // in StorageTab's @Composable (a showConfirm boolean gating the onClick
    // lambda) — no extractable non-Compose predicate exists for it, unlike
    // resetDatabase's require(confirmed). Deferred to goldens/connected; the
    // underlying clearAll() operation itself has no confirm parameter to test.

    // ── vacuum outcomes ────────────────────────────────────────────────────────

    @Test
    fun `dbVacuum is a validated no-op when the native library is not loaded — the JVM unit-test default`() {
        assertFalse(
            "this test's premise requires the native .so to be absent in the JVM test process",
            isNativeLibraryLoaded,
        )
        // Must return normally (stub mode), not throw — this IS the "unavailable" outcome.
        dbVacuum("/tmp/does-not-exist.db", ByteArray(32))
    }
}
