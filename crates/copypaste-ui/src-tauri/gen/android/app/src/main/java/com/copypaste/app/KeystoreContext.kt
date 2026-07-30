package com.copypaste.app

import android.content.Context

/**
 * Hands the application context to the Rust side, which needs it to reach the
 * Android Keystore that wraps the device secret.
 *
 * This reports a fact and decides nothing, which is the line drawn in
 * `capture/mod.rs`: whether the keystore is reachable, and what to tell the
 * user if it is not, is decided in Rust.
 *
 * The *application* context, not the activity: the native side pins it for the
 * life of the process, and an activity reference would dangle the moment the
 * activity is recreated.
 */
object KeystoreContext {
  init {
    System.loadLibrary("copypaste_ui_lib")
  }

  external fun initialize(context: Context)
}
