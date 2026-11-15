package com.copypaste.app

import android.content.pm.PackageInstaller

internal fun appUpdateResultCode(status: Int): String = when (status) {
    PackageInstaller.STATUS_SUCCESS -> "installed"
    PackageInstaller.STATUS_FAILURE_ABORTED -> "installation_cancelled"
    PackageInstaller.STATUS_FAILURE_BLOCKED -> "installation_blocked"
    PackageInstaller.STATUS_FAILURE_CONFLICT -> "installation_conflict"
    PackageInstaller.STATUS_FAILURE_INCOMPATIBLE -> "installation_incompatible"
    PackageInstaller.STATUS_FAILURE_INVALID -> "package_invalid"
    PackageInstaller.STATUS_FAILURE_STORAGE -> "installation_storage"
    else -> "installation_failed"
}
