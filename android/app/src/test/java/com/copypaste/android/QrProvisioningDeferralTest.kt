package com.copypaste.android

import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.assertFalse
import org.junit.Test
import java.io.File

/**
 * CopyPaste-tqt0: Verifies that QR provisioning (6th field of cppair:// payload)
 * is deferred to post-confirmation (inside finalizeSync) and NOT applied at
 * scan / deep-link parse time.
 *
 * Two layers:
 *  1. Pure-function null-path tests for extractQrProvisioning (no android.util.Base64
 *     needed — these paths return null before reaching the base64 decode).
 *  2. Source-code guard tests: PairActivity.kt must only call applyQrProvisioning
 *     inside finalizeSync (after PAKE bootstrap, passed in as BootstrapResult),
 *     never in the scan callback or deep-link LaunchedEffect.
 *
 * CopyPaste-1jms.33: runPairAndSync was split into runBootstrap (phase 1: PAKE) and
 * finalizeSync (phase 2: provisioning + sync + roster commit). The security invariant
 * is unchanged: applyQrProvisioning still runs only after the PAKE exchange completes.
 *
 * Note: The base64-decode code path inside extractQrProvisioning uses
 * android.util.Base64 which is not available in JVM unit tests (pure Kotlin/JVM,
 * no robolectric). Parse-success paths are therefore covered by on-device
 * instrumented tests rather than here.
 */
class QrProvisioningDeferralTest {

    // ── 1. Pure-function null-path tests ──────────────────────────────────────
    //
    // These cases all return null BEFORE reaching android.util.Base64.decode,
    // so they run on the JVM without Android APIs.

    @Test
    fun `extractQrProvisioning returns null for CPPAIR1 payload`() {
        // CPPAIR1 addr_hint may contain IPv4 dots that collide with the '.' delimiter;
        // provisioning is intentionally not supported for v1 payloads.
        val payload = "CPPAIR1.fp.tok.id.name.192.168.1.1:8080"
        assertNull(extractQrProvisioning(payload))
    }

    @Test
    fun `extractQrProvisioning returns null when fewer than 7 dot-fields`() {
        // CPPAIR2 with exactly 6 dot-separated parts (0..5) — no provisioning field.
        val payload = "CPPAIR2.fp.tok.id.name.addr"  // split(".") yields 6 parts
        assertNull(extractQrProvisioning(payload))
    }

    @Test
    fun `extractQrProvisioning returns null for empty seventh field`() {
        // Trailing dot → 7th field is empty string after split.
        val payload = "CPPAIR2.fp.tok.id.name.addr."
        assertNull(extractQrProvisioning(payload))
    }

    @Test
    fun `extractQrProvisioning returns null for whitespace-only seventh field`() {
        val payload = "CPPAIR2.fp.tok.id.name.addr.   "
        assertNull(extractQrProvisioning(payload))
    }

    @Test
    fun `extractQrProvisioning returns null for non-CPPAIR prefix`() {
        assertNull(extractQrProvisioning("randomstring"))
        assertNull(extractQrProvisioning(""))
        assertNull(extractQrProvisioning("CPPAIR3.fp.tok.id.name.addr.b64"))
    }

    // ── 2. Source-code guard tests ─────────────────────────────────────────────
    //
    // Security invariant: applyQrProvisioning must ONLY appear inside
    // runPairAndSync (which runs after the PAKE handshake). It must NOT appear
    // in the scan-result callback or the deep-link LaunchedEffect.

    private val pairActivitySrc: String by lazy {
        val candidates = listOf(
            "android/app/src/main/java/com/copypaste/android/PairActivity.kt",
            "../android/app/src/main/java/com/copypaste/android/PairActivity.kt",
            "../../android/app/src/main/java/com/copypaste/android/PairActivity.kt",
        )
        candidates
            .map { File(it) }
            .firstOrNull { it.exists() }
            ?.readText()
            ?: error("Could not locate PairActivity.kt from test working directory")
    }

    @Test
    fun `scan result block does not call applyQrProvisioning`() {
        val src = pairActivitySrc

        // The scanner result lambda is between ScanContract() and fun launchScanner().
        val scanStart = src.indexOf("ScanContract()")
        assertTrue("PairActivity must contain a ScanContract() scanner", scanStart >= 0)

        val launchScannerStart = src.indexOf("fun launchScanner()", scanStart)
        assertTrue("PairActivity must have fun launchScanner() after ScanContract()", launchScannerStart >= 0)

        val scanBlock = src.substring(scanStart, launchScannerStart)

        assertFalse(
            "CopyPaste-tqt0: applyQrProvisioning MUST NOT be called inside the scan " +
                "result block. Provisioning must be deferred to finalizeSync (post-confirmation).",
            scanBlock.contains("applyQrProvisioning"),
        )
    }

    @Test
    fun `scan result block sets pendingProvisioningRaw`() {
        val src = pairActivitySrc

        val scanStart = src.indexOf("ScanContract()")
        val launchScannerStart = src.indexOf("fun launchScanner()", scanStart)
        val scanBlock = src.substring(scanStart, launchScannerStart)

        assertTrue(
            "CopyPaste-tqt0: the scan result block must set pendingProvisioningRaw to retain " +
                "the raw payload for deferred provisioning.",
            scanBlock.contains("pendingProvisioningRaw"),
        )
    }

