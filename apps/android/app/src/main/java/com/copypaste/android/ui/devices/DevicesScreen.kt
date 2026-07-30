package com.copypaste.android.ui.devices

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.selection.DisableSelection
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.copypaste.android.R
import com.copypaste.android.ui.components.ErrorBanner
import com.copypaste.ffi.PairedDevice

/**
 * Paired devices: mint a code, accept one, unpair, check, sync.
 *
 * ## The pairing code
 *
 * INV-14: display-only. The code is drawn inside a [DisableSelection] block, so
 * long-press-to-select — which is how text is lifted on Android — does nothing.
 * There is no tap-to-copy affordance, because a live credential on the system
 * clipboard is readable by whatever the user pastes into next, and by the
 * clipboard-preview overlay on top of that.
 *
 * It is also never logged: the [PairingCode] wrapper it arrives in redacts its
 * own `toString`, so even `Log.d(TAG, "$state")` prints nothing useful.
 *
 * ## Peer names
 *
 * INV-15: a peer's name is self-reported and unverified until a session
 * confirms it, so it is labelled as such rather than presented as fact.
 */
@Composable
fun DevicesScreen(
    viewModel: DevicesViewModel,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(0.dp),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var showAccept by rememberSaveable { mutableStateOf(false) }
    var pendingUnpair by remember { mutableStateOf<PairedDevice?>(null) }

    Column(modifier = modifier.fillMaxSize().padding(horizontal = 12.dp)) {
        state.errorMessage?.let { message ->
            ErrorBanner(
                message = stringResource(message),
                onDismiss = viewModel::dismissError,
                modifier = Modifier.padding(vertical = 8.dp),
            )
        }

        Text(
            text = stringResource(R.string.devices_this_device, viewModel.thisDeviceName),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(vertical = 8.dp),
        )

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = viewModel::mintPairingCode) {
                Text(stringResource(R.string.devices_pair_new))
            }
            OutlinedButton(onClick = { showAccept = true }) {
                Text(stringResource(R.string.devices_enter_code))
            }
        }

        when {
            state.loading -> Box(
                Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(
                    Modifier.semantics { contentDescription = LOADING_LABEL },
                )
            }

            state.isEmpty -> Box(
                Modifier.fillMaxSize().padding(32.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = stringResource(R.string.devices_empty),
                    style = MaterialTheme.typography.bodyLarge,
                )
            }

            else -> LazyColumn(
                contentPadding = contentPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxSize().padding(top = 12.dp),
            ) {
                items(state.peers, key = { it.pairingId }) { peer ->
                    PeerRow(
                        peer = peer,
                        result = state.results[peer.pairingId],
                        onCheck = { viewModel.checkPeer(peer) },
                        onSync = { viewModel.syncPeer(peer) },
                        onUnpair = { pendingUnpair = peer },
                    )
                }
            }
        }
    }

    state.mintedCode?.let { code ->
        PairingCodeDialog(
            code = code,
            pairingId = state.mintedPairingId.orEmpty(),
            onDismiss = viewModel::dismissPairingCode,
        )
    }

    if (showAccept) {
        AcceptPairingDialog(
            inFlight = state.acceptInFlight,
            onSubmit = { code, address ->
                viewModel.acceptPairing(code, address)
                showAccept = false
            },
            onDismiss = { showAccept = false },
        )
    }

    // INV-18: one confirm dialog at a time, which `pendingUnpair` being a
    // single nullable slot enforces structurally.
    pendingUnpair?.let { peer ->
        AlertDialog(
            onDismissRequest = { pendingUnpair = null },
            title = { Text(stringResource(R.string.unpair_title)) },
            text = { Text(stringResource(R.string.unpair_body)) },
            confirmButton = {
                TextButton(onClick = {
                    viewModel.unpair(peer)
                    pendingUnpair = null
                }) { Text(stringResource(R.string.action_unpair)) }
            },
            dismissButton = {
                TextButton(onClick = { pendingUnpair = null }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }
}

@Composable
private fun PeerRow(
    peer: PairedDevice,
    result: PeerResult?,
    onCheck: () -> Unit,
    onSync: () -> Unit,
    onUnpair: () -> Unit,
) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Text(text = peer.name, style = MaterialTheme.typography.titleSmall)

            // INV-15. The name above came from the peer and nothing has
            // confirmed it, so it is labelled rather than presented as fact.
            Text(
                text = stringResource(R.string.devices_unverified),
                style = MaterialTheme.typography.labelSmall,
            )

            Text(
                text = peer.lastAddr ?: stringResource(R.string.devices_never_reached),
                style = MaterialTheme.typography.bodySmall,
            )

            when (result) {
                PeerResult.Busy -> Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.padding(top = 4.dp),
                ) {
                    CircularProgressIndicator(Modifier.padding(end = 8.dp))
                    Text(stringResource(R.string.devices_working))
                }

                is PeerResult.Reached -> Text(
                    text = stringResource(R.string.devices_reachable),
                    style = MaterialTheme.typography.bodySmall,
                )

                // The friendly sentence, resolved from a string resource. Never
                // the exception's own text (INV-12).
                is PeerResult.Failed -> Text(
                    text = stringResource(result.message),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )

                null -> Unit
            }

            Row(
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
            ) {
                TextButton(onClick = onCheck) {
                    Text(stringResource(R.string.action_check))
                }
                TextButton(onClick = onSync) {
                    Text(stringResource(R.string.action_sync))
                }
                TextButton(onClick = onUnpair) {
                    Text(stringResource(R.string.action_unpair))
                }
            }
        }
    }
}

