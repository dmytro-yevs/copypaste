package com.copypaste.android

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.MonoFontFamily
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// SAS pairing modal (port of macOS DevicesView SasPairingModal)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * CopyPaste-3vpq: peer metadata card shown inside the SAS dialog while
 * [status.state] == "awaiting_sas". Mirrors the macOS SasPairingModal which
 * displays the peer's model, OS, and IP so the user can verify they are pairing
 * with the right device before comparing the Short Authentication String.
 *
 * Only rows with non-null/non-blank values are rendered — early handshake polls
 * may not have received metadata yet (peerModel==null), so the card degrades
 * gracefully and is invisible when no fields are known.
 */
@Composable
private fun SasPeerMetadataCard(status: PairStatus) {
    val c = LocalCpColors.current
    // Pre-resolve string resources outside buildList (stringResource is @Composable;
    // it cannot be called inside a non-@Composable lambda like buildList).
    val labelModel = stringResource(R.string.meta_label_model)
    val labelOs = stringResource(R.string.meta_label_os)
    val labelVersion = stringResource(R.string.meta_label_version)
    val labelLocalIp = stringResource(R.string.meta_label_local_ip)
    val labelPublicIp = stringResource(R.string.meta_label_public_ip)
    val labelFingerprint = stringResource(R.string.meta_label_fingerprint)

    // Collect the non-blank field pairs we have. Order + fields mirror the macOS
    // SasPairingModal: Model, OS, Version, Local/Public IP, then Fingerprint.
    val fields = buildList {
        status.peerModel?.takeIf { it.isNotBlank() }?.let { add(labelModel to it) }
        status.peerOs?.takeIf { it.isNotBlank() }?.let { add(labelOs to it) }
        // CopyPaste-crh3.35: show the partner's app version (parity with macOS).
        status.peerAppVersion?.takeIf { it.isNotBlank() }?.let { add(labelVersion to it) }
        status.peerLocalIp?.takeIf { it.isNotBlank() }?.let { add(labelLocalIp to it) }
        status.peerPublicIp?.takeIf { it.isNotBlank() }?.let { add(labelPublicIp to it) }
        // CopyPaste-crh3.29: show the partner's fingerprint so a user can verify
        // it out-of-band (parity with macOS — impossible to verify without it).
        status.peerFingerprint?.takeIf { it.isNotBlank() }?.let { add(labelFingerprint to it) }
    }
    // Nothing to show yet — the card is silent (not even a placeholder).
    if (fields.isEmpty()) return

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(c.elevated, RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        fields.forEach { (label, value) ->
            MetaRow(label = label, value = value)
        }
    }
}

/**
 * Modal that drives a discovery-initiated SAS pairing to completion.
 *
 * Behaviour mirrors the macOS [SasPairingModal]:
 *  - polls [pairGetSas] every [SAS_POLL_MS];
 *  - `initiating` → spinner ("Connecting…");
 *  - `awaiting_sas` with a code → shows the 6-digit SAS + Match / Doesn't match;
 *  - `awaiting_sas` without a code → "Waiting for the other device…";
 *  - `confirmed` → persists the peer (KEK-wrapped session key + fill-missing
 *    provisioning) and shows success;
 *  - `rejected` / `aborted` / `timed_out` → error;
 *  - a TRAILING `idle` observed AFTER an active state is itself terminal
 *    ("pairing ended"): if the user already accepted locally, treat as success,
 *    else show a neutral "ended" close state — never loop on idle.
 *
 * Closing before a terminal state calls [pairAbort] exactly once; after any
 * terminal state [pairReset] is called to clear the native state machine.
 *
 * SECURITY: the SAS code is shown on screen but NEVER logged; the session-key
 * bytes are wrapped + zeroized and never logged.
 */
