//! Native protected-presenter capability bridge.
//!
//! These C symbols carry only opaque ids supplied by native code. They return
//! a boolean and allocate nothing, so no pairing secret or custom buffer
//! protocol crosses the language boundary. The protected presenter must use
//! this binding before it can consider a context active.

use std::ffi::{c_char, CStr};
use std::sync::OnceLock;

#[repr(C)]
pub struct ProtectedPairingBuffer {
    pub bytes: *mut u8,
    pub len: usize,
}
#[repr(C)]
pub struct ProtectedPairingStatus {
    pub state: u32,
    pub expires_in_ms: u64,
}

/// Attaches a native-only protected context to an existing Rust ceremony.
///
/// # Safety
///
/// Both pointers must name NUL-terminated UTF-8 strings valid for the duration
/// of this call. The function neither stores nor frees either pointer.
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_attach_protected_pairing_context(
    ceremony_id: *const c_char,
    context_id: *const c_char,
) -> bool {
    let (Some(ceremony_id), Some(context_id)) =
        (unsafe { (read_identifier(ceremony_id), read_identifier(context_id)) })
    else {
        return false;
    };
    crate::protected::attach_native_context(&ceremony_id, &context_id).is_some()
}

/// Attaches a context and returns its current non-zero ceremony generation.
/// Zero means the ceremony/context pair was rejected.
///
/// # Safety
///
/// Both pointers must name NUL-terminated UTF-8 strings valid for this call.
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_begin_protected_pairing_context(
    ceremony_id: *const c_char,
    context_id: *const c_char,
) -> u64 {
    let (Some(ceremony_id), Some(context_id)) =
        (unsafe { (read_identifier(ceremony_id), read_identifier(context_id)) })
    else {
        return 0;
    };
    crate::protected::attach_native_context(&ceremony_id, &context_id).unwrap_or(0)
}

/// Renders an invitation as a PNG after the bound presenter has enabled
/// capture protection. The native caller owns the returned buffer and must release it
/// with [`copypaste_flutter_free_protected_pairing_buffer`].
///
/// # Safety
///
/// All identifier pointers must be valid NUL-terminated UTF-8 for this call;
/// `output` must be a writable pointer to one [`ProtectedPairingBuffer`].
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_reveal_protected_pairing_qr_png(
    ceremony_id: *const c_char,
    context_id: *const c_char,
    generation: u64,
    output: *mut ProtectedPairingBuffer,
) -> bool {
    if output.is_null() {
        return false;
    }
    let (Some(ceremony_id), Some(context_id)) =
        (unsafe { (read_identifier(ceremony_id), read_identifier(context_id)) })
    else {
        return false;
    };
    let Some(mut bytes) = native_runtime().block_on(crate::protected::reveal_qr_png(
        &ceremony_id,
        &context_id,
        generation,
    )) else {
        return false;
    };
    bytes.shrink_to_fit();
    let buffer = ProtectedPairingBuffer {
        bytes: bytes.as_mut_ptr(),
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    unsafe { output.write(buffer) };
    true
}

/// Reveals the handshake-bound SAS only while the current ceremony is awaiting
/// confirmation and its Rust-owned lifetime remains positive.
///
/// # Safety
///
/// `context_id` and `output` follow the same rules as QR reveal.
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_reveal_protected_pairing_sas(
    context_id: *const c_char,
    generation: u64,
    output: *mut ProtectedPairingBuffer,
) -> bool {
    if output.is_null() {
        return false;
    }
    let Some(context_id) = (unsafe { read_identifier(context_id) }) else {
        return false;
    };
    let Some(mut bytes) =
        native_runtime().block_on(crate::protected::reveal_sas(&context_id, generation))
    else {
        return false;
    };
    bytes.shrink_to_fit();
    let buffer = ProtectedPairingBuffer {
        bytes: bytes.as_mut_ptr(),
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    unsafe { output.write(buffer) };
    true
}

#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_protected_pairing_join(
    context: *const c_char,
    generation: u64,
    code: *const c_char,
    addr: *const c_char,
) -> bool {
    let (Some(context), Some(code), Some(addr)) = (unsafe {
        (
            read_identifier(context),
            read_identifier(code),
            read_identifier(addr),
        )
    }) else {
        return false;
    };
    native_runtime().block_on(crate::protected::native_join(
        &context, generation, code, addr,
    ))
}

#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_protected_pairing_join_uri(
    context: *const c_char,
    generation: u64,
    uri: *const c_char,
) -> bool {
    let (Some(context), Some(uri)) = (unsafe { (read_identifier(context), read_identifier(uri)) })
    else {
        return false;
    };
    native_runtime().block_on(crate::protected::native_join_uri(&context, generation, uri))
}

