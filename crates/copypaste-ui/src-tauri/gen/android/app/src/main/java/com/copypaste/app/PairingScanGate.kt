package com.copypaste.app

internal enum class ScanStep {
    BUSY,
    REQUEST_PERMISSION,
    START_SCANNER,
    PERMISSION_DENIED,
}

internal class PairingScanGate {
    var inFlight: Boolean = false
        private set

    fun begin(permissionGranted: Boolean): ScanStep {
        if (inFlight) return ScanStep.BUSY
        inFlight = true
        return if (permissionGranted) ScanStep.START_SCANNER else ScanStep.REQUEST_PERMISSION
    }

    fun permissionResult(granted: Boolean): ScanStep {
        if (!inFlight) return ScanStep.BUSY
        if (granted) return ScanStep.START_SCANNER
        inFlight = false
        return ScanStep.PERMISSION_DENIED
    }

    fun finish() {
        inFlight = false
    }
}
