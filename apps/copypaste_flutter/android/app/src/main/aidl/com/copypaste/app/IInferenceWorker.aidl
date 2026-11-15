package com.copypaste.app;

import android.os.ParcelFileDescriptor;

interface IInferenceWorker {
    int open(in ParcelFileDescriptor channel);
    void stop();
}
