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
    objects::{GlobalRef, JByteArray, JClass, JLongArray, JObject, JObjectArray, JString, JValue},
    sys::{jboolean, jint, jlong, JNI_FALSE, JNI_TRUE},
    JNIEnv, JavaVM,
};

const SMS_HANDLER_UNAVAILABLE: jint = -1;
const SMS_HANDLER_DISABLED: jint = 0;
const SMS_HANDLER_ENABLED: jint = 1;
const SMS_INGEST_RETRY: jint = -1;
const SMS_INGEST_IGNORED: jint = 0;
const SMS_INGEST_CAPTURED: jint = 1;

/// Keeps the application context alive for as long as Rust can access Android
/// Keystore or other context-backed services.
static APPLICATION_CONTEXT: OnceLock<GlobalRef> = OnceLock::new();
pub(crate) static JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();
// Retain the app class while Java supplies its loader. Native sync workers
// cannot resolve application classes through FindClass on attached threads.
static CLIPBOARD_HOST_CLASS: OnceLock<GlobalRef> = OnceLock::new();
pub(crate) static INFERENCE_HOST_CLASS: OnceLock<GlobalRef> = OnceLock::new();
/// The home-screen widget provider, told when what it shows changed.
static WIDGET_PROVIDER_CLASS: OnceLock<GlobalRef> = OnceLock::new();
static RUNTIME: OnceLock<Arc<copypaste_runtime::Runtime>> = OnceLock::new();

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
    class: JClass,
    context: JObject,
) {
    if INFERENCE_HOST_CLASS.get().is_none() {
        let class = env
            .find_class("com/copypaste/app/InferenceWorkerHost")
            .and_then(|class| env.new_global_ref(class));
        match class {
            Ok(class) => {
                let _ = INFERENCE_HOST_CLASS.set(class);
            }
            Err(_) => {
                let _ = env.exception_clear();
                return;
            }
        }
    }
    if WIDGET_PROVIDER_CLASS.get().is_none() {
        match env
            .find_class("com/copypaste/app/RecentClipsWidgetProvider")
            .and_then(|class| env.new_global_ref(class))
        {
            Ok(class) => {
                let _ = WIDGET_PROVIDER_CLASS.set(class);
            }
            Err(_) => {
                let _ = env.exception_clear();
            }
        }
    }
    if CLIPBOARD_HOST_CLASS.get().is_none() {
        let Ok(class) = env.new_global_ref(class) else {
            return;
        };
        let _ = CLIPBOARD_HOST_CLASS.set(class);
    }
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
    let _ = JAVA_VM.set(Arc::new(java_vm));
    copypaste_core::file_transfer::set_folder_writer(Arc::new(DocumentFolder));
    copypaste_core::file_reference::set_uri_resolver(Arc::new(|reference| {
        use std::os::fd::FromRawFd;
        let open = || -> Result<std::fs::File, jni::errors::Error> {
            let vm = JAVA_VM
                .get()
                .ok_or(jni::errors::Error::NullPtr("Java VM"))?;
            let mut env = vm.attach_current_thread()?;
            let class = CLIPBOARD_HOST_CLASS
                .get()
                .ok_or(jni::errors::Error::NullPtr("source host"))?;
            let value = env.new_string(reference)?;
            let fd = env
                .call_static_method(
                    class,
                    "openFileSource",
                    "(Ljava/lang/String;)I",
                    &[(&value).into()],
                )?
                .i()?;
            if fd < 0 {
                return Err(jni::errors::Error::NullPtr("file source"));
            }
            // Kotlin detached this descriptor; ownership transfers exactly once.
            Ok(unsafe { std::fs::File::from_raw_fd(fd) })
        };
        open().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "The source file could not be opened.",
            )
        })
    }));

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
    crate::runtime_log::init(&data_dir);
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
            let (Some(vm), Some(host)) = (JAVA_VM.get(), INFERENCE_HOST_CLASS.get()) else {
                return;
            };
            // Modules are optional: their failures must not keep history,
            // capture and sync from starting.
            if runtime
                .set_inference_launcher(Arc::new(copypaste_modules::AndroidInferenceLauncher::new(
                    Arc::clone(vm),
                    host.clone(),
                )))
                .is_err()
            {
                tracing::error!("could not initialize the Android inference launcher");
            }
            let runtime = Arc::new(runtime);
            if runtime.attach_module_services().is_err() {
                tracing::error!("could not attach the module host services");
            }
            let _ = RUNTIME.set(runtime);
        }
        Err(error) => tracing::error!(%error, "could not open Android runtime"),
    }
}

