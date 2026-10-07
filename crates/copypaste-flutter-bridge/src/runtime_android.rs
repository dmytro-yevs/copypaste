//! Android process bootstrap for the in-process CopyPaste runtime.
//!
//! Android Keystore access is intentionally initialized before the Flutter
//! engine starts Rust work. `Keyring::load_or_create` depends on the process
//! global NDK context supplied here; opening the encrypted history first would
//! otherwise fail instead of creating or reading the app-private key.

#![cfg(target_os = "android")]

use std::ffi::c_void;
use std::sync::{Arc, OnceLock};

use jni::{
    objects::{GlobalRef, JByteArray, JClass, JObject, JValue},
    sys::{jboolean, jlong, JNI_FALSE, JNI_TRUE},
    JNIEnv, JavaVM,
};

/// Keeps the application context alive for as long as Rust can access Android
/// Keystore or other context-backed services.
static APPLICATION_CONTEXT: OnceLock<GlobalRef> = OnceLock::new();
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
static RUNTIME: OnceLock<Arc<copypaste_runtime::Runtime>> = OnceLock::new();
static SMS_NETWORK_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Initializes the context required by `android-native-keyring-store`.
///
/// `MainActivity` invokes this exactly once from `onCreate`, before it calls
/// into Flutter. Repeated activity creation is harmless because the process
/// retains the original application context. The JNI entry point intentionally
/// has no Dart-facing data or clipboard behavior.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_MainActivity_initializeNdkContext(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
) {
    if APPLICATION_CONTEXT.get().is_some() {
        return;
    }

    let Ok(application_context) = env.new_global_ref(context) else {
        tracing::error!("could not retain the Android application context");
        return;
    };
    let Ok(java_vm) = env.get_java_vm() else {
        tracing::error!("could not access the Android Java VM");
        return;
    };
    let _ = JAVA_VM.set(java_vm);

    if APPLICATION_CONTEXT.set(application_context).is_ok() {
        let application_context = APPLICATION_CONTEXT
            .get()
            .expect("the Android application context was stored");
        // The activity calls this before the Flutter engine makes its first
        // Rust request. `ndk_context` requires exactly one initialization for
        // the process, and this GlobalRef makes the context valid afterwards.
        unsafe {
            ndk_context::initialize_android_context(
                JAVA_VM
                    .get()
                    .expect("the Java VM was stored")
                    .get_java_vm_pointer() as *mut c_void,
                application_context.as_obj().as_raw() as *mut c_void,
            );
        }
    }
}

