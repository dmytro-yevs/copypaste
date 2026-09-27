package com.copypaste.app;

/** Signals only that Android recorded a clipboard access; no log line or content crosses IPC. */
oneway interface IClipCascadeCaptureListener {
    void onClipboardAccess() = 1;
    void onCaptureStopped() = 2;
}
