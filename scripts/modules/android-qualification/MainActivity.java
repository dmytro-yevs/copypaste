package com.copypaste.qualification;

import android.app.Activity;
import android.os.Bundle;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.atomic.AtomicBoolean;

/** Runs the actual module host in app-private storage without Internet permission. */
public final class MainActivity extends Activity {
    private static final AtomicBoolean started = new AtomicBoolean();
    public static long[] openWorker() { return InferenceWorkerHost.openWorker(); }
    public static void closeWorker(long id) { InferenceWorkerHost.closeWorker(id); }
    static { System.loadLibrary("copypaste_module_qualification"); }
    private static native String qualify(
        String packagePath, String fixtures, String data, String appVersion,
        String commit, String runId, String phase
    );

    @Override
    public void onCreate(Bundle state) {
        super.onCreate(state);
        InferenceWorkerHost.initialize(getApplicationContext());
        // Activity recreation must not start a second workload in the same process.
        if (!started.compareAndSet(false, true)) return;
        new Thread(() -> {
            String phase = getIntent().getStringExtra("phase");
            if (phase == null) phase = "execute";
            File result = new File(getFilesDir(), phase + ".json");
            try {
                File packageFile = new File(getFilesDir(), "module.cpmodule");
                File fixtures = new File(getFilesDir(), "fixtures");
                if (!"cleanup".equals(phase)) {
                    copyAsset("module.cpmodule", packageFile);
                    copyDirectory("fixtures", fixtures);
                }
                String receipt = qualify(
                    packageFile.getAbsolutePath(), fixtures.getAbsolutePath(),
                    new File(getFilesDir(), "host").getAbsolutePath(),
                    getIntent().getStringExtra("appVersion"),
                    getIntent().getStringExtra("commit"),
                    getIntent().getStringExtra("runId"), phase
                );
                try (FileOutputStream output = new FileOutputStream(result)) {
                    output.write(receipt.getBytes(StandardCharsets.UTF_8));
                }
            } catch (Throwable error) {
                try (FileOutputStream output = new FileOutputStream(result)) {
                    output.write(("{\"failure\":\"" + error.getClass().getSimpleName() + "\"}").getBytes(StandardCharsets.UTF_8));
                } catch (Exception writeError) {
                    android.util.Log.e("CopyPasteQualification", "Could not write qualification result.", writeError);
                }
                android.util.Log.e("CopyPasteQualification", "Native module qualification failed.", error);
            }
            runOnUiThread(this::finish);
        }).start();
    }

    private void copyAsset(String name, File destination) throws Exception {
        try (InputStream source = getAssets().open(name);
             FileOutputStream output = new FileOutputStream(destination)) {
            byte[] buffer = new byte[64 * 1024];
            int count;
            while ((count = source.read(buffer)) != -1) output.write(buffer, 0, count);
        }
    }

    private void copyDirectory(String name, File destination) throws Exception {
        String[] children = getAssets().list(name);
        if (children.length == 0) {
            copyAsset(name, destination);
            return;
        }
        if (!destination.mkdir()) throw new IllegalStateException("Fixture storage already exists.");
        for (String child : children) copyDirectory(name + "/" + child, new File(destination, child));
    }
}
