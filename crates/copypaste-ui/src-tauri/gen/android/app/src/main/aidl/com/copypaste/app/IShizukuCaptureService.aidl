package com.copypaste.app;

import com.copypaste.app.IClipCascadeCaptureListener;

interface IShizukuCaptureService {
    void destroy() = 16777114;
    boolean start(IClipCascadeCaptureListener listener) = 1;
    void stop() = 2;
}
