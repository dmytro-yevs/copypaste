package com.copypaste.qualification;

import android.app.Service;
import android.content.Intent;
import android.os.Binder;
import android.os.IBinder;
import android.os.ParcelFileDescriptor;
import android.os.Process;
import com.copypaste.app.IInferenceWorker;
import java.util.concurrent.atomic.AtomicBoolean;

/** Test-only process host for the shared production Rust inference protocol. */
public final class InferenceWorkerService extends Service {
    static { System.loadLibrary("copypaste_module_qualification"); }
    private static native void runWorker(int descriptor);
    private final AtomicBoolean opened = new AtomicBoolean();
    private final IInferenceWorker.Stub binder = new IInferenceWorker.Stub() {
        @Override public int open(ParcelFileDescriptor channel) {
            if (Binder.getCallingUid() != Process.myUid() || !opened.compareAndSet(false, true)) {
                throw new SecurityException("Invalid worker admission");
            }
            int descriptor = channel.detachFd();
            new Thread(() -> {
                try { runWorker(descriptor); }
                finally { Process.killProcess(Process.myPid()); }
            }, "qualification-inference").start();
            return Process.myPid();
        }
        @Override public void stop() {
            if (Binder.getCallingUid() != Process.myUid()) throw new SecurityException("Invalid worker owner");
            Process.killProcess(Process.myPid());
        }
    };
    @Override public IBinder onBind(Intent intent) { return binder; }
    @Override public void onDestroy() { super.onDestroy(); Process.killProcess(Process.myPid()); }
}
