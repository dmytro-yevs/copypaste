package com.copypaste.android

import android.view.WindowManager
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

// ---------------------------------------------------------------------------
// android-pairing connected security check (S8, CopyPaste-myh8.8). Per the
// project's "connected-test CI availability" resolved decision, this run is
// REQUIRED LOCALLY for S8 (`:app:connectedDebugAndroidTest`) — CI stays
// advisory-only until CopyPaste-k1l0. No emulator is available in this
// sandbox; this class is written so it COMPILES
// (`:app:compileDebugAndroidTestKotlin`) and is ready for the pending local
// emulator run (bd-noted as outstanding).
//
// android-pairing spec, "Scanner window must set FLAG_SECURE (SECURITY)":
// asserts FLAG_SECURE on BOTH PairActivity (own-QR + scan-review + SAS/
// success flow) and PortraitCaptureActivity (the ZXing scanner, which
// previews the PEER's pairing QR — itself pairing material). FLAG_SECURE
// alone blocks both screenshot capture and the recents-switcher thumbnail —
// there is no separate "blocked recents" flag to assert.
//
// PortraitCaptureActivity extends zxing-android-embedded's CaptureActivity,
// which requests the CAMERA permission and starts the preview inside its own
// onCreate(); no androidx.test GrantPermissionRule dependency is wired for
// androidTest yet, so on a real device/emulator without CAMERA pre-granted
// the camera preview itself may fail asynchronously AFTER onCreate() returns
// — that does not affect this test, since FLAG_SECURE is set synchronously
// before super.onCreate() runs (see PortraitCaptureActivity KDoc).
// ---------------------------------------------------------------------------
@RunWith(AndroidJUnit4::class)
class PairSecurityConnectedTest {

    @Test
    fun pairActivitySetsFlagSecure() {
        ActivityScenario.launch(PairActivity::class.java).use { scenario ->
            scenario.onActivity { activity ->
                val flags = activity.window.attributes.flags
                assertTrue(
                    "PairActivity must set FLAG_SECURE unconditionally " +
                        "(pairing QR/PAKE material is on screen)",
                    flags and WindowManager.LayoutParams.FLAG_SECURE != 0,
                )
            }
        }
    }

    @Test
    fun portraitCaptureActivitySetsFlagSecure() {
        ActivityScenario.launch(PortraitCaptureActivity::class.java).use { scenario ->
            scenario.onActivity { activity ->
                val flags = activity.window.attributes.flags
                assertTrue(
                    "PortraitCaptureActivity must set FLAG_SECURE before the " +
                        "preview renders (P0-1 — the scanned peer QR is pairing material)",
                    flags and WindowManager.LayoutParams.FLAG_SECURE != 0,
                )
            }
        }
    }
}
