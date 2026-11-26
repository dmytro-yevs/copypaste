package com.copypaste.app

import android.content.pm.PackageInstaller

// PackageInstaller reports a package verifier rejection with the public status
// of a user cancellation; only its internal code tells the two apart.
internal const val appUpdateLegacyStatusExtra = "android.content.pm.extra.LEGACY_STATUS"
private const val verifierRejected = -22

internal fun appUpdateResultCode(status: Int, legacyStatus: Int = 0): String = when (status) {
    PackageInstaller.STATUS_SUCCESS -> "installed"
    PackageInstaller.STATUS_FAILURE_ABORTED ->
        if (legacyStatus == verifierRejected) "installation_blocked" else "installation_cancelled"
    PackageInstaller.STATUS_FAILURE_BLOCKED -> "installation_blocked"
    PackageInstaller.STATUS_FAILURE_CONFLICT -> "installation_conflict"
    PackageInstaller.STATUS_FAILURE_INCOMPATIBLE -> "installation_incompatible"
    PackageInstaller.STATUS_FAILURE_INVALID -> "package_invalid"
    PackageInstaller.STATUS_FAILURE_STORAGE -> "installation_storage"
    else -> "installation_failed"
}
