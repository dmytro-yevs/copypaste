package com.copypaste.app

import com.google.android.gms.common.moduleinstall.ModuleInstallStatusUpdate

internal enum class ScanInstallStep {
    WAIT,
    START_SCANNER,
    FAILED,
}

internal class PairingScanInstall {
    fun requested(alreadyInstalled: Boolean): ScanInstallStep =
        if (alreadyInstalled) ScanInstallStep.START_SCANNER else ScanInstallStep.WAIT

    fun updated(state: Int): ScanInstallStep = when (state) {
        ModuleInstallStatusUpdate.InstallState.STATE_COMPLETED -> ScanInstallStep.START_SCANNER
        ModuleInstallStatusUpdate.InstallState.STATE_FAILED,
        ModuleInstallStatusUpdate.InstallState.STATE_CANCELED -> ScanInstallStep.FAILED
        else -> ScanInstallStep.WAIT
    }
}
