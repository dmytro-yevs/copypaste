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
    static { System.loadLibrary("copypaste_module_qualification"); }
    private static native String qualify(
        String packagePath, String fixtures, String data, String appVersion,
        String commit, String runId, String phase
    );

    @Override
    public void onCreate(Bundle state) {
        super.onCreate(state);
        // Activity recreation must not start a second workload in the same process.
        if (!started.compareAndSet(false, true)) return;
        new Thread(() -> {
            String phase = getIntent().getStringExtra("phase");
            if (phase == null) phase = "execute";
            File result = new File(getFilesDir(), phase + ".json");
            try {
                File packageFile = new File(getFilesDir(), "ocr.cpmodule");
                File fixtures = new File(getFilesDir(), "fixtures");
                if (!"cleanup".equals(phase)) {
                    copyAsset("ocr.cpmodule", packageFile);
                    if (!fixtures.mkdir()) throw new IllegalStateException("Fixture storage already exists.");
                    for (String name : getAssets().list("fixtures")) {
                        copyAsset("fixtures/" + name, new File(fixtures, name));
                    }
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
}
