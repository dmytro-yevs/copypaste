//! All native loading and pointer ownership are confined to this boundary.
use crate::ModuleError;
use copypaste_module_sdk::{
    native::{NativeApi, NativeBuffer, NativeHostApi, NativeReply, MAX_HOST_BYTES},
    ModuleEnvironment, ModuleInvocation, ModuleOutput, ModuleUnloadPolicy, MAX_INVOCATION_BYTES,
    MODULE_API_VERSION,
};
use libloading::Library;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

/// Each module owns its execution lock. Long commands do not hold the registry
/// lock or serialize commands in other modules. Lifecycle changes close admission
/// first and wait for already running work before releasing code and assets.
pub(crate) struct ModuleInstance {
    accepting: Arc<AtomicBool>,
    module: Mutex<Option<NativeModule>>,
}

impl ModuleInstance {
    pub(crate) fn new() -> Self {
        Self {
            accepting: Arc::new(AtomicBool::new(true)),
            module: Mutex::new(None),
        }
    }

    pub(crate) fn invoke(
        &self,
        verifier: &crate::package::PackageVerifier,
        package_dir: &Path,
        data_dir: &Path,
        invocation: &ModuleInvocation,
        services: Option<Arc<crate::SyncServices>>,
        id: &str,
    ) -> Result<ModuleOutput, ModuleError> {
        let mut module = self.module.lock().map_err(|_| ModuleError::State)?;
        if !self.accepting.load(Ordering::Acquire) {
            return Err(ModuleError::Disabled);
        }
        if module.is_none() {
            let manifest = verifier.installed(package_dir, true)?;
            std::fs::create_dir_all(data_dir)?;
            if let Some(provider) = &manifest.search_provider {
                let languages = invocation
                    .preferences
                    .get(&provider.language_field)
                    .and_then(serde_json::Value::as_array)
                    .ok_or(ModuleError::State)?
                    .iter()
                    .map(|value| value.as_str().map(str::to_owned).ok_or(ModuleError::State))
                    .collect::<Result<Vec<_>, _>>()?;
                let model = provider.model_for(&languages).ok_or(ModuleError::State)?;
                crate::resources::verify(data_dir, model)?;
            }
            let environment = ModuleEnvironment {
                package_dir: package_dir.to_string_lossy().into_owned(),
                data_dir: data_dir.to_string_lossy().into_owned(),
            };
            *module = Some(NativeModule::load(
                &package_dir.join(manifest.entrypoint),
                &environment,
                manifest.unload_policy,
                services.map(|services| {
                    Box::new(HostBridge {
                        id: id.into(),
                        services,
                        accepting: Arc::clone(&self.accepting),
                    })
                }),
            )?);
        }
        module.as_mut().ok_or(ModuleError::Load)?.invoke(invocation)
    }

    pub(crate) fn stop(&self) {
        self.accepting.store(false, Ordering::Release);
        // Poison must not prevent teardown after a committed lifecycle change.
        self.module
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}

pub(crate) struct NativeModule {
    api: NativeApi,
    context: *mut c_void,
    _library: Arc<Library>,
    _host: Option<Box<HostBridge>>,
}

// SAFETY: SDK modules are Send; the per-instance mutex serializes calls and drop.
unsafe impl Send for NativeModule {}

impl NativeModule {
    fn load(
        path: &Path,
        environment: &ModuleEnvironment,
        policy: ModuleUnloadPolicy,
        host: Option<Box<HostBridge>>,
    ) -> Result<Self, ModuleError> {
        // SAFETY: only authenticated first-party code reaches this boundary. Its ABI must
        // implement the SDK ownership contract; native code is trusted, not sandboxed.
        unsafe {
            let library = load_library(path, policy)?;
            let entry: libloading::Symbol<unsafe extern "C" fn() -> *const NativeApi> = library
                .get(b"copypaste_module_v1\0")
                .map_err(|_| ModuleError::Load)?;
            let api = entry();
            if api.is_null() {
                return Err(ModuleError::Load);
            }
            #[repr(C)]
            struct Header {
                api_version: u32,
                struct_size: usize,
            }
            let header = &*api.cast::<Header>();
            if header.api_version != MODULE_API_VERSION
                || header.struct_size != std::mem::size_of::<NativeApi>()
            {
                return Err(ModuleError::Load);
            }
            let api = *api;
            let bytes = serde_json::to_vec(environment).map_err(|_| ModuleError::State)?;
            let context = if let Some(bridge) = host.as_deref() {
                let create: libloading::Symbol<
                    unsafe extern "C" fn(*const u8, usize, *const NativeHostApi) -> *mut c_void,
                > = library
                    .get(b"copypaste_module_with_host_v1\0")
                    .map_err(|_| ModuleError::Load)?;
                let host_api = NativeHostApi {
                    api_version: MODULE_API_VERSION,
                    struct_size: std::mem::size_of::<NativeHostApi>(),
                    context: (bridge as *const HostBridge).cast_mut().cast(),
                    request: host_request,
                    release: host_release,
                };
                create(bytes.as_ptr(), bytes.len(), &host_api)
            } else {
                (api.create)(bytes.as_ptr(), bytes.len())
            };
            if context.is_null() {
                return Err(ModuleError::Load);
            }
            Ok(Self {
                api,
                context,
                _library: library,
                _host: host,
            })
        }
    }

