package com.copypaste.app;

interface IShizukuGrantService {
    boolean applyCaptureGrants(String packageName);
    boolean applySmsGrants(String packageName, int userId, boolean otpSupported);
    void destroy();
}