/// Put the newest clip back on an empty clipboard once the app has focus.
pub(crate) fn restore_clipboard_on_start() -> bool {
    RUNTIME
        .get()
        .is_some_and(|runtime| runtime.restore_clipboard_on_start())
}

/// Returns the runtime's cached history pages to the allocator when Android
/// reports memory pressure. A runtime that is not open yet holds none.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_MainActivity_releaseRuntimeMemory(
    _env: JNIEnv,
    _class: JClass,
) {
    if let Some(Err(error)) = RUNTIME.get().map(|runtime| runtime.store.release_memory()) {
        tracing::warn!(%error, "could not release cached history pages");
    }
    purge_allocator();
}

/// Asks bionic to return free heap pages to the kernel. `mallopt` exists only
/// from API 26, below the minimum SDK, so it is resolved at run time; older
/// releases ignore the purge options and report failure.
fn purge_allocator() {
    // bionic <malloc.h>: M_PURGE (API 28) and M_PURGE_ALL (API 34).
    const M_PURGE: libc::c_int = -101;
    const M_PURGE_ALL: libc::c_int = -104;
    type Mallopt = unsafe extern "C" fn(libc::c_int, libc::c_int) -> libc::c_int;
    // SAFETY: the name is NUL-terminated and RTLD_DEFAULT searches the loaded
    // libraries; a non-null result is bionic's `mallopt`, which takes two ints.
    unsafe {
        let symbol = libc::dlsym(libc::RTLD_DEFAULT, c"mallopt".as_ptr());
        if symbol.is_null() {
            return;
        }
        let mallopt = std::mem::transmute::<*mut libc::c_void, Mallopt>(symbol);
        if mallopt(M_PURGE_ALL, 0) == 0 {
            mallopt(M_PURGE, 0);
        }
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
    env: JNIEnv,
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
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_classify(
    _env: JNIEnv,
    _class: JClass,
    token: jlong,
    secret: jboolean,
    transient: jboolean,
) -> jboolean {
    if token > 0
        && RUNTIME.get().is_some_and(|runtime| {
            runtime.capture_admission().classify(
                token as u64,
                copypaste_ipc::ClipboardPrivacy {
                    secret: secret != JNI_FALSE,
                    transient: transient != JNI_FALSE,
                },
            )
        })
    {
        JNI_TRUE
    } else {
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
) -> jint {
    // Mirrors NativeRuntimeCapture.INGEST_* in Kotlin.
    const REFUSED: jint = 0;
    const SAVED: jint = 1;
    const SAVED_SECRET: jint = 2;
    if token <= 0 {
        return REFUSED;
    }
    let Some(runtime) = RUNTIME.get() else {
        return REFUSED;
    };
    let Some(read) = runtime.capture_admission().acquire(
        token as u64,
        copypaste_runtime::capture_admission::CaptureScope::Read,
    ) else {
        return REFUSED;
    };
    let Ok(text) = env.get_string((&text).into()) else {
        return REFUSED;
    };
    let text = text.to_string_lossy().into_owned();
    drop(read);
    match runtime.capture_text_operation(token as u64, &text) {
        Ok(privacy) if privacy.secret => SAVED_SECRET,
        Ok(_) => SAVED,
        Err(_) => REFUSED,
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_ingestFile(
    mut env: JNIEnv,
    _class: JClass,
    token: jlong,
    size_bytes: jlong,
    filename: JString,
    mime_type: JString,
    source_reference: JString,
) -> jboolean {
    let Some(runtime) = RUNTIME.get() else {
        return JNI_FALSE;
    };
    let Some(read) = runtime.capture_admission().acquire(
        token as u64,
        copypaste_runtime::capture_admission::CaptureScope::Read,
    ) else {
        return JNI_FALSE;
    };
    let values = (|| {
        Some((
            env.get_string(&filename)
                .ok()?
                .to_string_lossy()
                .into_owned(),
            env.get_string(&mime_type)
                .ok()?
                .to_string_lossy()
                .into_owned(),
            env.get_string(&source_reference)
                .ok()?
                .to_string_lossy()
                .into_owned(),
        ))
    })();
    drop(read);
    let Some((filename, mime_type, reference)) = values else {
        return JNI_FALSE;
    };
    if size_bytes >= 0
        && runtime
            .capture_file_operation(
                token as u64,
                size_bytes as u64,
                &filename,
                &mime_type,
                &reference,
                None,
            )
            .is_ok()
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Files copied or shared together; arrays are parallel and in clipboard order.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_ingestFiles(
    mut env: JNIEnv,
    _class: JClass,
    token: jlong,
    sizes: JLongArray,
    filenames: JObjectArray,
    mime_types: JObjectArray,
    source_references: JObjectArray,
) -> jboolean {
    let Some(runtime) = RUNTIME.get() else {
        return JNI_FALSE;
    };
    let Some(read) = runtime.capture_admission().acquire(
        token as u64,
        copypaste_runtime::capture_admission::CaptureScope::Read,
    ) else {
        return JNI_FALSE;
    };
    let values = (|| {
        let count = env.get_array_length(&sizes).ok()?;
        let mut lengths = vec![0; usize::try_from(count).ok()?];
        env.get_long_array_region(&sizes, 0, &mut lengths).ok()?;
        let mut strings = |array: &JObjectArray| {
            if env.get_array_length(array).ok()? != count {
                return None;
            }
            (0..count)
                .map(|index| {
                    let value = JString::from(env.get_object_array_element(array, index).ok()?);
                    let text = env.get_string(&value).ok()?.to_string_lossy().into_owned();
                    let _ = env.delete_local_ref(value);
                    Some(text)
                })
                .collect::<Option<Vec<_>>>()
        };
        let names = strings(&filenames)?;
        let mimes = strings(&mime_types)?;
        let references = strings(&source_references)?;
        let sizes = lengths
            .into_iter()
            .map(|size| u64::try_from(size).ok())
            .collect::<Option<Vec<_>>>()?;
        Some((sizes, names, mimes, references))
    })();
    drop(read);
    let Some((sizes, names, mimes, references)) = values else {
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        return JNI_FALSE;
    };
    let files = sizes
        .iter()
        .zip(&names)
        .zip(&mimes)
        .zip(&references)
        .map(
            |(((size_bytes, filename), mime_type), reference)| copypaste_runtime::CaptureFile {
                filename,
                mime_type,
                reference,
                size_bytes: *size_bytes,
            },
        )
        .collect::<Vec<_>>();
    if runtime
        .capture_file_set_operation(token as u64, &files, None)
        .is_ok()
    {
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
    length: jint,
    content_type: JObject,
    filename: JObject,
    source_reference: JObject,
    source_package_name: JObject,
    source_name: JObject,
    source_icon: JByteArray,
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
    // Java's owned PNG buffer can have spare capacity; only its written prefix is data.
    let Ok(array_length) = env.get_array_length(&bytes) else {
        return JNI_FALSE;
    };
    if length <= 0 || length > array_length || length > 16 * 1024 * 1024 {
        return JNI_FALSE;
    }
    let mut payload = vec![0u8; length as usize];
    // JNI represents byte arrays as signed bytes; the representation is identical.
    let signed =
        unsafe { std::slice::from_raw_parts_mut(payload.as_mut_ptr().cast::<i8>(), payload.len()) };
    if env.get_byte_array_region(&bytes, 0, signed).is_err() {
        return JNI_FALSE;
    }
    let (Ok(content_type), Ok(filename), Ok(source_reference)) = (
        env.get_string((&content_type).into()),
        env.get_string((&filename).into()),
        env.get_string((&source_reference).into()),
    ) else {
        return JNI_FALSE;
    };
    let content_type = content_type.to_string_lossy().into_owned();
    let filename = filename.to_string_lossy().into_owned();
    let source_reference = source_reference.to_string_lossy().into_owned();
    // Optional metadata cannot prevent the image from being saved.
    let source_app = (|| {
        let package_name = env.get_string((&source_package_name).into()).ok()?;
        let package_name = package_name.to_string_lossy().into_owned();
        if package_name.is_empty()
            || package_name.len() > 255
            || !package_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._".contains(&byte))
        {
            return None;
        }
        let name = env
            .get_string((&source_name).into())
            .ok()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty() && name.len() <= 1024 && !name.contains('\0'));
        let icon = env
            .get_array_length(&source_icon)
            .ok()
            .filter(|length| {
                *length > 0 && *length as usize <= copypaste_ipc::MAX_SOURCE_APP_ICON_BYTES
            })
            .and_then(|_| env.convert_byte_array(&source_icon).ok())
            .and_then(|png| copypaste_core::SourceAppIconMetadata::new(&png, 64, 64));
        Some(copypaste_runtime::CaptureSourceApp {
            bundle_id: package_name,
            name,
            icon,
        })
    })();
    drop(read);
    if runtime
        .capture_binary_operation(
            token as u64,
            &payload,
            &content_type,
            (!filename.is_empty()).then_some(filename.as_str()),
            (!source_reference.is_empty()).then_some(source_reference.as_str()),
            source_app.as_ref(),
        )
        .is_ok()
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

// This snapshot distinguishes disabled policy from temporarily busy admission.
// All provider reads and commits still require runtime-owned capture scopes.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_implicitCaptureAllowed(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if RUNTIME
        .get()
        .is_some_and(|runtime| runtime.implicit_capture_allowed())
    {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Fixed Android lifecycle events for the runtime log. Kotlin passes a code,
/// never text, so only these literal messages can be recorded; the codes match
/// `RuntimeLogEvent` in `NativeRuntimeCapture.kt`.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_logEvent(
    _env: JNIEnv,
    _class: JClass,
    event: jint,
) {
    match event {
        1 => tracing::info!("background capture service started"),
        2 => tracing::info!("background capture service stopped"),
        3 => tracing::warn!("background capture could not start; access is missing"),
        4 => tracing::warn!("background capture stopped unexpectedly"),
        5 => tracing::info!("clipboard polling started"),
        6 => tracing::info!("clipboard polling stopped"),
        7 => tracing::debug!("clipboard polling paused while the screen is off"),
        8 => tracing::debug!("clipboard polling resumed"),
        9 => tracing::info!("background sync service started"),
        10 => tracing::info!("background sync service stopped"),
        11 => tracing::warn!("background sync service was not allowed to start"),
        12 => tracing::info!("background capture method changed"),
        _ => tracing::debug!("unknown android lifecycle event"),
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
pub extern "system" fn Java_com_copypaste_app_NativeSmsModules_handlerState(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    match RUNTIME.get().map(|runtime| runtime.has_sms_module()) {
        Some(Ok(true)) => SMS_HANDLER_ENABLED,
        Some(Ok(false)) => SMS_HANDLER_DISABLED,
        Some(Err(_)) | None => SMS_HANDLER_UNAVAILABLE,
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeSmsModules_capturePaused(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if RUNTIME
        .get()
        .is_some_and(|runtime| runtime.sms_capture_paused())
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
) -> jint {
    let Some(runtime) = RUNTIME.get() else {
        return SMS_INGEST_RETRY;
    };
    if token <= 0 {
        return SMS_INGEST_RETRY;
    }
    let Some(read) = runtime.capture_admission().acquire(
        token as u64,
        copypaste_runtime::capture_admission::CaptureScope::Read,
    ) else {
        return if runtime.sms_capture_paused() {
            SMS_INGEST_IGNORED
        } else {
            SMS_INGEST_RETRY
        };
    };
    let Ok(text) = env.get_string((&text).into()) else {
        return SMS_INGEST_RETRY;
    };
    let text = text.to_string_lossy().into_owned();
    drop(read);
    // Local clipboard capture remains available while networking is down.
    let _ = crate::api::handler::async_runtime().block_on(runtime.start_listener());
    match runtime.capture_sms_operation(token as u64, &text) {
        copypaste_runtime::SmsCaptureOutcome::Captured => SMS_INGEST_CAPTURED,
        copypaste_runtime::SmsCaptureOutcome::Ignored => SMS_INGEST_IGNORED,
        copypaste_runtime::SmsCaptureOutcome::Retry => SMS_INGEST_RETRY,
    }
}

/// Saves into a user-chosen folder (a Storage Access Framework tree).
struct DocumentFolder;

impl DocumentFolder {
    fn call<T>(
        work: impl FnOnce(&mut JNIEnv, &GlobalRef) -> Result<T, jni::errors::Error>,
    ) -> Result<T, jni::errors::Error> {
        let vm = JAVA_VM
            .get()
            .ok_or(jni::errors::Error::NullPtr("Java VM"))?;
        let mut env = vm.attach_current_thread()?;
        let class = CLIPBOARD_HOST_CLASS
            .get()
            .ok_or(jni::errors::Error::NullPtr("folder host"))?;
        work(&mut env, class)
    }
}

impl copypaste_core::file_transfer::FolderWriter for DocumentFolder {
    fn create(
        &self,
        folder: &str,
        name: &str,
        mime: &str,
    ) -> std::io::Result<(std::fs::File, String)> {
        use std::os::fd::FromRawFd;
        let created = Self::call(|env, class| {
            let (folder, name, mime) = (
                env.new_string(folder)?,
                env.new_string(name)?,
                env.new_string(mime)?,
            );
            let document = env
                .call_static_method(
                    class,
                    "createFolderFile",
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[(&folder).into(), (&name).into(), (&mime).into()],
                )?
                .l()?;
            if document.is_null() {
                return Err(jni::errors::Error::NullPtr("folder document"));
            }
            let document: String = env.get_string(&JString::from(document))?.into();
            let target = env.new_string(&document)?;
            let fd = env
                .call_static_method(
                    class,
                    "openFileSink",
                    "(Ljava/lang/String;)I",
                    &[(&target).into()],
                )?
                .i()?;
            Ok((document, fd))
        });
        match created {
            Ok((document, fd)) if fd >= 0 => {
                // Kotlin detached this descriptor; ownership transfers exactly once.
                Ok((unsafe { std::fs::File::from_raw_fd(fd) }, document))
            }
            Ok((document, _)) => {
                self.remove(&document);
                Err(std::io::ErrorKind::PermissionDenied.into())
            }
            Err(_) => Err(std::io::ErrorKind::NotFound.into()),
        }
    }

    fn remove(&self, location: &str) {
        let _ = Self::call(|env, class| {
            let target = env.new_string(location)?;
            env.call_static_method(
                class,
                "deleteFolderFile",
                "(Ljava/lang/String;)V",
                &[(&target).into()],
            )?;
            Ok(())
        });
    }
}

struct AndroidClipboard;

impl copypaste_runtime::ClipboardWriter for AndroidClipboard {
    fn write(
        &self,
        payload: &copypaste_core::ClipboardPayload,
        content_type: &str,
        options: copypaste_runtime::WriteOptions,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        self.write_native(payload, content_type, false, options)
    }

    fn write_received(
        &self,
        payload: &copypaste_core::ClipboardPayload,
        content_type: &str,
        secret: bool,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        let options = copypaste_runtime::WriteOptions::copy(secret);
        self.write_native(payload, content_type, true, options)
    }

    fn clipboard_is_empty(&self) -> Option<bool> {
        let (vm, class) = (JAVA_VM.get()?, CLIPBOARD_HOST_CLASS.get()?);
        let mut env = vm.attach_current_thread().ok()?;
        let answer = env
            .call_static_method(class, "clipboardIsEmpty", "()Z", &[])
            .and_then(|value| value.z());
        if answer.is_err() {
            let _ = env.exception_clear();
        }
        answer.ok()
    }

    fn history_cleared(&self) {
        notify_widgets(true);
    }

    fn device_removed(&self, _device_id: &str) {
        notify_widgets(true);
    }

    fn surfaces_changed(&self) {
        notify_widgets(false);
    }
}

/// Tells the home-screen widget that its content changed. Kotlin coalesces
/// ordinary changes; `cleared` (history cleared, device removed) empties every
/// instance at once. Carries no content.
fn notify_widgets(cleared: bool) {
    let (Some(vm), Some(class), Some(context)) = (
        JAVA_VM.get(),
        WIDGET_PROVIDER_CLASS.get(),
        APPLICATION_CONTEXT.get(),
    ) else {
        return;
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return;
    };
    if env
        .call_static_method(
            class,
            "notifyChanged",
            "(Landroid/content/Context;Z)V",
            &[JValue::Object(context.as_obj()), jni_bool(cleared)],
        )
        .is_err()
    {
        let _ = env.exception_clear();
    }
}

impl AndroidClipboard {
    fn write_native(
        &self,
        payload: &copypaste_core::ClipboardPayload,
        content_type: &str,
        received: bool,
        options: copypaste_runtime::WriteOptions,
    ) -> Result<(), copypaste_core::ClipboardWriteError> {
        let vm = JAVA_VM
            .get()
            .ok_or(copypaste_core::ClipboardWriteError::Failed)?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|_| copypaste_core::ClipboardWriteError::Failed)?;
        let ok = match payload {
            copypaste_core::ClipboardPayload::Text(text) => {
                call_text(&mut env, text, content_type, received, options)
            }
            copypaste_core::ClipboardPayload::Image {
                content_type,
                bytes,
            } => call_binary(
                &mut env,
                bytes,
                match content_type.as_str() {
                    "image/png" => "copypaste-image.png",
                    "image/tiff" => "copypaste-image.tiff",
                    _ => "copypaste-image",
                },
                content_type,
                received,
                options,
            ),
            copypaste_core::ClipboardPayload::File { bytes, metadata } => {
                if let Some(file) = metadata
                    .as_ref()
                    .filter(|file| file.source_reference.is_some())
                {
                    let path = file
                        .source_reference
                        .as_deref()
                        .ok_or(copypaste_core::ClipboardWriteError::Failed)?;
                    let path = env
                        .new_string(path)
                        .map_err(|_| copypaste_core::ClipboardWriteError::Failed)?;
                    let name = env
                        .new_string(&file.filename)
                        .map_err(|_| copypaste_core::ClipboardWriteError::Failed)?;
                    let mime = env
                        .new_string(&file.mime_type)
                        .map_err(|_| copypaste_core::ClipboardWriteError::Failed)?;
                    let class = CLIPBOARD_HOST_CLASS
                        .get()
                        .ok_or(copypaste_core::ClipboardWriteError::Failed)?;
                    let call = if file.more_references.is_empty() {
                        env.call_static_method(
                            class,
                            "writeClipboardFile",
                            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZZ)Z",
                            &[
                                (&path).into(),
                                (&name).into(),
                                (&mime).into(),
                                jni_bool(options.secret),
                                jni_bool(options.transient),
                            ],
                        )
                    } else {
                        let paths = string_array(&mut env, &path, &file.more_references)
                            .ok_or(copypaste_core::ClipboardWriteError::Failed)?;
                        env.call_static_method(
                            class,
                            "writeClipboardFiles",
                            "([Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZZ)Z",
                            &[
                                (&paths).into(),
                                (&name).into(),
                                (&mime).into(),
                                jni_bool(options.secret),
                                jni_bool(options.transient),
                            ],
                        )
                    };
                    return call
                        .ok()
                        .and_then(|value| value.z().ok())
                        .filter(|value| *value)
                        .map(|_| ())
                        .ok_or(copypaste_core::ClipboardWriteError::Failed);
                }
                call_binary(
                    &mut env,
                    bytes,
                    metadata
                        .as_ref()
                        .map_or("copypaste-file", |file| file.filename.as_str()),
                    metadata
                        .as_ref()
                        .map_or("application/octet-stream", |file| file.mime_type.as_str()),
                    received,
                    options,
                )
            }
            copypaste_core::ClipboardPayload::Unsupported { .. } => {
                return Err(copypaste_core::ClipboardWriteError::UnsupportedContent);
            }
        };
        ok.then_some(())
            .ok_or(copypaste_core::ClipboardWriteError::Failed)
    }
}

fn string_array<'local>(
    env: &mut JNIEnv<'local>,
    first: &JString,
    more: &[String],
) -> Option<JObjectArray<'local>> {
    let length = i32::try_from(more.len() + 1).ok()?;
    let array = env
        .new_object_array(length, "java/lang/String", JObject::null())
        .ok()?;
    env.set_object_array_element(&array, 0, first).ok()?;
    for (index, value) in (1..).zip(more) {
        let value = env.new_string(value).ok()?;
        env.set_object_array_element(&array, index, &value).ok()?;
        env.delete_local_ref(value).ok()?;
    }
    Some(array)
}

fn jni_bool(value: bool) -> JValue<'static, 'static> {
    JValue::Bool(if value { JNI_TRUE } else { JNI_FALSE })
}

fn call_text(
    env: &mut JNIEnv,
    text: &str,
    content_type: &str,
    received: bool,
    options: copypaste_runtime::WriteOptions,
) -> bool {
    let (Ok(text), Ok(content_type)) = (env.new_string(text), env.new_string(content_type)) else {
        return false;
    };
    let Some(class) = CLIPBOARD_HOST_CLASS.get() else {
        return false;
    };
    env.call_static_method(
        class,
        "writeClipboardText",
        "(Ljava/lang/String;Ljava/lang/String;ZZZ)Z",
        &[
            JValue::Object(&text.into()),
            JValue::Object(&content_type.into()),
            jni_bool(received),
            jni_bool(options.secret),
            jni_bool(options.transient),
        ],
    )
    .ok()
    .and_then(|value| value.z().ok())
    .unwrap_or(false)
}

fn call_binary(
    env: &mut JNIEnv,
    bytes: &[u8],
    filename: &str,
    mime_type: &str,
    received: bool,
    options: copypaste_runtime::WriteOptions,
) -> bool {
    let (Ok(bytes), Ok(filename), Ok(mime_type)) = (
        env.byte_array_from_slice(bytes),
        env.new_string(filename),
        env.new_string(mime_type),
    ) else {
        return false;
    };
    let Some(class) = CLIPBOARD_HOST_CLASS.get() else {
        return false;
    };
    env.call_static_method(
        class,
        "writeClipboardBinary",
        "([BLjava/lang/String;Ljava/lang/String;ZZZ)Z",
        &[
            JValue::Object(&JObject::from(bytes)),
            JValue::Object(&filename.into()),
            JValue::Object(&mime_type.into()),
            jni_bool(received),
            jni_bool(options.secret),
            jni_bool(options.transient),
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
    runtime.start_module_sync();
    // Store work stays off the few async workers, as `dispatch` asks hosts.
    let admission = match runtime.route(&method).await {
        copypaste_runtime::Route::Reactor => return Ok(runtime.request(0, method).await),
        copypaste_runtime::Route::Blocking(admission) => admission,
    };
    let runtime = Arc::clone(runtime);
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        runtime.request_blocking(0, method)
    })
    .await
    .map_err(|_| crate::api::RuntimeError::internal())
}

/// [`copypaste_ipc::Method::ImagePreview`] with the PNG kept as bytes, since no
/// IPC text form is needed in-process.
pub(crate) async fn image_preview(
    item_id: String,
    max_edge: Option<u32>,
    bounds: Option<copypaste_ipc::ImagePreviewBounds>,
) -> Result<crate::api::ClipImagePreview, crate::api::RuntimeError> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(crate::api::RuntimeError::not_initialized)?;
    runtime
        .start_listener()
        .await
        .map_err(|_| crate::api::RuntimeError::internal())?;
    runtime.start_module_sync();
    match runtime.image_preview_png(item_id, max_edge, bounds).await {
        Ok(thumbnail) => Ok(crate::api::ClipImagePreview {
            png_bytes: thumbnail.png,
            width: thumbnail.width,
            height: thumbnail.height,
        }),
        Err(refusal) => Err(crate::client::response_to_result(*refusal)
            .err()
            .unwrap_or_else(crate::api::RuntimeError::internal)),
    }
}

/// Serves one IPC method for the native surfaces that run without a Flutter
/// engine (keyboard panel, tiles, widget). Takes `{"method", "params"}` JSON and
/// returns the response JSON with id 0. It never starts the P2P listener or
/// module sync: reactor methods (network I/O) are refused as `unsupported`.
/// Kotlin calls it from one worker thread, never the main thread.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeAccess_request<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    method: JString<'local>,
) -> jni::sys::jstring {
    let response = match env.get_string(&method) {
        Ok(method) => native_request(&method.to_string_lossy()),
        Err(_) => {
            let _ = env.exception_clear();
            invalid_native_request()
        }
    };
    let encoded = serde_json::to_string(&response).unwrap_or_else(|_| {
        serde_json::to_string(&copypaste_ipc::Response::err(
            0,
            copypaste_ipc::ErrorCode::Internal,
            "The reply could not be encoded.",
        ))
        .unwrap_or_default()
    });
    match env.new_string(encoded) {
        Ok(encoded) => encoded.into_raw(),
        Err(_) => {
            let _ = env.exception_clear();
            std::ptr::null_mut()
        }
    }
}

fn native_request(json: &str) -> copypaste_ipc::Response {
    use copypaste_runtime::Route;
    let Some(runtime) = RUNTIME.get() else {
        return copypaste_ipc::Response::err(
            0,
            copypaste_ipc::ErrorCode::NotReady,
            "CopyPaste is still starting.",
        );
    };
    let Ok(method) = serde_json::from_str::<copypaste_ipc::Method>(json) else {
        return invalid_native_request();
    };
    let admission = match block_on(runtime.route(&method)) {
        Route::Blocking(admission) => admission,
        Route::Reactor => {
            return copypaste_ipc::Response::err(
                0,
                copypaste_ipc::ErrorCode::Unsupported,
                "This action needs the CopyPaste app.",
            )
        }
    };
    let response = runtime.request_blocking(0, method);
    drop(admission);
    response
}

fn invalid_native_request() -> copypaste_ipc::Response {
    copypaste_ipc::Response::err(
        0,
        copypaste_ipc::ErrorCode::InvalidRequest,
        "The request is not a valid method.",
    )
}

/// Drives [`copypaste_runtime::Runtime::route`] on the calling JNI thread.
/// Its only await is a runtime-agnostic semaphore, so no reactor is needed.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}

pub(crate) async fn stream(
    method: copypaste_ipc::Method,
    mut on_frame: impl FnMut(copypaste_ipc::ResponseData) -> Result<(), crate::api::RuntimeError>,
) -> Result<copypaste_ipc::Response, crate::api::RuntimeError> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(crate::api::RuntimeError::not_initialized)?;
    runtime
        .start_listener()
        .await
        .map_err(|_| crate::api::RuntimeError::internal())?;
    let (frames, mut queued) = tokio::sync::mpsc::channel(1);
    let worker = Arc::clone(runtime);
    let work = tokio::task::spawn_blocking(move || {
        worker.request_stream(0, method, &mut |frame| frames.blocking_send(frame).is_ok())
    });
    let mut failed = Ok(());
    while let Some(frame) = queued.recv().await {
        if let Some(data) = frame.data {
            failed = on_frame(data);
            if failed.is_err() {
                break;
            }
        }
    }
    // Closing the queue abandons the operation; the join waits for its cleanup.
    drop(queued);
    let response = work
        .await
        .map_err(|_| crate::api::RuntimeError::internal())?;
    failed.map(|()| response)
}

async fn push_capture_snapshot(
    runtime: &Arc<copypaste_runtime::Runtime>,
    sink: &crate::frb_generated::StreamSink<crate::api::RuntimeEvent>,
) -> Result<bool, crate::api::RuntimeError> {
    let runtime = Arc::clone(runtime);
    let status = tokio::task::spawn_blocking(move || runtime.status_data())
        .await
        .map_err(|_| crate::api::RuntimeError::internal())?;
    Ok(sink
        .add(crate::client::typed_runtime_event(
            copypaste_ipc::RuntimeEventData {
                file_operation: None,
                change: None,
                capture: Some(status),
                progress: None,
            },
        ))
        .is_ok())
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
    let mut runtime_events = runtime.subscribe_runtime_events();
    let mut transfers = runtime.store.subscribe_file_operations();
    if sink.add(crate::client::runtime_ready()).is_err() {
        return Ok(());
    }
    if !push_capture_snapshot(runtime, &sink).await? {
        return Ok(());
    }
    let mut cancelled = crate::runtime::watch_stop_rx(watch_id)?;
    loop {
        let event = tokio::select! {
            changed = cancelled.changed() => {
                changed.map_err(|_| crate::api::RuntimeError::internal())?;
                return Ok(());
            }
            event = events.recv() => match event {
                Ok(event) => crate::client::typed_runtime_event(copypaste_ipc::RuntimeEventData { file_operation: None, change: Some(event), capture: None, progress: None }),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if sink.add(crate::client::runtime_ready()).is_err() { return Ok(()); }
                    if !push_capture_snapshot(runtime, &sink).await? { return Ok(()); }
                    continue;
                },
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            changed = transfers.recv() => crate::client::typed_runtime_event(copypaste_ipc::RuntimeEventData { file_operation: Some(changed.unwrap_or_default()), change: None, capture: None, progress: None }),
            event = runtime_events.recv() => match event {
                Ok(event) => crate::client::typed_runtime_event(event),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if sink.add(crate::client::runtime_ready()).is_err() { return Ok(()); }
                    if !push_capture_snapshot(runtime, &sink).await? { return Ok(()); }
                    continue;
                },
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
        };
        if sink.add(event).is_err() {
            return Ok(());
        }
    }
}

/// Only discard Android grants after the durable store has released the source.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_NativeRuntimeCapture_fileSourceRetained(
    mut env: JNIEnv,
    _class: JClass,
    reference: JString,
) -> jboolean {
    let Some(runtime) = RUNTIME.get() else {
        return JNI_TRUE;
    };
    let Ok(reference) = env.get_string(&reference) else {
        return JNI_TRUE;
    };
    let retained = runtime
        .store
        .retains_file_reference(&reference.to_string_lossy())
        .unwrap_or(true);
    if retained {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}
