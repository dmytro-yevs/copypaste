package com.copypaste.app;

interface IShizukuGrantService {
    boolean applyCaptureGrants(String packageName);
    void destroy();
}
