package com.copypaste.app

/** Returning from the file picker must not recapture an unchanged clipboard. */
internal class ClipboardFocusCaptureGuard {
    private var beforePicker: Long? = null
    val hasPendingPicker: Boolean get() = beforePicker != null

    fun beginFilePicker(generation: Long?) { beforePicker = generation }
    fun abandonFilePicker() { beforePicker = null }

    fun shouldCaptureAfterFocus(generation: Long?): Boolean {
        val previous = beforePicker
        beforePicker = null
        // Missing generation evidence never suppresses a real clipboard read.
        return previous == null || generation == null || previous != generation
    }
}
