//! All native loading and pointer ownership are confined to this boundary.
use crate::{
    bridge::{HostAccess, HostBridge, HostShared},
    endpoints::Endpoints,
    ModuleError,
};
use copypaste_module_sdk::{
    native::{
        NativeApi, NativeBuffer, NativeHostApi, NativeHostApiV2, NativeReply, MAX_HOST_BYTES,
    },
    HostError, ModuleDelivery, ModuleEnvironment, ModuleInvocation, ModuleOutput,
    ModuleUnloadPolicy, MAX_DELIVERY_BYTES, MAX_INVOCATION_BYTES, MODULE_API_VERSION,
};
use libloading::Library;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

/// Each module owns its execution lock. Long commands do not hold the registry
/// lock or serialize commands in other modules. Lifecycle changes close admission
/// first and wait for already running work before releasing code and assets.
pub(crate) struct ModuleInstance {
    id: String,
    owner: u64,
    shared: Arc<HostShared>,
    accepting: Arc<AtomicBool>,
    lifetime: Arc<crate::host_lifetime::HostLifetime>,
    endpoints: Arc<Endpoints>,
    module: Mutex<Option<NativeModule>>,
}

/// What loading an instance on first use needs.
pub(crate) struct LoadSpec<'a> {
    pub(crate) verifier: &'a crate::package::PackageVerifier,
    pub(crate) package_dir: &'a Path,
    pub(crate) data_dir: &'a Path,
    pub(crate) preferences: &'a BTreeMap<String, serde_json::Value>,
    pub(crate) access: HostAccess,
}

impl ModuleInstance {
    pub(crate) fn new(id: &str, shared: Arc<HostShared>) -> Self {
        static OWNERS: AtomicU64 = AtomicU64::new(0);
        Self {
            id: id.into(),
            owner: OWNERS.fetch_add(1, Ordering::Relaxed),
            shared,
            accepting: Arc::new(AtomicBool::new(true)),
            lifetime: Arc::new(crate::host_lifetime::HostLifetime::default()),
            endpoints: Arc::default(),
            module: Mutex::new(None),
        }
    }

    pub(crate) fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    fn loaded<'a>(
        &self,
        module: &'a mut Option<NativeModule>,
        spec: LoadSpec<'_>,
    ) -> Result<&'a mut NativeModule, ModuleError> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err(ModuleError::Disabled);
        }
        if module.is_none() {
            let manifest = spec.verifier.installed(spec.package_dir, true)?;
            std::fs::create_dir_all(spec.data_dir)?;
            if let Some(provider) = &manifest.search_provider {
                let languages = spec
                    .preferences
                    .get(&provider.language_field)
                    .and_then(serde_json::Value::as_array)
                    .ok_or(ModuleError::State)?
                    .iter()
                    .map(|value| value.as_str().map(str::to_owned).ok_or(ModuleError::State))
                    .collect::<Result<Vec<_>, _>>()?;
                let model = provider.model_for(&languages).ok_or(ModuleError::State)?;
                crate::resources::verify(spec.data_dir, model)?;
            }
            let environment = ModuleEnvironment {
                package_dir: spec.package_dir.to_string_lossy().into_owned(),
                data_dir: spec.data_dir.to_string_lossy().into_owned(),
            };
            let host = spec.access.needed().then(|| {
                Box::new(HostBridge {
                    id: self.id.clone(),
                    owner: self.owner,
                    access: spec.access,
                    shared: Arc::clone(&self.shared),
                    accepting: Arc::clone(&self.accepting),
                    lifetime: Arc::clone(&self.lifetime),
                    endpoints: Arc::clone(&self.endpoints),
                })
            });
            *module = Some(NativeModule::load(
                &spec.package_dir.join(&manifest.entrypoint),
                &environment,
                manifest.unload_policy,
                !manifest.endpoints.is_empty(),
                host,
            )?);
        }
        module.as_mut().ok_or(ModuleError::Load)
    }

    pub(crate) fn invoke(
        &self,
        spec: LoadSpec<'_>,
        invocation: &ModuleInvocation,
    ) -> Result<ModuleOutput, ModuleError> {
        let mut module = self.module.lock().map_err(|_| ModuleError::State)?;
        self.loaded(&mut module, spec)?.invoke(invocation)
    }

    /// Loads the module if needed and delivers `EndpointOpen`.
    pub(crate) fn open_endpoint(
        &self,
        spec: LoadSpec<'_>,
        open: &ModuleDelivery,
    ) -> Result<(), ModuleError> {
        let mut module = self.module.lock().map_err(|_| ModuleError::State)?;
        self.loaded(&mut module, spec)?.deliver(open)
    }

    /// Delivers to an open connection of the already loaded module.
    pub(crate) fn deliver(
        &self,
        connection: &str,
        delivery: &ModuleDelivery,
    ) -> Result<(), ModuleError> {
        let mut module = self.module.lock().map_err(|_| ModuleError::State)?;
        if !self.accepting.load(Ordering::Acquire) || !self.endpoints.is_open(connection) {
            return Err(ModuleError::Disabled);
        }
        module
            .as_mut()
            .ok_or(ModuleError::Disabled)?
            .deliver(delivery)
    }

    /// The final delivery of a connection; a stopping instance already sent it.
    pub(crate) fn deliver_close(&self, connection: &str) {
        let Ok(mut module) = self.module.lock() else {
            return;
        };
        if self.accepting.load(Ordering::Acquire) {
            if let Some(module) = module.as_mut() {
                let _ = module.deliver(&ModuleDelivery::EndpointClose {
                    connection: connection.into(),
                });
            }
        }
    }

    pub(crate) fn stop(&self) {
        self.accepting.store(false, Ordering::Release);
        self.lifetime.revoke();
        // A command waiting on the user must return before teardown can lock it.
        self.shared.prompts.dismiss(self.owner);
        let connections = self.endpoints.close_all();
        // Poison must not prevent teardown after a committed lifecycle change.
        let mut module = self
            .module
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(loaded) = module.as_mut() {
            for connection in connections {
                let _ = loaded.deliver(&ModuleDelivery::EndpointClose { connection });
            }
        }
        module.take();
    }
}

