package com.copypaste.android.data

import androidx.annotation.StringRes
import com.copypaste.android.R
import com.copypaste.android.keystore.DeviceSecret
import com.copypaste.android.keystore.DeviceSecretUnavailable
import com.copypaste.ffi.CopyPasteException

/**
 * The single code → copy mapping. Manifest 06 INV-12.
 *
 * **No code path in this app may render an exception's own text.** Not
 * `e.message`, not `e.toString()`, not `Log`-then-show. In v1 that leaked the
 * daemon's Unix socket path into the DOM, into screenshots and into the
 * accessibility tree, and the socket path contains the local username
 * (`CopyPaste-tzzu`, `CopyPaste-j5qg`). On Android the equivalent is the app's
 * private data directory, which is just as much a disclosure.
 *
 * Two things make that a property rather than a promise:
 *
 * 1. `CopyPasteException`'s subclasses carry **no fields**, by construction in
 *    `copypaste-ffi`. Their `message` is the empty string. There is nothing to
 *    leak even if someone does render it.
 * 2. This function is the only place an exception becomes text, and it returns
 *    a string *resource id* rather than a string — so the mapping is exhaustive
 *    at compile time, and the copy is translatable.
 *
 * The raw throwable is for the log and the crash reporter, never the screen.
 */
@StringRes
fun friendlyMessage(error: Throwable): Int = when (error) {
    // ---- the core -------------------------------------------------------
    is CopyPasteException.Locked -> R.string.error_locked
    is CopyPasteException.LegacyData -> R.string.error_legacy_data
    is CopyPasteException.ItemNotFound -> R.string.error_item_gone
    is CopyPasteException.EmptyContent -> R.string.error_nothing_to_save
    is CopyPasteException.Crypto -> R.string.error_unreadable_item
    is CopyPasteException.Storage -> R.string.error_storage

    // ---- pairing --------------------------------------------------------
    is CopyPasteException.InvalidPairingCode -> R.string.error_bad_code
    is CopyPasteException.InvalidAddress -> R.string.error_bad_address
    is CopyPasteException.PairingRefused -> R.string.error_pairing_refused
    is CopyPasteException.PeerNotFound -> R.string.error_no_such_device
    is CopyPasteException.PeerAddressUnknown -> R.string.error_never_reached
    is CopyPasteException.PeerUnreachable -> R.string.error_no_response
    is CopyPasteException.PeerStore -> R.string.error_device_list
    is CopyPasteException.SyncUnavailable -> R.string.error_sync_unavailable

    // A caller bug rather than a user-facing condition, but it still has to
    // render as *something* rather than as a raw type name.
    is CopyPasteException.BadDeviceSecret -> R.string.error_storage

    // ---- the keystore ---------------------------------------------------
    is DeviceSecretUnavailable -> when (error.reason) {
        DeviceSecret.Reason.KEY_INVALIDATED -> R.string.error_keystore_invalidated
        DeviceSecret.Reason.UNREADABLE -> R.string.error_keystore_unreadable
    }

    // ---- anything else --------------------------------------------------
    // Deliberately not `error.message`. An unexpected throwable is exactly the
    // case where the text is least likely to have been vetted.
    else -> R.string.error_unexpected
}
