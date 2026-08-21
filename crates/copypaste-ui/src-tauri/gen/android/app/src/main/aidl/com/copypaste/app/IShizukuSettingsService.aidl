package com.copypaste.app;

interface IShizukuSettingsService {
    void destroy() = 16777114;
    boolean setClipboardAccessNotifications(boolean suppressed) = 1;
    boolean preparePersistentCaptureState(String packageName) = 2;
    boolean refreshClipCascadeSetup(String packageName) = 3;
}
