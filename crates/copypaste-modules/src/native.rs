//! All native loading and pointer ownership are confined to this boundary.
use crate::ModuleError;
use copypaste_module_sdk::{
    native::NativeApi, ModuleEnvironment, ModuleInvocation, ModuleOutput, ModuleUnloadPolicy,
    MAX_INVOCATION_BYTES, MODULE_API_VERSION,
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
    accepting: AtomicBool,
    module: Mutex<Option<NativeModule>>,
}

impl ModuleInstance {
    pub(crate) fn new() -> Self {
        Self {
            accepting: AtomicBool::new(true),
            module: Mutex::new(None),
        }
    }

    pub(crate) fn invoke(
        &self,
        verifier: &crate::package::PackageVerifier,
        package_dir: &Path,
        data_dir: &Path,
        invocation: &ModuleInvocation,
    ) -> Result<ModuleOutput, ModuleError> {
        let mut module = self.module.lock().map_err(|_| ModuleError::State)?;
        if !self.accepting.load(Ordering::Acquire) {
            return Err(ModuleError::Disabled);
        }
        if module.is_none() {
            let manifest = verifier.installed(package_dir, true)?;
            std::fs::create_dir_all(data_dir)?;
            let environment = ModuleEnvironment {
                package_dir: package_dir.to_string_lossy().into_owned(),
                data_dir: data_dir.to_string_lossy().into_owned(),
            };
            *module = Some(NativeModule::load(
                &package_dir.join(manifest.entrypoint),
                &environment,
                manifest.unload_policy,
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
}

// SAFETY: SDK modules are Send; the per-instance mutex serializes calls and drop.
unsafe impl Send for NativeModule {}

impl NativeModule {
    pub(crate) fn load(
        path: &Path,
        environment: &ModuleEnvironment,
        policy: ModuleUnloadPolicy,
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
            let context = (api.create)(bytes.as_ptr(), bytes.len());
            if context.is_null() {
                return Err(ModuleError::Load);
            }
            Ok(Self {
                api,
                context,
                _library: library,
            })
        }
    }

    pub(crate) fn invoke(
        &mut self,
        invocation: &ModuleInvocation,
    ) -> Result<ModuleOutput, ModuleError> {
        let bytes = serde_json::to_vec(invocation).map_err(|_| ModuleError::State)?;
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