@Composable
internal fun SasPairingDialog(
    peer: DiscoveredPeer,
    settings: Settings,
    onClose: () -> Unit,
    onPaired: () -> Unit,
) {
    val c = LocalCpColors.current
    val scope = rememberCoroutineScope()

    // Current pairing status; starts optimistically at "initiating".
    var status by remember {
        mutableStateOf(
            PairStatus(
                state = "initiating",
                sas = null,
                role = null,
                peerFingerprint = null,
                peerSyncAddr = null,
                sessionKey = null,
                peerProvisioning = null,
                // ABI 14 (HB-1b): peer metadata, populated by the native side on confirm.
                peerModel = null,
                peerOs = null,
                peerAppVersion = null,
                peerLocalIp = null,
                peerPublicIp = null,
                peerDeviceId = null,
            )
        )
    }
    // Transient (non-terminal) poll/confirm error.
    var error by remember { mutableStateOf<String?>(null) }
    // True while a pairConfirmSas call is in flight (disables the buttons).
    var confirmPending by remember { mutableStateOf(false) }
    // Neutral terminal close state — handshake ended on a trailing idle without a
    // local confirm. Distinct from the wire `aborted` state.
    var ended by remember { mutableStateOf(false) }
    // True once a terminal Confirmed has been observed — closing then must NOT
    // call pairAbort (the pairing already succeeded).
    val confirmedRef = remember { mutableStateOf(false) }
    // True once the user locally accepted (clicked Match): disambiguates a
    // trailing idle (local-accepted + idle ⇒ success).
    val localAcceptedRef = remember { mutableStateOf(false) }

    val terminal = ended ||
        status.state == "confirmed" ||
        status.state == "rejected" ||
        status.state == "aborted" ||
        status.state == "timed_out"

    // Persist a confirmed pairing: KEK-wrap the session key, upsert the peer, and
    // apply peer provisioning fill-missing (copied from PairActivity). Runs on IO.
    suspend fun persistConfirmed(st: PairStatus) {
        val fingerprint = st.peerFingerprint ?: return
        val keyUBytes = st.sessionKey ?: return
        withContext(Dispatchers.IO) {
            val rawSessionKey = ByteArray(keyUBytes.size) { keyUBytes[it].toByte() }
            try {
                val (wrappedB64, ivB64) = settings.wrapSessionKey(rawSessionKey)
                val nowMs = System.currentTimeMillis()
                settings.upsertPeer(
                    PairedPeer(
                        fingerprint = fingerprint,
                        syncAddr = st.peerSyncAddr ?: "",
                        name = peer.deviceName,
                        sessionKeyWrappedB64 = wrappedB64,
                        sessionKeyIvB64 = ivB64,
                        lastSyncMs = nowMs,
                        pairedAtMs = nowMs,
                        // HB-1b (ABI 14): persist the peer's device metadata received
                        // over the discovery/SAS pairing for the Wave-3 device card.
                        peerModel = st.peerModel,
                        peerOs = st.peerOs,
                        peerAppVersion = st.peerAppVersion,
                        peerLocalIp = st.peerLocalIp,
                        peerPublicIp = st.peerPublicIp,
                        // CopyPaste-3k6m (ABI 17): persist the peer's stable device UUID so
                        // OriginDeviceFilter resolves clipboard item names by UUID.
                        peerDeviceId = st.peerDeviceId,
                    )
                )

                // Apply peer provisioning fill-missing — NEVER overwrite a value
                // this device already configured (mirror the daemon's rule and the
                // PairActivity QR block). Never log the derived key bytes.
                st.peerProvisioning?.let { prov ->
                    val applied = mutableListOf<String>()
                    prov.supabaseUrl?.takeIf { it.isNotBlank() }?.let { url ->
                        if (settings.supabaseUrl.isBlank()) {
                            settings.supabaseUrl = url
                            applied += "supabaseUrl"
                        }
                    }
                    prov.supabaseAnonKey?.takeIf { it.isNotBlank() }?.let { anon ->
                        if (settings.supabaseAnonKey.isBlank()) {
                            settings.supabaseAnonKey = anon
                            applied += "supabaseAnonKey"
                        }
                    }
                    prov.relayUrl?.takeIf { it.isNotBlank() }?.let { relay ->
                        if (settings.relayUrl.isBlank()) {
                            settings.relayUrl = relay
                            applied += "relayUrl"
                        }
                    }
                    prov.derivedSyncKey?.takeIf { it.isNotEmpty() }?.let { keyU ->
                        if (settings.cloudSyncKeyDirect == null) {
                            val keyBytes = ByteArray(keyU.size) { keyU[it].toByte() }
                            settings.cloudSyncKeyDirect = keyBytes
                            applied += "derivedSyncKey"
                        }
                    }
                    if (applied.isNotEmpty()) {
                        Log.i(TAG, "SAS provisioning applied (fill-missing): ${applied.joinToString(", ")}")
                    }
                }
            } finally {
                // Zero the raw session key copy once it has been wrapped.
                rawSessionKey.fill(0)
            }
        }
    }

    // Poll pair_get_sas until a terminal state. The native state machine resets to
    // idle after a terminal outcome, so a trailing idle (after an active state) is
    // itself terminal — never re-poll on it.
    // CopyPaste-crh3.27: pre-resolve the watchdog timeout message — stringResource
    // is @Composable and cannot be called inside the LaunchedEffect coroutine.
    val watchdogTimeoutMsg = stringResource(R.string.sas_watchdog_timeout)
    LaunchedEffect(peer.deviceId) {
        var sawActive = false
        // CopyPaste-crh3.27: UI watchdog — give up after SAS_WATCHDOG_MS (parity
        // with the macOS SasPairingModal) so the dialog never hangs forever on a
        // peer that never reaches a terminal state. Surfaces a timeout error;
        // Close then aborts the native pairing via handleClose (terminal stays
        // false, mirroring macOS where the watchdog sets an error, not a state).
        val watchdogDeadline = System.currentTimeMillis() + SAS_WATCHDOG_MS
        while (true) {
            if (System.currentTimeMillis() >= watchdogDeadline) {
                error = watchdogTimeoutMsg
                return@LaunchedEffect
            }
            val next = try {
                withContext(Dispatchers.IO) { pairGetSas() }
            } catch (e: Exception) {
                // CopyPaste-jwga: never surface raw exception detail to users.
                error = ErrorMessages.friendlySasError(e)
                return@LaunchedEffect
            }

            when (next.state) {
                "initiating", "awaiting_sas" -> {
                    sawActive = true
                    status = next
                    delay(SAS_POLL_MS)
                }
                "confirmed" -> {
                    confirmedRef.value = true
                    status = next
                    persistConfirmed(next)
                    onPaired()
                    pairReset()
                    return@LaunchedEffect
                }
                "rejected", "aborted", "timed_out" -> {
                    status = next
                    pairReset()
                    return@LaunchedEffect
                }
                else -> {
                    // state == "idle"
                    if (sawActive) {
                        if (confirmedRef.value || localAcceptedRef.value) {
                            confirmedRef.value = true
                            // Persist from the last status we held the keys on.
                            persistConfirmed(status)
                            status = PairStatus(
                                state = "confirmed",
                                sas = null,
                                role = null,
                                peerFingerprint = status.peerFingerprint,
                                peerSyncAddr = status.peerSyncAddr,
                                sessionKey = null,
                                peerProvisioning = null,
                                // HB-1b: carry forward the peer metadata we last held.
                                peerModel = status.peerModel,
                                peerOs = status.peerOs,
                                peerAppVersion = status.peerAppVersion,
                                peerLocalIp = status.peerLocalIp,
                                peerPublicIp = status.peerPublicIp,
                                // CopyPaste-3k6m: carry forward peer_device_id.
                                peerDeviceId = status.peerDeviceId,
                            )
                            onPaired()
                        } else {
                            ended = true
                        }
                        pairReset()
                        return@LaunchedEffect
                    }
                    // Idle before any active state — keep waiting.
                    status = next
                    delay(SAS_POLL_MS)
                }
            }
        }
    }

    // Close: abort the pairing unless it already succeeded (exactly once), then
    // ALWAYS reset the native pairing state machine.
    //
    // HB-8: pairAbort() moves the SM to the terminal `Aborted` state but leaves
    // `try_begin` claimed, so without a follow-up pairReset() every later pairing
    // attempt failed with "a pairing is already in flight". pairReset() returns
    // the SM to Idle. It is idempotent and safe whether we aborted, already hit a
    // terminal state, or the pairing succeeded.
    fun handleClose() {
        if (!confirmedRef.value && !terminal) {
            // Abort branch: abort, then reset, on the same IO dispatcher so the
            // reset is ordered AFTER the abort.
            scope.launch(Dispatchers.IO) {
                pairAbort()
                pairReset()
            }
        } else {
            // Already-terminal / confirmed branch: nothing to abort, but still
            // clear the SM so the next pairing can claim it.
            scope.launch(Dispatchers.IO) { pairReset() }
        }
        onClose()
    }

    fun handleConfirm(accept: Boolean) {
        confirmPending = true
        error = null
        // Record the local accept up-front so a trailing idle is read as success.
        if (accept) localAcceptedRef.value = true
        scope.launch {
            try {
                withContext(Dispatchers.IO) { pairConfirmSas(accept) }
                if (!accept) {
                    // User said it doesn't match — abort path already handled by
                    // the native side; close immediately.
                    onClose()
                    return@launch
                }
                // On accept keep polling; the next tick reflects confirmed/rejected.
            } catch (e: Exception) {
                // The decision never reached the native side — undo the optimistic
                // accept flag so a later trailing idle isn't misread as success.
                localAcceptedRef.value = false
                // CopyPaste-jwga: never surface raw exception detail to users.
                error = ErrorMessages.friendlySasError(e)
            } finally {
                confirmPending = false
            }
        }
    }

    val title = peer.displayName()

    // §8 glass SAS modal (audit #10, §10) — appearance only; pairing logic
    // (handleConfirm/handleClose, status machine) is untouched.
    GlassAlertDialog(
        onDismissRequest = { handleClose() },
        title = { Text("Pair “$title”") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                when {
                    ended -> {
                        Text(
                            "Pairing ended — check the other device.",
                            color = c.dim,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "confirmed" -> {
                        Text(
                            "Paired ✓",
                            color = c.ok,
                            style = MaterialTheme.typography.titleSmall,
                        )
                    }
                    status.state == "rejected" || status.state == "aborted" || status.state == "timed_out" -> {
                        Text(
                            when (status.state) {
                                "timed_out" -> "Pairing timed out."
                                "rejected" -> "Pairing was rejected."
                                else -> "Pairing was cancelled."
                            },
                            color = c.err,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "awaiting_sas" && status.sas != null -> {
                        // CopyPaste-3vpq: peer metadata card — macOS shows model/OS/IP during
                        // awaiting_sas. Rendered before the SAS prompt so the user can verify
                        // they are pairing with the right device before confirming the code.
                        SasPeerMetadataCard(status = status)
                        Text(
                            stringResource(R.string.sas_confirm_prompt),
                            color = c.dim,
                            style = MaterialTheme.typography.bodySmall,
                        )
                        // §10 SAS per-digit cells — styleguide .sas: each digit in its
                        // own 38dp-wide centered mono cell, 28sp/600, letterSpacing 1.1sp
                        // (≈.04em at 28sp), gap 8dp.
                        //
                        // CopyPaste-quux: the SAS code must NOT be copyable to the system
                        // clipboard. Copying it opens a sniff window — any other app that
                        // reads the clipboard during or after pairing gets the active pairing
                        // token. The row is display-only (no clickable, no long-press copy).
                        val sasFull = status.sas ?: ""
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 8.dp),
                        ) {
                            sasFull.forEach { digit ->
                                Box(
                                    contentAlignment = Alignment.Center,
                                    modifier = Modifier.width(38.dp),
                                ) {
                                    Text(
                                        text = digit.toString(),
                                        color = c.text,
                                        fontFamily = MonoFontFamily,
                                        fontSize = 28.sp,
                                        fontWeight = FontWeight.SemiBold,
                                        letterSpacing = 1.1.sp,
                                        textAlign = TextAlign.Center,
                                    )
                                }
                            }
                        }
                    }
                    status.state == "awaiting_sas" -> {
                        // CopyPaste-3vpq: show peer metadata even while waiting for the
                        // peer to accept — same macOS parity, displayed above the spinner.
                        SasPeerMetadataCard(status = status)
                        // Accepted locally; waiting for the peer to also accept.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(modifier = Modifier.size(18.dp))
                            Text(
                                stringResource(R.string.sas_waiting_other),
                                color = c.dim,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                    else -> {
                        // initiating / idle-before-active → connecting spinner.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(modifier = Modifier.size(18.dp))
                            Text(
                                stringResource(R.string.sas_connecting),
                                color = c.dim,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                }
                error?.let { msg ->
                    if (!terminal) {
                        Text(msg, color = c.err, style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        },
        confirmButton = {
            when {
                terminal -> {
                    CopyPasteButton(onClick = { onClose() }, variant = ButtonVariant.GHOST) { Text("Close") }
                }
                status.state == "awaiting_sas" && status.sas != null -> {
                    CopyPasteButton(
                        enabled = !confirmPending,
                        onClick = { handleConfirm(true) },
                        variant = ButtonVariant.PRIMARY,
                    ) { Text(if (confirmPending) "…" else "Match") }
                }
                else -> {}
            }
        },
        dismissButton = {
            when {
                terminal -> {}
                status.state == "awaiting_sas" && status.sas != null -> {
                    CopyPasteButton(
                        enabled = !confirmPending,
                        onClick = { handleConfirm(false) },
                        variant = ButtonVariant.GHOST,
                    ) { Text("Doesn't match") }
                }
                else -> {
                    CopyPasteButton(onClick = { handleClose() }, variant = ButtonVariant.GHOST) { Text("Cancel") }
                }
            }
        },
    )
}

private const val TAG = "DevicesActivity"
