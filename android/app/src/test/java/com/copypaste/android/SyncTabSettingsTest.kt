package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLog

/**
 * CopyPaste-myh8.9 wave 4 (§I "Sync tab"): 3-layer preservation coverage for
 * `sync_on_wifi_only`, `p2p_sync_enabled`, `lan_visibility`,
 * `auto_apply_synced_clip` (persistence only — consumer covered by
 * [RepairedSettingsConsumersTest.autoApplySyncedClip]), `relay_enabled`/
 * `supabase_enabled` (Immediate), `supabase_url`/`supabase_anon_key`/
 * `supabase_email` round-trip, `relay_url`.
 *
 * Secrets (`cloudSyncPassphrase`/`supabaseEmail`/`supabasePassword`): [Settings]
 * always wires a REAL `AndroidKeystoreKekCipher` (not injectable through the
 * facade), so the fake-seam pattern is exercised against [KeystoreSecretStore]
 * directly — the same class, same wrap/unwrap contract, same pattern as
 * [KeystoreSecretStoreTest] — rather than via [Settings]. `supabaseUrl`/
 * `supabaseAnonKey` (non-secret) ARE exercised through [Settings] directly.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SyncTabSettingsTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    private fun secretStore(prefs: FakeSharedPreferences = FakeSharedPreferences()) =
        KeystoreSecretStore(prefs, FakeKekCipher(), FakeBase64Codec)

    // ── sync_on_wifi_only ────────────────────────────────────────────────────

    @Test
    fun `sync_on_wifi_only defaults to false and round-trips`() {
        val ctx = context()
        assertFalse(Settings(ctx).syncOnWifiOnly)
        Settings(ctx).syncOnWifiOnly = true
        assertTrue(Settings(ctx).syncOnWifiOnly)
    }

    @Test
    fun `sync_on_wifi_only upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("sync_on_wifi_only", true).apply()
        assertTrue(Settings(ctx).syncOnWifiOnly)
    }

    // ── p2p_sync_enabled ─────────────────────────────────────────────────────

    @Test
    fun `p2p_sync_enabled defaults to true and round-trips`() {
        val ctx = context()
        assertTrue(Settings(ctx).p2pSyncEnabled)
        Settings(ctx).p2pSyncEnabled = false
        assertFalse(Settings(ctx).p2pSyncEnabled)
    }

    @Test
    fun `p2p_sync_enabled upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean(Settings.KEY_P2P_SYNC_ENABLED, false).apply()
        assertFalse(Settings(ctx).p2pSyncEnabled)
    }

    // ── lan_visibility ───────────────────────────────────────────────────────

    @Test
    fun `lan_visibility defaults to true and round-trips`() {
        val ctx = context()
        assertTrue(Settings(ctx).lanVisibility)
        Settings(ctx).lanVisibility = false
        assertFalse(Settings(ctx).lanVisibility)
    }

    @Test
    fun `lan_visibility upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("lan_visibility", false).apply()
        assertFalse(Settings(ctx).lanVisibility)
    }

    // ── auto_apply_synced_clip (persistence only; consumer in RepairedSettingsConsumersTest) ──

    @Test
    fun `auto_apply_synced_clip defaults to true and round-trips`() {
        val ctx = context()
        assertTrue(Settings(ctx).autoApplySyncedClip)
        Settings(ctx).autoApplySyncedClip = false
        assertFalse(Settings(ctx).autoApplySyncedClip)
    }

    @Test
    fun `auto_apply_synced_clip upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("auto_apply_synced_clip", false).apply()
        assertFalse(Settings(ctx).autoApplySyncedClip)
    }

    // ── relay_enabled / supabase_enabled (Immediate) ─────────────────────────

    @Test
    fun `relay_enabled and supabase_enabled default to true and are independently toggleable`() {
        val ctx = context()
        val settings = Settings(ctx)
        assertTrue(settings.relayEnabled)
        assertTrue(settings.supabaseEnabled)

        settings.relayEnabled = false
        assertFalse(settings.relayEnabled)
        assertTrue("disabling relay must not affect the independent supabase flag", settings.supabaseEnabled)

        settings.supabaseEnabled = false
        assertFalse(settings.supabaseEnabled)
    }

    @Test
    fun `relay_enabled is readable immediately after a bare setter write — no Save batch required`() {
        val ctx = context()
        Settings(ctx).relayEnabled = false
        assertFalse("Immediate mode: a fresh instance must see the write without a Save", Settings(ctx).relayEnabled)
    }

    // ── supabase_url / supabase_anon_key round-trip ──────────────────────────

    @Test
    fun `supabase_url round-trips and trims a trailing slash`() {
        val ctx = context()
        val settings = Settings(ctx)
        settings.supabaseUrl = "https://abc.supabase.co/"
        assertEquals("https://abc.supabase.co", Settings(ctx).supabaseUrl)
    }

    @Test
    fun `supabase_anon_key round-trips through a fresh Settings instance`() {
        val ctx = context()
        Settings(ctx).supabaseAnonKey = "anon-key-value"
        assertEquals("anon-key-value", Settings(ctx).supabaseAnonKey)
    }

    @Test
    fun `supabase_url and supabase_anon_key upgrade fixture — pre-redesign raw keys survive`() {
        val ctx = context()
        rawPrefs().edit()
            .putString("supabase_url", "https://legacy.supabase.co")
            .putString("supabase_anon_key", "legacy-anon-key")
            .apply()

        val settings = Settings(ctx)
        assertEquals("https://legacy.supabase.co", settings.supabaseUrl)
        assertEquals("legacy-anon-key", settings.supabaseAnonKey)
    }

    // ── relay_url ────────────────────────────────────────────────────────────

    @Test
    fun `relay_url defaults to blank and round-trips`() {
        val ctx = context()
        assertEquals("", Settings(ctx).relayUrl)
        Settings(ctx).relayUrl = "https://relay.example.com"
        assertEquals("https://relay.example.com", Settings(ctx).relayUrl)
    }

    @Test
    fun `relay_url upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putString("relay_url", "https://legacy-relay.example.com").apply()
        assertEquals("https://legacy-relay.example.com", Settings(ctx).relayUrl)
    }

    // ── secrets: KEK-wrapped supabaseEmail / supabasePassword / cloudSyncPassphrase ──
    // Exercised directly against KeystoreSecretStore with fake seams (see class kdoc).

    @Test
    fun `supabaseEmail round-trips through the KEK wrap-unwrap fake seam`() {
        val s = secretStore()
        s.supabaseEmail = "user@example.com"
        assertEquals("user@example.com", s.supabaseEmail)
    }

    @Test
    fun `supabasePassword round-trips through the KEK wrap-unwrap fake seam`() {
        val s = secretStore()
        s.supabasePassword = "hunter2-super-secret"
        assertEquals("hunter2-super-secret", s.supabasePassword)
    }

    @Test
    fun `cloudSyncPassphrase round-trips through the KEK wrap-unwrap fake seam`() {
        val s = secretStore()
        s.cloudSyncPassphrase = "correct horse battery staple"
        assertEquals("correct horse battery staple", s.cloudSyncPassphrase)
    }

    @Test
    fun `no raw secret value written in this test ever appears in a captured Log line`() {
        ShadowLog.stream = null // ensure we only inspect the shadow's captured buffer, not stdout
        val secretEmail = "leak-check-user@example.com"
        val secretPassword = "leak-check-hunter2-secret"
        val secretPassphrase = "leak-check-correct-horse-battery-staple"

        val s = secretStore()
        s.supabaseEmail = secretEmail
        s.supabasePassword = secretPassword
        s.cloudSyncPassphrase = secretPassphrase
        // Also exercise the non-secret Settings-facade path so its logging (if any) is covered too.
        val ctx = context()
        Settings(ctx).supabaseUrl = "https://leak-check.supabase.co"

        val leaked = ShadowLog.getLogs().any { item ->
            val msg = item.msg ?: ""
            msg.contains(secretEmail) || msg.contains(secretPassword) || msg.contains(secretPassphrase)
        }
        assertFalse("no captured log line may contain a raw secret value", leaked)
    }
}