/// Opens the in-process runtime from the Android-owned files directory.
///
/// Kotlin resolves the path rather than Rust guessing a shared or external
/// location. The runtime is process-global by design: a second activity must
/// reuse the same encrypted store and P2P listener.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_MainActivity_initializeRuntime(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JObject,
    device_name: JObject,
    model: JObject,
    os_version: JObject,
    device_class: JObject,
) {
    if RUNTIME.get().is_some() {
        return;
    }
    let Ok(data_dir) = env.get_string((&data_dir).into()) else {
        return;
    };
    let Ok(device_name) = env.get_string((&device_name).into()) else {
        return;
    };
    let (Ok(model), Ok(os_version), Ok(device_class)) = (
        env.get_string((&model).into()),
        env.get_string((&os_version).into()),
        env.get_string((&device_class).into()),
    ) else {
        return;
    };
    let data_dir = std::path::PathBuf::from(data_dir.to_string_lossy().into_owned());
    let device_name = device_name.to_string_lossy().into_owned();
    copypaste_p2p::DeviceProfile::set_android_hardware_profile(
        copypaste_p2p::AndroidHardwareProfile {
            model: Some(model.to_string_lossy().into_owned()),
            os_version: Some(os_version.to_string_lossy().into_owned()),
            device_class: copypaste_ipc::DeviceClass::from_wire_name(
                &device_class.to_string_lossy(),
            ),
        },
    );
    match copypaste_runtime::Runtime::open_with_clipboard(
        &data_dir,
        &device_name,
        copypaste_p2p::DEFAULT_PORT,
        Arc::new(AndroidClipboard),
    ) {
        Ok(runtime) => {
            let _ = RUNTIME.set(Arc::new(runtime));
        }
        Err(error) => tracing::error!(%error, "could not open Android runtime"),
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_openHost(
    _env: JNIEnv,
    _class: JClass,
    explicit: jboolean,
) -> jlong {
    RUNTIME
        .get()
        .and_then(|runtime| {
            runtime
                .capture_admission()
                .open_host(if explicit == JNI_TRUE {
                    copypaste_runtime::capture_admission::CaptureKind::Explicit
                } else {
                    copypaste_runtime::capture_admission::CaptureKind::Implicit
                })
        })
        .unwrap_or(0) as jlong
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_revokeHost(
    _env: JNIEnv,
    _class: JClass,
    host: jlong,
) -> jboolean {
    if host > 0
        && RUNTIME
            .get()
            .is_some_and(|runtime| runtime.capture_admission().revoke_host(host as u64).is_ok())
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_drainHost(
    _env: JNIEnv,
    _class: JClass,
    host: jlong,
) -> jboolean {
    if host > 0
        && RUNTIME
            .get()
            .is_some_and(|runtime| runtime.capture_admission().drain_host(host as u64).is_ok())
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_begin(
    mut env: JNIEnv,
    _class: JClass,
    host: jlong,
    cancellation: JObject,
) -> jlong {
    if host <= 0 {
        return 0;
    }
    let Some(runtime) = RUNTIME.get() else {
        return 0;
    };
    let Ok(callback) = env.new_global_ref(cancellation) else {
        return 0;
    };
    runtime
        .capture_admission()
        .begin(
            host as u64,
            Arc::new(move || {
                if let Some(vm) = JAVA_VM.get() {
                    if let Ok(mut env) = vm.attach_current_thread() {
                        if env
                            .call_method(callback.as_obj(), "run", "()V", &[])
                            .is_err()
                        {
                            let _ = env.exception_clear();
                        }
                    }
                }
            }),
        )
        .unwrap_or(0) as jlong
}

/// Rust keeps the owned permit through Java execution and every exception path.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_scoped(
    mut env: JNIEnv,
    _class: JClass,
    token: jlong,
    completion: jboolean,
    content_type: JObject,
    callback: JObject,
) -> jboolean {
    if token <= 0 {
        return JNI_FALSE;
    }
    let Some(runtime) = RUNTIME.get() else {
        return JNI_FALSE;
    };
    let scope = if completion == JNI_TRUE {
        copypaste_runtime::capture_admission::CaptureScope::Completion
    } else {
        copypaste_runtime::capture_admission::CaptureScope::Read
    };
    let Some(permit) = runtime.capture_admission().acquire(token as u64, scope) else {
        return JNI_FALSE;
    };
    let Ok(content_type) = env.get_string((&content_type).into()) else {
        return JNI_FALSE;
    };
    let limit = permit
        .config
        .capture_limit_bytes(&content_type.to_string_lossy());
    if env
        .call_method(
            callback,
            "run",
            "(J)V",
            &[JValue::Long(limit.min(i64::MAX as u64) as jlong)],
        )
        .is_ok()
    {
        JNI_TRUE
    } else {
        let _ = env.exception_clear();
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_abandon(
    _env: JNIEnv,
    _class: JClass,
    token: jlong,
) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.capture_admission().abandon(token as u64);
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_ingestText(
    mut env: JNIEnv,
    _class: JClass,
    token: jlong,
    text: JObject,
) -> jboolean {
    if token <= 0 {
        return JNI_FALSE;
    }
    let Some(runtime) = RUNTIME.get() else {
        return JNI_FALSE;
    };
    let Some(read) = runtime.capture_admission().acquire(
        token as u64,
        copypaste_runtime::capture_admission::CaptureScope::Read,
    ) else {
        return JNI_FALSE;
    };
    let Ok(text) = env.get_string((&text).into()) else {
        return JNI_FALSE;
    };
    let text = text.to_string_lossy().into_owned();
    drop(read);
    if runtime.capture_text_operation(token as u64, &text).is_ok() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_ingestBinary(
    mut env: JNIEnv,
    _class: JClass,
    token: jlong,
    bytes: JByteArray,
    content_type: JObject,
    filename: JObject,
    source_reference: JObject,
) -> jboolean {
    if token <= 0 {
        return JNI_FALSE;
    }
    let Some(runtime) = RUNTIME.get() else {
        return JNI_FALSE;
    };
    let Some(read) = runtime.capture_admission().acquire(
        token as u64,
        copypaste_runtime::capture_admission::CaptureScope::Read,
    ) else {
        return JNI_FALSE;
    };
    let (Ok(bytes), Ok(content_type), Ok(filename), Ok(source_reference)) = (
        env.convert_byte_array(bytes),
        env.get_string((&content_type).into()),
        env.get_string((&filename).into()),
        env.get_string((&source_reference).into()),
    ) else {
        return JNI_FALSE;
    };
    let content_type = content_type.to_string_lossy().into_owned();
    let filename = filename.to_string_lossy().into_owned();
    let source_reference = source_reference.to_string_lossy().into_owned();
    drop(read);
    if runtime
        .capture_binary_operation(
            token as u64,
            &bytes,
            &content_type,
            (!filename.is_empty()).then_some(filename.as_str()),
            (!source_reference.is_empty()).then_some(source_reference.as_str()),
        )
        .is_ok()
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_setCaptureRunning(
    _env: JNIEnv,
    _class: JClass,
    running: jboolean,
) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.set_capture_running(running == JNI_TRUE);
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_notifyOnCopyEnabled(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if RUNTIME
        .get()
        .is_some_and(|runtime| runtime.notify_on_copy_enabled())
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_notificationPreviewEnabled(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if RUNTIME
        .get()
        .is_some_and(|runtime| runtime.notification_preview_enabled())
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_soundOnCopyEnabled(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if RUNTIME
        .get()
        .is_some_and(|runtime| runtime.sound_on_copy_enabled())
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeSmsModules_hasHandler(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if RUNTIME
        .get()
        .is_some_and(|runtime| runtime.has_sms_module())
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeSmsModules_openHost(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    RUNTIME
        .get()
        .and_then(|runtime| {
            runtime
                .capture_admission()
                .open_host(copypaste_runtime::capture_admission::CaptureKind::ModuleEvent)
        })
        .unwrap_or(0) as jlong
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeSmsModules_ingest(
    mut env: JNIEnv,
    _class: JClass,
    token: jlong,
    text: JObject,
) -> jboolean {
    let Some(runtime) = RUNTIME.get() else {
        return JNI_FALSE;
    };
    if token <= 0 {
        return JNI_FALSE;
    }
    let Some(read) = runtime.capture_admission().acquire(
        token as u64,
        copypaste_runtime::capture_admission::CaptureScope::Read,
    ) else {
        return JNI_FALSE;
    };
    let Ok(text) = env.get_string((&text).into()) else {
        return JNI_FALSE;
    };
    let text = text.to_string_lossy().into_owned();
    drop(read);
    if SMS_NETWORK_RUNTIME.get().is_none() {
        if let Ok(network) = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            let _ = SMS_NETWORK_RUNTIME.set(network);
        }
    }
    if let Some(network) = SMS_NETWORK_RUNTIME.get() {
        // Local clipboard capture remains available while networking is down.
        let _ = network.block_on(runtime.start_listener());
    }
    if runtime
        .capture_sms_operation(token as u64, &text)
        .unwrap_or(false)
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

struct AndroidClipboard;

impl copypaste_runtime::ClipboardWriter for AndroidClipboard {
    fn write(
        &self,
        payload: &copypaste_core::ClipboardPayload,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        let vm = JAVA_VM
            .get()
            .ok_or(copypaste_core::ClipboardWriteError::Failed)?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|_| copypaste_core::ClipboardWriteError::Failed)?;
        let ok = match payload {
            copypaste_core::ClipboardPayload::Text(text) => call_text(&mut env, text),
            copypaste_core::ClipboardPayload::Image {
                content_type,
                bytes,
            } => call_binary(&mut env, bytes, "copypaste-image", content_type),
            copypaste_core::ClipboardPayload::File { bytes, metadata } => {
                if let Some(source_reference) = metadata
                    .as_ref()
                    .and_then(|file| file.source_reference.as_deref())
                {
                    call_text(&mut env, source_reference)
                } else {
                    call_binary(
                        &mut env,
                        bytes,
                        metadata
                            .as_ref()
                            .map_or("copypaste-file", |file| file.filename.as_str()),
                        metadata
                            .as_ref()
                            .map_or("application/octet-stream", |file| file.mime_type.as_str()),
                    )
                }
            }
            copypaste_core::ClipboardPayload::Unsupported { .. } => {
                return Err(copypaste_core::ClipboardWriteError::UnsupportedContent);
            }
        };
        ok.then_some(())
            .ok_or(copypaste_core::ClipboardWriteError::Failed)
    }
}

fn call_text(env: &mut JNIEnv, text: &str) -> bool {
    let Ok(text) = env.new_string(text) else {
        return false;
    };
    env.call_static_method(
        "com/copypaste/app/MainActivity",
        "writeClipboardText",
        "(Ljava/lang/String;)Z",
        &[JValue::Object(&text.into())],
    )
    .ok()
    .and_then(|value| value.z().ok())
    .unwrap_or(false)
}

fn call_binary(env: &mut JNIEnv, bytes: &[u8], filename: &str, mime_type: &str) -> bool {
    let (Ok(bytes), Ok(filename), Ok(mime_type)) = (
        env.byte_array_from_slice(bytes),
        env.new_string(filename),
        env.new_string(mime_type),
    ) else {
        return false;
    };
    env.call_static_method(
        "com/copypaste/app/MainActivity",
        "writeClipboardBinary",
        "([BLjava/lang/String;Ljava/lang/String;)Z",
        &[
            JValue::Object(&JObject::from(bytes)),
            JValue::Object(&filename.into()),
            JValue::Object(&mime_type.into()),
        ],
    )
    .ok()
    .and_then(|value| value.z().ok())
    .unwrap_or(false)
}

pub(crate) async fn request(
    method: copypaste_ipc::Method,
) -> Result<copypaste_ipc::Response, crate::api::RuntimeError> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(crate::api::RuntimeError::not_initialized)?;
    runtime
        .start_listener()
        .await
        .map_err(|_| crate::api::RuntimeError::internal())?;
    Ok(runtime.request(0, method).await)
}

pub(crate) async fn watch(
    watch_id: u64,
    sink: crate::frb_generated::StreamSink<crate::api::RuntimeEvent>,
) -> Result<(), crate::api::RuntimeError> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(crate::api::RuntimeError::not_initialized)?;
    runtime
        .start_listener()
        .await
        .map_err(|_| crate::api::RuntimeError::internal())?;
    let mut events = runtime.subscribe_events();
    let mut cancelled = crate::runtime::watch_stop_rx(watch_id)?;
    loop {
        let event = tokio::select! {
            changed = cancelled.changed() => {
                changed.map_err(|_| crate::api::RuntimeError::internal())?;
                return Ok(());
            }
            event = events.recv() => match event {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
        };
        if sink
            .add(crate::api::RuntimeEvent {
                kind: match event.event {
                    copypaste_ipc::EventKind::Items => "items".into(),
                    copypaste_ipc::EventKind::Peers => "peers".into(),
                },
                item_count: event.item_count,
                captured: event.captured,
                captured_item_id: event.captured_item_id,
            })
            .is_err()
        {
            return Ok(());
        }
    }
}
