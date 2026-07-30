package com.copypaste.app

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Before super.onCreate, which is where Tauri's setup runs: that setup
    // opens the history database, and opening it needs the device secret out
    // of the Android Keystore, which cannot be found until this has run.
    KeystoreContext.initialize(applicationContext)
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  /**
   * The activity is `singleTask`, so a second launch arrives here rather than
   * in [onCreate] and `getIntent()` would otherwise still hold the first one.
   * `CapturePlugin` reads the re-arm extra off it, and a stale intent is the
   * difference between the loss notification landing on the right screen and
   * doing nothing at all.
   */
  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
  }
}