#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_protected_pairing_status(
    context: *const c_char,
    generation: u64,
    output: *mut ProtectedPairingStatus,
) -> bool {
    if output.is_null() {
        return false;
    }
    let Some(context) = (unsafe { read_identifier(context) }) else {
        return false;
    };
    let Some((state, expires_in_ms)) =
        native_runtime().block_on(crate::protected::native_status(&context, generation))
    else {
        return false;
    };
    unsafe {
        output.write(ProtectedPairingStatus {
            state,
            expires_in_ms,
        })
    };
    true
}

#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_protected_pairing_decide(
    context: *const c_char,
    generation: u64,
    sas: *const c_char,
    accept: bool,
) -> bool {
    let (Some(context), Some(sas)) = (unsafe { (read_identifier(context), read_identifier(sas)) })
    else {
        return false;
    };
    native_runtime().block_on(crate::protected::native_decision(
        &context,
        generation,
        zeroize::Zeroizing::new(sas),
        accept,
    ))
}

/// Releases and zeroizes a QR or other future protected artifact buffer.
///
/// # Safety
///
/// `buffer` must be a buffer returned once by this library and not previously
/// freed. Callers must never pass arbitrary pointers.
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_free_protected_pairing_buffer(
    buffer: ProtectedPairingBuffer,
) {
    if buffer.bytes.is_null() || buffer.len == 0 {
        return;
    }
    let mut bytes = unsafe { Vec::from_raw_parts(buffer.bytes, buffer.len, buffer.len) };
    zeroize::Zeroize::zeroize(&mut bytes);
}

/// Answers whether a native protected presenter still owns an active context.
///
/// # Safety
///
/// `context_id` must name a NUL-terminated UTF-8 string valid for this call.
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_protected_pairing_context_active(
    context_id: *const c_char,
) -> bool {
    let Some(context_id) = (unsafe { read_identifier(context_id) }) else {
        return false;
    };
    crate::protected::native_context_active(&context_id)
}

/// Detaches a native protected context during presenter teardown.
///
/// # Safety
///
/// `context_id` must name a NUL-terminated UTF-8 string valid for this call.
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_detach_protected_pairing_context(
    context_id: *const c_char,
) -> bool {
    let Some(context_id) = (unsafe { read_identifier(context_id) }) else {
        return false;
    };
    crate::protected::detach_native_context(&context_id)
}

/// Cancels the ceremony owned by a native protected context and releases all
/// Rust-held invitation state. Call this for close, Escape and presenter
/// teardown before detaching the context.
///
/// # Safety
///
/// `context_id` must name a NUL-terminated UTF-8 string valid for this call.
#[no_mangle]
pub unsafe extern "C" fn copypaste_flutter_cancel_protected_pairing_context(
    context_id: *const c_char,
) -> bool {
    let Some(context_id) = (unsafe { read_identifier(context_id) }) else {
        return false;
    };
    native_runtime().block_on(crate::protected::cancel_native_context(&context_id))
}

fn native_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("native protected pairing runtime initializes")
    })
}

unsafe fn read_identifier(pointer: *const c_char) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    // Copy before returning so no borrowed native pointer escapes this call.
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .ok()
        .map(str::to_owned)
}
