package com.copypaste.app

import android.content.Intent
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private val notificationWaiters = ArrayList<(Boolean) -> Unit>()

  private val notificationPermission = registerForActivityResult(
    ActivityResultContracts.RequestPermission(),
  ) { granted ->
    val waiters = ArrayList(notificationWaiters)
    notificationWaiters.clear()
    waiters.forEach { it(granted) }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    // Before super.onCreate, which is where Tauri's setup runs: that setup
    // opens the history database, and opening it needs the device secret out
    // of the Android Keystore, which cannot be found until this has run.
    KeystoreContext.initialize(applicationContext)
    // INV-35, before anything is drawn. The frontend clears this later if the
    // user has turned "Allow screenshots" on; starting protected is what makes
    // a preference that never loads fail in the safe direction.
    window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
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

  fun requestNotificationPermission(onResult: (Boolean) -> Unit) {
    if (CaptureNotifications.isPermissionGranted(this)) {
      onResult(true)
      return
    }
    notificationWaiters.add(onResult)
    if (notificationWaiters.size == 1) {
      notificationPermission.launch(android.Manifest.permission.POST_NOTIFICATIONS)
    }
  }
}
