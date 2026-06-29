package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-crh3.38: cloud account-mismatch banner — Android parity with macOS
 * [CloudAccountMismatchBanner] / cloudAccountMismatch detection.
 *
 * The banner warns when the configured cloud account differs from a paired peer's
 * account: Supabase RLS only shares rows owned by the same GoTrue user, so a
 * mismatch silently breaks cloud sync.
 *
 * [detectCloudAccountMismatch] mirrors the macOS predicate exactly:
 *  - false when localAccountId is null (cloud-sync off / not signed in)
 *  - false when no peer carries an account id (legacy / not-yet-plumbed)
 *  - false when all peers with ids match the local id
 *  - true  when ANY peer's account id differs from the local id
 */
class CloudAccountMismatchBannerTest {

    @Test
    fun `hidden when local account id is null`() {
        assertFalse(detectCloudAccountMismatch(null, listOf("proj/uid_1")))
    }

    @Test
    fun `hidden when no peers carry an account id`() {
        assertFalse(detectCloudAccountMismatch("proj/uid_local", listOf(null, null)))
    }

    @Test
    fun `hidden when peer list is empty (ids not yet plumbed)`() {
        assertFalse(detectCloudAccountMismatch("proj/uid_local", emptyList()))
    }

    @Test
    fun `hidden when all peers with ids match local id`() {
        val id = "proj_shared/uid_same"
        assertFalse(detectCloudAccountMismatch(id, listOf(id, id, null)))
    }

    @Test
    fun `shown when one peer differs from local id`() {
        assertTrue(detectCloudAccountMismatch("proj/uid_local", listOf("proj_other/uid_99")))
    }

    @Test
    fun `shown when any peer differs even if others match`() {
        val local = "proj/uid_local"
        assertTrue(detectCloudAccountMismatch(local, listOf(local, "proj_other/uid_99")))
    }

    @Test
    fun `hidden when local matches the only peer that has an id`() {
        val local = "proj/uid_local"
        assertFalse(detectCloudAccountMismatch(local, listOf(null, local)))
    }
}