    @Test
    fun `deep-link LaunchedEffect does not call applyQrProvisioning`() {
        val src = pairActivitySrc

        val deepLinkStart = src.indexOf("LaunchedEffect(incomingDeepLinkPayload)")
        assertTrue(
            "PairActivity must have a LaunchedEffect(incomingDeepLinkPayload)",
            deepLinkStart >= 0,
        )

        // This block ends before LaunchedEffect(incomingDeepLinkError).
        val nextEffect = src.indexOf("LaunchedEffect(incomingDeepLinkError)", deepLinkStart)
        assertTrue(
            "PairActivity must have LaunchedEffect(incomingDeepLinkError) after the deep-link block",
            nextEffect >= 0,
        )

        val deepLinkBlock = src.substring(deepLinkStart, nextEffect)

        assertFalse(
            "CopyPaste-tqt0: applyQrProvisioning MUST NOT be called in the deep-link " +
                "LaunchedEffect. A hostile cppair:// must not seed settings without consent. " +
                "Provisioning is deferred to finalizeSync (post PAKE + user review).",
            deepLinkBlock.contains("applyQrProvisioning"),
        )
    }

    @Test
    fun `deep-link LaunchedEffect sets pendingProvisioningRaw`() {
        val src = pairActivitySrc

        val deepLinkStart = src.indexOf("LaunchedEffect(incomingDeepLinkPayload)")
        val nextEffect = src.indexOf("LaunchedEffect(incomingDeepLinkError)", deepLinkStart)
        val deepLinkBlock = src.substring(deepLinkStart, nextEffect)

        assertTrue(
            "CopyPaste-tqt0: the deep-link LaunchedEffect must set pendingProvisioningRaw.",
            deepLinkBlock.contains("pendingProvisioningRaw"),
        )
    }

    @Test
    fun `finalizeSync calls applyQrProvisioning post-PAKE`() {
        // CopyPaste-1jms.33: runPairAndSync was split into runBootstrap (PAKE only)
        // and finalizeSync (provisioning + sync + roster). The security invariant is
        // unchanged: applyQrProvisioning appears in finalizeSync, which only runs
        // after the user reviews the peer metadata and clicks "Confirm & sync".
        val src = pairActivitySrc

        val finalizeSyncStart = src.indexOf("fun finalizeSync(")
        assertTrue("PairActivity must have fun finalizeSync()", finalizeSyncStart >= 0)

        // finalizeSync receives a BootstrapResult (already validated by PAKE in runBootstrap).
        // applyQrProvisioning must appear inside finalizeSync.
        val applyIdx = src.indexOf("applyQrProvisioning(", finalizeSyncStart)

        assertTrue(
            "CopyPaste-tqt0: applyQrProvisioning must appear somewhere inside finalizeSync",
            applyIdx > finalizeSyncStart,
        )
        // runBootstrap must NOT call applyQrProvisioning.
        val runBootstrapStart = src.indexOf("fun runBootstrap(")
        assertTrue("PairActivity must have fun runBootstrap()", runBootstrapStart >= 0)
        val runBootstrapEnd = finalizeSyncStart  // runBootstrap ends before finalizeSync
        val bootstrapApplyIdx = src.indexOf("applyQrProvisioning(", runBootstrapStart)
        assertTrue(
            "CopyPaste-tqt0: applyQrProvisioning must NOT appear inside runBootstrap — " +
                "provisioning must be deferred to finalizeSync (post user review).",
            bootstrapApplyIdx < 0 || bootstrapApplyIdx >= runBootstrapEnd,
        )
    }

    @Test
    fun `pendingProvisioningRaw is cleared to null after successful pairing`() {
        val src = pairActivitySrc

        // After successful pairing, pendingProvisioningRaw = null must appear
        // inside finalizeSync so the retained payload is dropped post-confirmation.
        // CopyPaste-1jms.33: formerly checked in runPairAndSync; now in finalizeSync.
        val finalizeSyncStart = src.indexOf("fun finalizeSync(")
        assertTrue("PairActivity must have fun finalizeSync()", finalizeSyncStart >= 0)

        val clearIdx = src.indexOf("pendingProvisioningRaw = null", finalizeSyncStart)
        assertTrue(
            "CopyPaste-tqt0: pendingProvisioningRaw must be set to null inside finalizeSync " +
                "after successful pairing so the retained payload cannot leak into a future pair.",
            clearIdx > finalizeSyncStart,
        )
    }

    // ── 3. applyQrProvisioning fill-missing semantics (source inspection) ─────

    private val provisioningSrc: String by lazy {
        val candidates = listOf(
            "android/app/src/main/java/com/copypaste/android/PairProvisioning.kt",
            "../android/app/src/main/java/com/copypaste/android/PairProvisioning.kt",
            "../../android/app/src/main/java/com/copypaste/android/PairProvisioning.kt",
        )
        candidates
            .map { File(it) }
            .firstOrNull { it.exists() }
            ?.readText()
            ?: error("Could not locate PairProvisioning.kt from test working directory")
    }

    @Test
    fun `applyQrProvisioning guards each write with isBlank check`() {
        val src = provisioningSrc
        // Fill-missing semantics: only write when the existing value is blank.
        assertTrue(
            "applyQrProvisioning must guard relayUrl write with isBlank()",
            src.contains("settings.relayUrl.isBlank()"),
        )
        assertTrue(
            "applyQrProvisioning must guard supabaseUrl write with isBlank()",
            src.contains("settings.supabaseUrl.isBlank()"),
        )
        assertTrue(
            "applyQrProvisioning must guard supabaseAnonKey write with isBlank()",
            src.contains("settings.supabaseAnonKey.isBlank()"),
        )
    }
}
