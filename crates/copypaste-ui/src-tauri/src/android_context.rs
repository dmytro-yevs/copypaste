//! Hands the app's Android `Context` to `copypaste-core`'s keystore backend.
//!
//! That backend reaches the Android Keystore through
//! `android-native-keyring-store`, which finds the JavaVM and the application
//! context through the `ndk-context` crate. **Tauri does not populate
//! `ndk-context`** — tao keeps its own activity registry and wry deleted the
//! dependency in 0.51 — so the app has to, and this module is the whole of
//! that. Without it the first keystore call reports
//! `CryptoError::KeystoreUnavailable` and the app opens no history at all.
//!
//! It is not routed through a Tauri plugin like `capture::android` is, because
//! a plugin call cannot hand over a raw `JavaVM` pointer: `ndk-context` needs
//! the JNI frame itself.
//!
//! **Never compiled on this host** — `jni` and `ndk-context` are Android-only
//! dependencies. ADR-0003 records what a first device run would falsify.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::OnceLock;

use jni::objects::JObject;
use jni::JNIEnv;

/// Called from `MainActivity.onCreate`, before `super.onCreate` — Tauri's setup
/// opens the keystore during it, and the keystore needs this first.
///
/// Idempotent by construction: `initialize_android_context` asserts it is
/// called once, and `onCreate` runs again every time the activity is recreated.
///
/// Failures are silent on purpose. There is no user-facing thing to say here,
/// no Rust logging is configured this early, and the state it leaves behind is
/// one the keystore backend already reports properly.
#[no_mangle]
pub extern "system" fn Java_com_copypaste_app_KeystoreContext_initialize(
    env: JNIEnv,
    _this: JObject,
    context: JObject,
) {
    static INITIALIZED: OnceLock<()> = OnceLock::new();
    INITIALIZED.get_or_init(|| {
        let (Ok(context), Ok(vm)) = (env.new_global_ref(&context), env.get_java_vm()) else {
            return;
        };
        // The global reference is leaked deliberately: `ndk-context` keeps the
        // raw pointer for the life of the process, and dropping the reference
        // would leave it dangling. Kotlin passes the *application* context, so
        // what is pinned outlives every activity in any case.
        let raw = context.as_obj().as_raw();
        std::mem::forget(context);
        unsafe {
            ndk_context::initialize_android_context(
                vm.get_java_vm_pointer().cast::<c_void>(),
                raw.cast::<c_void>(),
            );
        }
    });
}
