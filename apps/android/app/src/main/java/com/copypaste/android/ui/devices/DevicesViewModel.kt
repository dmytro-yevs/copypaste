package com.copypaste.android.ui.devices

import androidx.annotation.StringRes
import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.copypaste.android.data.ClipboardRepository
import com.copypaste.android.data.friendlyMessage
import com.copypaste.ffi.PairedDevice
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** What one per-peer action did, for the row that asked for it. */
@Immutable
sealed interface PeerResult {
    data object Busy : PeerResult
    data class Reached(val atMs: Long) : PeerResult
    data class Failed(@StringRes val message: Int) : PeerResult
}

/**
 * The pairing code, held apart from everything else on purpose.
 *
 * `NewPairing` arrives from the FFI as a Kotlin `data class`, so its generated
 * `toString()` prints the code. This type exists so that never happens by
 * accident: the record is destructured the moment it arrives, the id goes into
 * [DevicesUiState] where the rest of the screen can see it, and the code is
 * carried by this one value whose [toString] is overridden to redact.
 *
 * Manifest 06 INV-14: a pairing secret is display-only. It reaches exactly one
 * composable, it is never put on the clipboard, and there is no code path that
 * writes it to a log.
 */
class PairingCode(val value: String) {
    /** Redacted. The whole reason this class exists rather than a `String`. */
    override fun toString(): String = "PairingCode(<redacted>)"
}

@Immutable
data class DevicesUiState(
    val peers: List<PairedDevice> = emptyList(),
    val loading: Boolean = true,
    /** The code being displayed, if the user has just minted one. */
    val mintedCode: PairingCode? = null,
    /** The non-secret id of that pairing — safe to keep, log and compare. */
    val mintedPairingId: String? = null,
    /** Per-peer action state, keyed by `pairingId`. */
    val results: Map<String, PeerResult> = emptyMap(),
    val acceptInFlight: Boolean = false,
    @StringRes val errorMessage: Int? = null,
) {
    val isEmpty: Boolean get() = !loading && peers.isEmpty()
}

/**
 * State for the Devices screen.
 *
 * There is no polling loop here. v1 polled `list_peers` every 10 s against a
 * daemon that might have learned something on its own; this app *is* the core,
 * so the peer list only changes when this ViewModel changes it. Adding a timer
 * would be a battery cost with nothing on the other end of it.
 */
class DevicesViewModel(private val repo: ClipboardRepository) : ViewModel() {

    private val _state = MutableStateFlow(DevicesUiState())
    val state: StateFlow<DevicesUiState> = _state.asStateFlow()

    val thisDeviceName: String get() = repo.deviceName

    init {
        refresh()
    }

    fun refresh() = viewModelScope.launch {
        try {
            _state.value = _state.value.copy(peers = repo.listPeers(), errorMessage = null)
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        } finally {
            // INV-30: the busy flag is released on every path.
            _state.value = _state.value.copy(loading = false)
        }
    }

    // -------------------------------------------------------------- pairing

    /**
     * Mint a pairing code to read out to the other device.
     *
     * The record is destructured immediately and never stored whole.
     */
    fun mintPairingCode() = viewModelScope.launch {
        try {
            val minted = repo.createPairing(PLACEHOLDER_PEER_NAME)
            _state.value = _state.value.copy(
                mintedCode = PairingCode(minted.code),
                mintedPairingId = minted.pairingId,
            )
            refresh()
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        }
    }

    /**
     * Stop showing the code.
     *
     * Called on every close path — the dialog's dismiss, its close button, and
     * the screen leaving the foreground. INV-16 asks that closing a pairing
     * flow always resets its state; there is no daemon state machine to abort
     * here, but the credential must still stop being on screen.
     */
    fun dismissPairingCode() {
        _state.value = _state.value.copy(mintedCode = null, mintedPairingId = null)
    }

    /**
     * Consume a code minted on the other device.
     *
     * Completes a real Noise handshake before the peer is stored, so a success
     * here means the pairing has worked at least once — not that a string was
     * accepted.
     */
    fun acceptPairing(code: String, address: String) = viewModelScope.launch {
        _state.value = _state.value.copy(acceptInFlight = true, errorMessage = null)
        try {
            repo.acceptPairing(code, address, PLACEHOLDER_PEER_NAME)
            refresh()
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        } finally {
            _state.value = _state.value.copy(acceptInFlight = false)
        }
    }

    // ---------------------------------------------------------- per-peer ops

    fun unpair(peer: PairedDevice) = viewModelScope.launch {
        setResult(peer.pairingId, PeerResult.Busy)
        try {
            repo.unpair(peer.pairingId)
            clearResult(peer.pairingId)
            refresh()
        } catch (e: Exception) {
            setResult(peer.pairingId, PeerResult.Failed(friendlyMessage(e)))
        }
    }

    /** Dial the peer and confirm it is reachable and still holds the pairing. */
    fun checkPeer(peer: PairedDevice) = viewModelScope.launch {
        setResult(peer.pairingId, PeerResult.Busy)
        try {
            val checked = repo.checkPeer(peer.pairingId)
            setResult(peer.pairingId, PeerResult.Reached(checked.lastSeenMs))
            refresh()
        } catch (e: Exception) {
            setResult(peer.pairingId, PeerResult.Failed(friendlyMessage(e)))
        }
    }

    /**
     * Sync history with the peer.
     *
     * Expected to fail with `SyncUnavailable` in this build — see
     * `crates/copypaste-ffi/src/pairing.rs` for exactly what is missing. The
     * button and its per-peer result are wired anyway so the path is real and
     * the copy is written; nothing here pretends the sync happened.
     */
    fun syncPeer(peer: PairedDevice) = viewModelScope.launch {
        setResult(peer.pairingId, PeerResult.Busy)
        try {
            repo.syncPeer(peer.pairingId)
            clearResult(peer.pairingId)
            refresh()
        } catch (e: Exception) {
            setResult(peer.pairingId, PeerResult.Failed(friendlyMessage(e)))
        }
    }

    fun dismissError() {
        _state.value = _state.value.copy(errorMessage = null)
    }

    // ------------------------------------------------------------- internals

    private fun setResult(pairingId: String, result: PeerResult) {
        _state.value = _state.value.copy(results = _state.value.results + (pairingId to result))
    }

    private fun clearResult(pairingId: String) {
        _state.value = _state.value.copy(results = _state.value.results - pairingId)
    }

    private companion object {
        /**
         * What a peer is called until it introduces itself.
         *
         * The real name arrives with the first sync session's hello, which this
         * build cannot run — so for now every peer keeps this until the user
         * has some other reason to know which is which. Honest placeholder
         * rather than a guess at the peer's identity.
         */
        const val PLACEHOLDER_PEER_NAME = "Paired device"
    }
}
