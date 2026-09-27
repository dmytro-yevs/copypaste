package com.copypaste.app

internal enum class ScanStep {
    BUSY,
    START_SCANNER,
}

internal class PairingScanGate {
    var inFlight: Boolean = false
        private set

    fun begin(): ScanStep {
        if (inFlight) return ScanStep.BUSY
        inFlight = true
        return ScanStep.START_SCANNER
    }

    fun finish() {
        inFlight = false
    }
}