type DeliverFn = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> NativeReply;

pub(crate) struct NativeModule {
    api: NativeApi,
    deliver: Option<DeliverFn>,
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
        endpoints: bool,
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
            // Resolved before creation, so a module missing it never runs.
            let deliver = if endpoints {
                Some(
                    *library
                        .get::<DeliverFn>(b"copypaste_module_deliver_v1\0")
                        .map_err(|_| ModuleError::Load)?,
                )
            } else {
                None
            };
            let bytes = serde_json::to_vec(environment).map_err(|_| ModuleError::State)?;
            let context = if let Some(bridge) = host.as_deref() {
                let host_api = NativeHostApi {
                    api_version: MODULE_API_VERSION,
                    struct_size: std::mem::size_of::<NativeHostApi>(),
                    context: (bridge as *const HostBridge).cast_mut().cast(),
                    request: host_request,
                    release: host_release,
                };
                if let Ok(create) =
                    library.get::<unsafe extern "C" fn(
                        *const u8,
                        usize,
                        *const NativeHostApiV2,
                    ) -> *mut c_void>(b"copypaste_module_with_host_v2\0")
                {
                    let host_v2 = NativeHostApiV2 {
                        api_version: 2,
                        struct_size: std::mem::size_of::<NativeHostApiV2>(),
                        base: host_api,
                        subscribe: host_subscribe,
                        unsubscribe: host_unsubscribe,
                    };
                    create(bytes.as_ptr(), bytes.len(), &host_v2)
                } else {
                    let create: libloading::Symbol<
                        unsafe extern "C" fn(*const u8, usize, *const NativeHostApi) -> *mut c_void,
                    > = library
                        .get(b"copypaste_module_with_host_v1\0")
                        .map_err(|_| ModuleError::Load)?;
                    create(bytes.as_ptr(), bytes.len(), &host_api)
                }
            } else {
                (api.create)(bytes.as_ptr(), bytes.len())
            };
            if context.is_null() {
                return Err(ModuleError::Load);
            }
            Ok(Self {
                api,
                deliver,
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

    fn deliver(&mut self, delivery: &ModuleDelivery) -> Result<(), ModuleError> {
        let deliver = self
            .deliver
            .ok_or_else(|| ModuleError::Invalid("This module has no endpoints.".into()))?;
        let bytes =
            zeroize::Zeroizing::new(serde_json::to_vec(delivery).map_err(|_| ModuleError::State)?);
        if bytes.len() > MAX_DELIVERY_BYTES {
            return Err(ModuleError::Invalid(
                "The message exceeds its size limit.".into(),
            ));
        }
        // SAFETY: as for `invoke`; the export shares its context and release.
        unsafe {
            let reply = deliver(self.context, bytes.as_ptr(), bytes.len());
            let result = if reply.status == 0 {
                Ok(())
            } else if reply.buffer.data.is_null() || reply.buffer.len > MAX_INVOCATION_BYTES {
                Err(ModuleError::Invalid(
                    "The module refused the message.".into(),
                ))
            } else {
                let bytes = std::slice::from_raw_parts(reply.buffer.data, reply.buffer.len);
                Err(ModuleError::Invalid(
                    String::from_utf8_lossy(bytes).into_owned(),
                ))
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

unsafe extern "C" fn host_subscribe(
    context: *mut c_void,
    callback: unsafe extern "C" fn(*mut c_void),
    callback_context: *mut c_void,
) -> u64 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: the bridge remains alive through module destruction.
    unsafe { &*context.cast::<HostBridge>() }
        .lifetime
        .subscribe(callback, callback_context)
}
unsafe extern "C" fn host_unsubscribe(context: *mut c_void, id: u64) {
    if !context.is_null() {
        // SAFETY: subscriptions are dropped before module destruction returns.
        unsafe { &*context.cast::<HostBridge>() }
            .lifetime
            .unsubscribe(id);
    }
}
unsafe extern "C" fn host_request(
    context: *mut c_void,
    data: *const u8,
    len: usize,
) -> NativeReply {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<Vec<u8>, Option<HostError>> {
            if context.is_null() || data.is_null() || len == 0 || len > MAX_HOST_BYTES {
                return Err(None);
            }
            // SAFETY: NativeModule retains the bridge until its module and all workers are destroyed.
            let bridge = unsafe { &*context.cast::<HostBridge>() };
            // SAFETY: the caller lends this bounded input for the callback duration.
            bridge.handle(unsafe { std::slice::from_raw_parts(data, len) })
        },
    ))
    .unwrap_or(Err(None));
    let (status, bytes) = match result {
        Ok(bytes) => (0, bytes),
        Err(Some(error)) => (1, serde_json::to_vec(&error).unwrap_or_default()),
        Err(None) => (1, b"The sync host service is unavailable.".to_vec()),
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
