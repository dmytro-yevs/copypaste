package com.copypaste.android

import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val TAG = "DevicesRevokeActions"

/**
 * Unpair / revoke / revoke-and-rotate / revoke-all state + actions for the
 * Devices screen.
 *
 * CopyPaste-vp63.39: split out of [DevicesController] (which owns it via the
 * [DevicesController.revoke] property) purely to keep both files under the
 * 500-line budget — every body below was moved verbatim from the former
 * `DevicesScreen` god-composable in DevicesActivity.kt. [onRefresh] is
 * [DevicesController.refresh], threaded through so a successful mutation
 * re-reads the roster.
 */
class DevicesRevokeActions(
    private val settings: Settings,
    private val scope: CoroutineScope,
    private val onRefresh: () -> Unit,
) {
    // Per-peer dialog targets (null = no dialog showing). Settable directly by
    // PeerRow callbacks and the dialogs' dismiss handlers (DevicesDialogs.kt).
    var unpairTarget by mutableStateOf<PairedPeer?>(null)
    var revokeTarget by mutableStateOf<PairedPeer?>(null)

    // Non-null when an async revokeDeviceAudit IO call failed — surfaced to the user.
    var revokeError by mutableStateOf<String?>(null)
        private set

    // CopyPaste-8qcm: Revoke+rotate state — non-null when the passphrase dialog is open.
    // Holds the peer selected for revoke+rotate; [revokePassphrase] is the current input.
    var revokeRotateTarget by mutableStateOf<PairedPeer?>(null)
        private set
    var revokePassphrase by mutableStateOf("")

    // True while the revokeDeviceAndRotateKey FFI call is in-flight.
    var revokeRotateInFlight by mutableStateOf(false)
        private set

    // CopyPaste-crh3.34: "Revoke all" state — mirrors macOS revokeAllConfirm/revokeAllPending.
    var revokeAllConfirmOpen by mutableStateOf(false)
        private set
    var revokeAllInFlight by mutableStateOf(false)
        private set

    /** Runs the unpair action (settings mutation) and refreshes the roster. */
    fun confirmUnpair(target: PairedPeer) {
        unpairTarget = null
        unpairPeer(settings, target.fingerprint)
        onRefresh()
    }

    /**
     * First dialog's "Revoke & rotate key" primary action: closes the revoke
     * dialog and opens the passphrase dialog for [target].
     */
    fun openRevokeRotate(target: PairedPeer) {
        revokeTarget = null
        revokePassphrase = ""
        revokeRotateTarget = target
    }

    /**
     * First dialog's "Revoke only" action — plain audit + roster removal.
     *
     * CopyPaste-94o4: atomic revoke — write the audit record FIRST on the IO
     * dispatcher; only remove the peer from the local roster once the DB write
     * succeeds. A mid-write crash or DB error no longer leaves asymmetric state
     * (peer gone locally but no audit record). On failure the peer is untouched
     * and an error dialog is shown so the user can retry.
     */
    fun revokeOnly(target: PairedPeer) {
        revokeTarget = null
        scope.launch {
            val ok = withContext(Dispatchers.IO) {
                runCatching {
                    revokeDeviceAudit(
                        dbPath = settings.dbPath,
                        key = settings.encryptionKey,
                        fingerprint = target.fingerprint,
                        name = target.displayName(),
                    )
                }
            }.fold(
                onSuccess = { true },
                onFailure = { e ->
                    Log.e(
                        TAG,
                        "revokeDeviceAudit failed for ${target.fingerprint.take(8)}: ${e.message}",
                        e,
                    )
                    false
                },
            )
            if (ok) {
                settings.removePeer(target.fingerprint)
                // CopyPaste-1jms.8: log the missing peer-signal limitation
                // (same constraint as unpairPeer — no durable pending-unpair queue).
                Log.w(
                    TAG,
                    "revokeOnly: peer ${target.fingerprint.take(16)}… removed locally. " +
                        "No unpair signal sent to peer — Android lacks a durable " +
                        "pending-unpair queue (see CopyPaste-1jms.8).",
                )
                onRefresh()
            } else {
                revokeError = "Failed to record revocation. The device was NOT removed — please try again."
            }
        }
    }

    fun dismissRevokeError() {
        revokeError = null
    }

    /**
     * Dismisses the revoke+rotate passphrase dialog (cancel button or
     * onDismissRequest) — guarded so an in-flight rotation cannot be dismissed
     * out from under itself.
     */
    fun cancelRevokeRotate() {
        if (!canDismissRevokeRotate(revokeRotateInFlight)) return
        revokeRotateTarget = null
        revokePassphrase = ""
    }

    /**
     * Confirms "Revoke & rotate key": rotates the sync passphrase then removes
     * the peer, mirroring the macOS revoke_and_rotate semantics.
     *
     * Security ordering:
     *   1. revokeDeviceAndRotateKey derives the new key from the passphrase via
     *      Argon2id BEFORE any DB write — a bad passphrase leaves state unchanged.
     *   2. On success: the new sync key is persisted in Settings, the peer is
     *      removed from the roster, and updateP2pListenerPeers is called with the
     *      revoked fingerprint in the denylist.
     *   3. On failure: the peer is untouched (same CopyPaste-94o4 guarantee).
     *
     * The returned new key bytes are NEVER logged (SECURITY: secret material).
     */
    fun confirmRevokeRotate() {
        val t = revokeRotateTarget ?: return
        val passphrase = revokePassphrase
        if (!isValidRotatePassphrase(passphrase)) return
        revokeRotateInFlight = true
        scope.launch {
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    // The single per-account derivation needs the stable Supabase
                    // account id ("<project_ref>|<user_id>"); captured into Settings
                    // on the last sign-in.
                    val userId = settings.supabaseUserId
                    if (userId.isBlank()) {
                        throw IllegalStateException(
                            "sign in to your Supabase account before rotating the sync passphrase",
                        )
                    }
                    val accountId = supabaseAccountId(settings.supabaseUrl, userId)
                    // revokeDeviceAndRotateKey: derives new key FIRST (bad
                    // passphrase → DecryptionFailed, no DB write), then writes the
                    // audit record + removes the peer row. Returns the new 32-byte
                    // raw sync key.
                    revokeDeviceAndRotateKey(
                        dbPath = settings.dbPath,
                        key = settings.encryptionKey,
                        fingerprint = t.fingerprint,
                        name = t.displayName(),
                        newPassphrase = passphrase,
                        accountId = accountId,
                    )
                }
            }
            revokeRotateInFlight = false
            result.fold(
                onSuccess = { newKeyBytes ->
                    // Persist the new passphrase so the next sync re-derives the
                    // key identically. NEVER log the passphrase or bytes.
                    settings.cloudSyncPassphrase = passphrase
                    newKeyBytes.fill(0) // zero raw key bytes after persisting
                    // Remove peer from roster (audit record already written by FFI).
                    settings.removePeer(t.fingerprint)
                    revokeRotateTarget = null
                    revokePassphrase = ""
                    onRefresh()
                },
                onFailure = { e ->
                    Log.e(
                        TAG,
                        "revokeDeviceAndRotateKey failed for ${t.fingerprint.take(8)}: ${e.message}",
                        e,
                    )
                    revokeError = "Revoke + key rotation failed: ${e.message ?: "unknown error"}. " +
                        "The device was NOT removed — please try again."
                    revokeRotateTarget = null
                    revokePassphrase = ""
                },
            )
        }
    }

    fun openRevokeAllConfirm() {
        revokeAllConfirmOpen = true
    }

    /** Guarded dismiss — the confirmation cannot be closed while revoking. */
    fun dismissRevokeAllConfirm() {
        if (canDismissRevokeAllConfirm(revokeAllInFlight)) {
            revokeAllConfirmOpen = false
        }
    }

    /**
     * CopyPaste-crh3.34: "Revoke all" — sequentially audits + removes every
     * peer in [peersToRevoke] (a snapshot of [DevicesController.peers] taken by
     * the caller so mutations during the IO loop don't see a stale iterator).
     */
    fun confirmRevokeAll(peersToRevoke: List<PairedPeer>) {
        revokeAllConfirmOpen = false
        revokeAllInFlight = true
        scope.launch {
            var anyFailed = false
            for (p in peersToRevoke) {
                // CopyPaste-94o4 ordering: write the audit record first; only
                // remove the peer from the roster once the DB write succeeds —
                // same guarantee as the single-peer "Revoke only" path.
                val ok = withContext(Dispatchers.IO) {
                    runCatching {
                        revokeDeviceAudit(
                            dbPath = settings.dbPath,
                            key = settings.encryptionKey,
                            fingerprint = p.fingerprint,
                            name = p.displayName(),
                        )
                    }
                }.fold(
                    onSuccess = { true },
                    onFailure = { e ->
                        Log.e(
                            TAG,
                            "revokeAll: audit failed for ${p.fingerprint.take(8)}: ${e.message}",
                            e,
                        )
                        false
                    },
                )
                if (ok) {
                    // CopyPaste-1jms.8: local removal only — no outbound unpair
                    // signal on Android (no durable pending-unpair queue yet).
                    settings.removePeer(p.fingerprint)
                } else {
                    anyFailed = true
                }
            }
            revokeAllInFlight = false
            if (anyFailed) {
                revokeError = "Some devices could not be fully revoked. " +
                    "Remaining devices were NOT removed — please retry."
            }
            onRefresh()
        }
    }
}