/**
 * The code, shown once.
 *
 * [DisableSelection] is the load-bearing part: without it a long press selects
 * the text and Android's selection toolbar offers Copy, which is exactly the
 * lift INV-14 exists to prevent.
 */
@Composable
private fun PairingCodeDialog(
    code: PairingCode,
    pairingId: String,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.pairing_title)) },
        text = {
            Column {
                Text(stringResource(R.string.pairing_instructions))

                DisableSelection {
                    Text(
                        text = code.value,
                        // Monospaced so groups line up while the user reads it
                        // out. Still `sp`, so it grows with the font-size
                        // setting like everything else (A11Y-15).
                        fontFamily = FontFamily.Monospace,
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(vertical = 16.dp),
                    )
                }

                Text(
                    text = stringResource(R.string.pairing_warning),
                    style = MaterialTheme.typography.labelSmall,
                )

                if (pairingId.isNotEmpty()) {
                    // The non-secret handle. Shown so a user comparing two
                    // devices has something safe to compare.
                    Text(
                        text = stringResource(R.string.pairing_id, pairingId),
                        style = MaterialTheme.typography.labelSmall,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_done)) }
        },
    )
}

@Composable
private fun AcceptPairingDialog(
    inFlight: Boolean,
    onSubmit: (code: String, address: String) -> Unit,
    onDismiss: () -> Unit,
) {
    // Not `rememberSaveable`: a typed pairing code is a credential, and
    // `rememberSaveable` writes to the saved-instance-state bundle, which the
    // system persists to disk.
    var code by remember { mutableStateOf("") }
    var address by rememberSaveable { mutableStateOf("") }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.accept_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.accept_instructions))
                OutlinedTextField(
                    value = code,
                    onValueChange = { code = it },
                    label = { Text(stringResource(R.string.accept_code_label)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = address,
                    onValueChange = { address = it },
                    label = { Text(stringResource(R.string.accept_address_label)) },
                    placeholder = { Text(stringResource(R.string.accept_address_hint)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = !inFlight && code.isNotBlank() && address.isNotBlank(),
                onClick = { onSubmit(code, address) },
            ) { Text(stringResource(R.string.action_pair)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        },
    )
}

private const val LOADING_LABEL = "Loading paired devices"
