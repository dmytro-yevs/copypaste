package com.copypaste.app

import android.Manifest
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.WebView
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private val notificationWaiters = ArrayList<(NotificationPermissionFacts) -> Unit>()
  private var imeInsetsHost: WebView? = null
  private val permissionPreferences by lazy {
    getSharedPreferences(PERMISSION_PREFERENCES, Context.MODE_PRIVATE)
  }

  private val notificationPermission = registerForActivityResult(
    ActivityResultContracts.RequestPermission(),
  ) {
    val waiters = ArrayList(notificationWaiters)
    notificationWaiters.clear()
    val facts = notificationPermissionFacts()
    waiters.forEach { it(facts) }
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

  override fun onContentChanged() {
    super.onContentChanged()
    // Tauri/Wry installs its RustWebView with setContentView, so this callback
    // reaches the actual host view synchronously before an accessibility dump.
    findHostWebView(window.decorView)?.let { webView ->
      webView.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
      if (imeInsetsHost !== webView) {
        imeInsetsHost = webView
        WebViewImeInsets.install(webView)
      }
    }
  }

  private fun findHostWebView(view: View): WebView? {
    if (view is WebView) return view
    if (view !is ViewGroup) return null
    for (index in 0 until view.childCount) {
      findHostWebView(view.getChildAt(index))?.let { return it }
    }
    return null
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

  fun requestNotificationPermission(onResult: (NotificationPermissionFacts) -> Unit) {
    permissionPreferences.edit().putBoolean(ASKED_NOTIFICATIONS, true).apply()
    if (CaptureNotifications.isPermissionGranted(this)) {
      onResult(notificationPermissionFacts())
      return
    }
    notificationWaiters.add(onResult)
    if (notificationWaiters.size == 1) {
      notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
    }
  }

  fun notificationPermissionFacts(): NotificationPermissionFacts {
    val rationale = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
      shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS)
    return NotificationPermissionFacts(
      apiLevel = Build.VERSION.SDK_INT,
      granted = CaptureNotifications.isPermissionGranted(this),
      everAsked = permissionPreferences.getBoolean(ASKED_NOTIFICATIONS, false),
      showRationale = rationale,
    )
  }

  private companion object {
    const val PERMISSION_PREFERENCES = "onboarding-permissions"
    const val ASKED_NOTIFICATIONS = "asked_notifications"
  }
}
