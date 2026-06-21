package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for the release-signing guard logic (CopyPaste-56gh).
 *
 * The bug: build.gradle.kts silently fell back to the debug signing config when
 * the release keystore was absent. On a CI release-tag build (GITHUB_REF contains
 * "refs/tags/") this produced a debug-signed APK which is update-incompatible with
 * the production release-signed APKs.
 *
 * The fix adds a predicate [isReleaseCiBuild] that returns true iff the current
 * environment is a CI tag build. The build script uses this to throw a
 * GradleException instead of falling back to debug signing.
 *
 * These tests validate the predicate logic without Gradle internals.
 */
class ReleaseSigningGuardTest {

    // ── isReleaseCiBuild ──────────────────────────────────────────────────────

    /**
     * Mirror of the build script predicate — pure function, no Gradle dependency.
     *
     * Returns true if GITHUB_REF is set and starts with "refs/tags/", indicating
     * a GitHub Actions release-tag run where the keystore MUST be present.
     */
    private fun isReleaseCiBuild(githubRef: String?): Boolean =
        githubRef != null && githubRef.startsWith("refs/tags/")

    @Test
    fun tagRef_isReleaseCiBuild_returnsTrue() {
        assertTrue(isReleaseCiBuild("refs/tags/v0.7.5"))
    }

    @Test
    fun tagRef_withoutV_isReleaseCiBuild_returnsTrue() {
        assertTrue(isReleaseCiBuild("refs/tags/0.7.5"))
    }

    @Test
    fun branchRef_isReleaseCiBuild_returnsFalse() {
        assertFalse(isReleaseCiBuild("refs/heads/main"))
    }

    @Test
    fun prRef_isReleaseCiBuild_returnsFalse() {
        assertFalse(isReleaseCiBuild("refs/pull/123/merge"))
    }

    @Test
    fun nullRef_isReleaseCiBuild_returnsFalse() {
        // Local build: GITHUB_REF not set → no CI → should NOT fail.
        assertFalse(isReleaseCiBuild(null))
    }

    @Test
    fun emptyRef_isReleaseCiBuild_returnsFalse() {
        assertFalse(isReleaseCiBuild(""))
    }

    // ── Signing decision matrix ───────────────────────────────────────────────

    /**
     * Signing decision: mirrors what the build script does AFTER the fix.
     *
     * - releaseConfigPresent=true  → always use release config.
     * - releaseConfigPresent=false AND isReleaseCiBuild → throw (fail loudly).
     * - releaseConfigPresent=false AND NOT isReleaseCiBuild → debug fallback (local).
     *
     * Returns: "release" | "debug" | "throw"
     */
    private fun signingDecision(releaseConfigPresent: Boolean, githubRef: String?): String =
        when {
            releaseConfigPresent -> "release"
            isReleaseCiBuild(githubRef) -> "throw"
            else -> "debug"
        }

    @Test
    fun releaseConfigPresent_always_usesRelease() {
        // Even on a tag build, if the config IS present → use it.
        assertEquals("release", signingDecision(true, "refs/tags/v1.0.0"))
        assertEquals("release", signingDecision(true, null))
    }

    @Test
    fun missingConfig_tagBuild_throws() {
        assertEquals("throw", signingDecision(false, "refs/tags/v0.7.5"))
    }

    @Test
    fun missingConfig_localBuild_usesDebug() {
        // Local developer or fork — no GITHUB_REF → debug fallback is OK.
        assertEquals("debug", signingDecision(false, null))
    }

    @Test
    fun missingConfig_prBuild_usesDebug() {
        // PR builds are not tag builds → debug fallback is acceptable.
        assertEquals("debug", signingDecision(false, "refs/pull/42/merge"))
    }

    @Test
    fun missingConfig_branchBuild_usesDebug() {
        assertEquals("debug", signingDecision(false, "refs/heads/main"))
    }

    // Duplicate assertEquals import fix: use the JUnit one via Assert.assertEquals alias
    private fun assertEquals(expected: String, actual: String) {
        org.junit.Assert.assertEquals(expected, actual)
    }
}
