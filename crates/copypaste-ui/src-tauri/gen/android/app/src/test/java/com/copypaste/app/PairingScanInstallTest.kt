package com.copypaste.app

import com.google.android.gms.common.moduleinstall.ModuleInstallStatusUpdate
import org.junit.Assert.assertEquals
import org.junit.Test

class PairingScanInstallTest {
    @Test
    fun acceptedInstallWaitsForCompletionUnlessTheModuleAlreadyExists() {
        val install = PairingScanInstall()

        assertEquals(ScanInstallStep.WAIT, install.requested(alreadyInstalled = false))
        assertEquals(ScanInstallStep.START_SCANNER, install.requested(alreadyInstalled = true))
    }

    @Test
    fun onlyCompletedInstallStartsTheScanner() {
        val install = PairingScanInstall()

        assertEquals(ScanInstallStep.WAIT, install.updated(ModuleInstallStatusUpdate.InstallState.STATE_PENDING))
        assertEquals(ScanInstallStep.WAIT, install.updated(ModuleInstallStatusUpdate.InstallState.STATE_DOWNLOADING))
        assertEquals(ScanInstallStep.START_SCANNER, install.updated(ModuleInstallStatusUpdate.InstallState.STATE_COMPLETED))
        assertEquals(ScanInstallStep.FAILED, install.updated(ModuleInstallStatusUpdate.InstallState.STATE_FAILED))
        assertEquals(ScanInstallStep.FAILED, install.updated(ModuleInstallStatusUpdate.InstallState.STATE_CANCELED))
    }
}