    pub(crate) fn invoke(
        &mut self,
        invocation: &ModuleInvocation,
    ) -> Result<ModuleOutput, ModuleError> {
        let bytes = zeroize::Zeroizing::new(
            serde_json::to_vec(invocation).map_err(|_| ModuleError::State)?,
        );
        if bytes.len() > MAX_INVOCATION_BYTES {
            return Err(ModuleError::Invalid(
                "The module input exceeds its size limit.".into(),
            ));
        }
        // SAFETY: context belongs to this loaded library and calls are serialized. The
        // reply stays borrowed until its owner's release callback runs exactly once.
        unsafe {
            let reply = (self.api.invoke)(self.context, bytes.as_ptr(), bytes.len());
            let result = if reply.buffer.data.is_null() || reply.buffer.len > MAX_INVOCATION_BYTES {
                Err(ModuleError::Invalid(
                    "The module returned an invalid result.".into(),
                ))
            } else {
                let bytes = std::slice::from_raw_parts(reply.buffer.data, reply.buffer.len);
                if reply.status == 0 {
                    serde_json::from_slice(bytes).map_err(|_| {
                        ModuleError::Invalid("The module returned an invalid result.".into())
                    })
                } else {
                    Err(ModuleError::Invalid(
                        String::from_utf8_lossy(bytes).into_owned(),
                    ))
                }
            };
            (self.api.release)(reply.buffer);
            result
        }
    }
}

// Rust statics are not dropped on manager teardown. Process-scoped code remains
// mapped until OS process exit, keeping third-party native callbacks valid.
static PROCESS_LIBRARIES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Library>>>> = OnceLock::new();

pub(crate) fn pinned_under(directory: &Path) -> bool {
    PROCESS_LIBRARIES.get().is_some_and(|libraries| {
        libraries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .keys()
            .any(|path| path.starts_with(directory))
    })
}

unsafe fn load_library(
    path: &Path,
    policy: ModuleUnloadPolicy,
) -> Result<Arc<Library>, ModuleError> {
    unsafe fn open(path: &Path) -> Result<Arc<Library>, ModuleError> {
        #[cfg(target_os = "windows")]
        let library: Library = unsafe {
            libloading::os::windows::Library::load_with_flags(
                path,
                libloading::os::windows::LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR
                    | libloading::os::windows::LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        }
        .map_err(|_| ModuleError::Load)?
        .into();
        #[cfg(not(target_os = "windows"))]
        let library = unsafe { Library::new(path) }.map_err(|_| ModuleError::Load)?;
        Ok(Arc::new(library))
    }
    if policy == ModuleUnloadPolicy::Instance {
        return unsafe { open(path) };
    }
    let mut libraries = PROCESS_LIBRARIES
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .map_err(|_| ModuleError::State)?;
    if let Some(library) = libraries.get(path) {
        return Ok(Arc::clone(library));
    }
    let library = unsafe { open(path) }?;
    libraries.insert(path.into(), Arc::clone(&library));
    Ok(library)
}

impl Drop for NativeModule {
    fn drop(&mut self) {
        // SAFETY: no invocation can overlap drop. The library remains loaded until
        // after destruction; modules must join owned background workers here.
        unsafe {
            (self.api.destroy)(self.context);
        }
    }
}

struct HostBridge {
    id: String,
    services: Arc<crate::SyncServices>,
    accepting: Arc<AtomicBool>,
}
unsafe extern "C" fn host_request(
    context: *mut c_void,
    data: *const u8,
    len: usize,
) -> NativeReply {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if context.is_null() || data.is_null() || len == 0 || len > MAX_HOST_BYTES {
            return Err(ModuleError::State);
        }
        // SAFETY: NativeModule retains the bridge until its module and all workers are destroyed.
        let bridge = unsafe { &*context.cast::<HostBridge>() };
        // SAFETY: the caller lends this bounded input for the callback duration.
        let input: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, len) })
                .map_err(|_| ModuleError::State)?;
        // Already admitted token rotations may finish saving encrypted metadata
        // during teardown. History access and the live lease close immediately.
        if !bridge.accepting.load(Ordering::Acquire)
            && !matches!(
                input.get("operation").and_then(serde_json::Value::as_str),
                Some("read_state" | "write_state" | "clear_state")
            )
        {
            return Err(ModuleError::Disabled);
        }
        let output = bridge.services.request(&bridge.id, input)?;
        let bytes = serde_json::to_vec(&output).map_err(|_| ModuleError::State)?;
        if bytes.len() > MAX_HOST_BYTES {
            return Err(ModuleError::State);
        }
        Ok(bytes)
    }))
    .unwrap_or(Err(ModuleError::State));
    let (status, bytes) = match result {
        Ok(bytes) => (0, bytes),
        Err(_) => (1, b"The sync host service is unavailable.".to_vec()),
    };
    let bytes = bytes.into_boxed_slice();
    let len = bytes.len();
    NativeReply {
        status,
        buffer: NativeBuffer {
            data: Box::into_raw(bytes).cast(),
            len,
        },
    }
}
unsafe extern "C" fn host_release(buffer: NativeBuffer) {
    if !buffer.data.is_null() {
        // SAFETY: the caller releases exactly the host allocation returned by host_request.
        let mut bytes =
            unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(buffer.data, buffer.len)) };
        use zeroize::Zeroize;
        bytes.zeroize();
    }
}
