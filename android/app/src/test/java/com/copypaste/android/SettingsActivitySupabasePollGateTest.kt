package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-myh8.9 wave 3 task 16: [shouldScheduleSupabasePoll] gates the
 * Supabase poll worker on `supabaseEnabled && isSupabaseConfigured` — the same
 * predicate [SupabasePollWorker.doWork] uses for its own self-gate — rather
 * than the legacy `syncBackend == SyncBackend.SUPABASE` enum hint (CopyPaste-26zi).
 *
 * Exercises the pure boolean overload directly: the [Settings]-reading overload
 * delegates to it, and [Settings.isSupabaseConfigured] transitively reads
 * keystore-backed fields (cloudSyncPassphrase/cloudSyncKeyDirect) that require
 * real AndroidKeyStore — unavailable under this module's Robolectric JVM tests
 * (see KeystoreSecretStoreTest's Fake seams for the same constraint).
 *
 * NOTE: this is a plain JUnit4 test in place of a Compose UI test — this module
 * has no `createComposeRule` usage anywhere under app/src/test (confirmed before
 * writing this file), so the SettingsTextField isError->OutlinedTextField
 * semantics assertion (task 17's other half) is left for the Wave 5
 * golden/screenshot tests rather than adding new Compose test infra here.
 */
class SettingsActivitySupabasePollGateTest {

    @Test
    fun `supabaseEnabled true and configured — poll is scheduled`() {
        assertTrue(shouldScheduleSupabasePoll(supabaseEnabled = true, isSupabaseConfigured = true))
    }

    @Test
    fun `supabaseEnabled false — poll is NOT scheduled even when configured`() {
        assertFalse(shouldScheduleSupabasePoll(supabaseEnabled = false, isSupabaseConfigured = true))
    }

    @Test
    fun `supabaseEnabled true but not configured — poll is NOT scheduled`() {
        assertFalse(shouldScheduleSupabasePoll(supabaseEnabled = true, isSupabaseConfigured = false))
    }

    @Test
    fun `supabaseEnabled false and not configured — poll is NOT scheduled`() {
        assertFalse(shouldScheduleSupabasePoll(supabaseEnabled = false, isSupabaseConfigured = false))
    }
}
