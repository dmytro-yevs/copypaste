//! The audited C ABI boundary. Allocations are always released by their owner.
use crate::{
    Module, ModuleEnvironment, ModuleInvocation, MAX_INVOCATION_BYTES, MODULE_API_VERSION,
};
use std::{
    ffi::c_void,
    marker::PhantomData,
    panic::{catch_unwind, AssertUnwindSafe},
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeBuffer {
    pub data: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeReply {
    pub status: u32,
    pub buffer: NativeBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeApi {
    pub api_version: u32,
    pub struct_size: usize,
    pub create: unsafe extern "C" fn(*const u8, usize) -> *mut c_void,
    pub invoke: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> NativeReply,
    pub destroy: unsafe extern "C" fn(*mut c_void),
    pub release: unsafe extern "C" fn(NativeBuffer),
}

pub struct Export<T>(PhantomData<T>);
impl<T: Module> Export<T> {
    pub const API: NativeApi = NativeApi {
        api_version: MODULE_API_VERSION,
        struct_size: std::mem::size_of::<NativeApi>(),
        create: create::<T>,
        invoke: invoke::<T>,
        destroy: destroy::<T>,
        release,
    };
}

unsafe fn decode<T: serde::de::DeserializeOwned>(data: *const u8, len: usize) -> Result<T, String> {
    if data.is_null() || len == 0 || len > MAX_INVOCATION_BYTES {
        return Err("Invalid module input.".into());
    }
    // SAFETY: ABI callers provide a readable borrowed buffer for the duration of this call.
    serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, len) })
        .map_err(|_| "Invalid module input.".into())
}

unsafe extern "C" fn create<T: Module>(data: *const u8, len: usize) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: forwarded from the documented native entrypoint contract.
        let environment: ModuleEnvironment = unsafe { decode(data, len) }?;
        T::create(environment).map(|module| Box::into_raw(Box::new(module)).cast())
    }))
    .ok()
    .and_then(Result::ok)
    .unwrap_or(std::ptr::null_mut())
}

unsafe extern "C" fn invoke<T: Module>(
    context: *mut c_void,
    data: *const u8,
    len: usize,
) -> NativeReply {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if context.is_null() {
            return Err("The module is not initialized.".into());
        }
        // SAFETY: the host serializes calls and retains the context returned by create<T>.
        let module = unsafe { &mut *context.cast::<T>() };
        // SAFETY: input is borrowed only for this invocation.
        let invocation: ModuleInvocation = unsafe { decode(data, len) }?;
        module.invoke(invocation)
    }))
    .unwrap_or_else(|_| Err("The module command failed.".into()));
    let (status, bytes) = match result {
        Ok(output) => match serde_json::to_vec(&output) {
            Ok(bytes) if bytes.len() <= MAX_INVOCATION_BYTES => (0, bytes),
            _ => (1, b"The module result exceeds its limit.".to_vec()),
        },
        Err(error) => (1, error.into_bytes().into_iter().take(4096).collect()),
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

unsafe extern "C" fn destroy<T: Module>(context: *mut c_void) {
    if !context.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: the host destroys this owned context exactly once after calls finish.
            drop(unsafe { Box::from_raw(context.cast::<T>()) });
        }));
    }
}

unsafe extern "C" fn release(buffer: NativeBuffer) {
    if !buffer.data.is_null() {
        // SAFETY: this is the unchanged buffer returned by invoke, released once by the host.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(buffer.data, buffer.len)) });
    }
}

/// Host callbacks remain valid until the native module finishes destruction.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeHostApi {
    pub api_version: u32,
    pub struct_size: usize,
    pub context: *mut c_void,
    pub request: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> NativeReply,
    pub release: unsafe extern "C" fn(NativeBuffer),
}

/// Large clipboard payloads use a separately bounded host channel; ordinary
/// module commands retain their existing 1 MiB limit.
pub const MAX_HOST_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct HostClient(NativeHostApi);
// SAFETY: the host supplies thread-safe callbacks and retains their context
// until every module-owned worker has joined during destruction.
unsafe impl Send for HostClient {}
unsafe impl Sync for HostClient {}
impl HostClient {
    pub fn request<I: serde::Serialize, O: serde::de::DeserializeOwned>(
        &self,
        input: &I,
    ) -> Result<O, String> {
        let mut bytes = serde_json::to_vec(input).map_err(|_| "Invalid host request.")?;
        if bytes.len() > MAX_HOST_BYTES {
            return Err("The host request exceeds its limit.".into());
        }
        // SAFETY: callback ownership is established by create_with_host.
        unsafe {
            let reply = (self.0.request)(self.0.context, bytes.as_ptr(), bytes.len());
            bytes.fill(0);
            let result = if reply.buffer.data.is_null() || reply.buffer.len > MAX_HOST_BYTES {
                Err("Invalid host response.".into())
            } else {
                let bytes = std::slice::from_raw_parts(reply.buffer.data, reply.buffer.len);
                if reply.status == 0 {
                    serde_json::from_slice(bytes).map_err(|_| "Invalid host response.".into())
                } else {
                    Err("The sync host service is unavailable.".into())
                }
            };
            (self.0.release)(reply.buffer);
            result
        }
    }
}

/// # Safety
/// The host must retain a valid thread-safe API and context until the returned
/// module is destroyed. Module-owned workers must join before destruction ends.
pub unsafe fn create_with_host<T: Module>(
    data: *const u8,
    len: usize,
    host: *const NativeHostApi,
) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(|| {
        if host.is_null() {
            return Err("The host is missing.".into());
        }
        // SAFETY: checked non-null; the caller promises a readable API record.
        let host = unsafe { *host };
        if host.api_version != MODULE_API_VERSION
            || host.struct_size != std::mem::size_of::<NativeHostApi>()
            || host.context.is_null()
        {
            return Err("The host API is incompatible.".into());
        }
        // SAFETY: environment bytes are borrowed for this call.
        let environment = unsafe { decode(data, len) }?;
        T::create_with_host(environment, HostClient(host))
            .map(|module| Box::into_raw(Box::new(module)).cast())
    }))
    .ok()
    .and_then(Result::ok)
    .unwrap_or(std::ptr::null_mut())
}
