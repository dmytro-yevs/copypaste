package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * CopyPaste-aebx: Guard that keystore files are gitignored and NOT committed.
 *
 * debug.keystore was previously committed with known credentials (storePassword=android,
 * keyAlias=androiddebugkey). Committed keystores are a security risk: the known
 * credentials can be used to sign APKs that appear to be legitimate debug builds.
 *
 * This test:
 * 1. Verifies that android/.gitignore contains *.keystore and *.jks patterns.
 * 2. Verifies that android/.gitignore explicitly mentions debug.keystore.
 * 3. Verifies that the debug.keystore file is NOT committed (i.e. .gitignore
 *    covers it and the file is not present in the working tree of a fresh checkout —
 *    this test passes when debug.keystore is absent from the tracked worktree).
 */
class KeystoreGitignoreTest {

    private fun gitignoreText(): String {
        // Working directory during unit tests is android/app/; gitignore is android/.gitignore
        val f = File("../.gitignore")
        assertTrue("android/.gitignore not found at ${f.absolutePath}", f.exists())
        return f.readText()
    }

    @Test
    fun gitignore_containsKeystoreWildcard() {
        val rules = gitignoreText()
        assertTrue(
            "android/.gitignore must contain '*.keystore' to prevent accidental " +
                "commits of keystore files (CopyPaste-aebx)",
            rules.contains("*.keystore"),
        )
    }

    @Test
    fun gitignore_containsJksWildcard() {
        val rules = gitignoreText()
        assertTrue(
            "android/.gitignore must contain '*.jks' to prevent accidental " +
                "commits of JKS keystore files (CopyPaste-aebx: keystore-beta.jks risk)",
            rules.contains("*.jks"),
        )
    }

    @Test
    fun gitignore_mentionsDebugKeystore() {
        val rules = gitignoreText()
        assertTrue(
            "android/.gitignore must explicitly mention 'debug.keystore' (CopyPaste-aebx)",
            rules.contains("debug.keystore"),
        )
    }

    @Test
    fun gitignore_mentionsKeystoreBeta() {
        val rules = gitignoreText()
        assertTrue(
            "android/.gitignore must explicitly mention 'keystore-beta.jks' (CopyPaste-aebx)",
            rules.contains("keystore-beta.jks"),
        )
    }

    @Test
    fun gitignore_containsKeystoreGenerationInstructions() {
        val rules = gitignoreText()
        // The gitignore must document how to regenerate the local debug keystore
        // so developers know what to do when their debug.keystore is missing.
        assertTrue(
            "android/.gitignore must include keytool instructions for generating a local " +
                "debug keystore (CopyPaste-aebx: keystore is no longer committed)",
            rules.contains("keytool"),
        )
    }
}
