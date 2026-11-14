//! The module through its exported C ABI, with the fake behind a real
//! `HostClient`.
#![allow(unsafe_code)]
use super::fake::Fake;
use crate::host::Host;
use crate::AiTools;
use copypaste_module_sdk::native::{
    create_with_host, deliver, Export, NativeBuffer, NativeHostApi, NativeReply,
};
use copypaste_module_sdk::{
    ModuleDelivery, ModuleEnvironment, ModuleHostRequest, ModuleInvocation, MODULE_API_VERSION,
};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::OnceLock;

static HOST: OnceLock<Fake> = OnceLock::new();

fn owned(status: u32, bytes: Vec<u8>) -> NativeReply {
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

unsafe extern "C" fn request(_: *mut c_void, data: *const u8, len: usize) -> NativeReply {
    // SAFETY: the client lends the request for this call.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    let request: ModuleHostRequest = serde_json::from_slice(bytes).unwrap();
    match HOST.get_or_init(Fake::default).call(&request) {
        Ok(value) => owned(0, serde_json::to_vec(&value).unwrap()),
        Err(error) => owned(1, serde_json::to_vec(&error).unwrap()),
    }
}

unsafe extern "C" fn release(buffer: NativeBuffer) {
    // SAFETY: every buffer comes from `owned`.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(buffer.data, buffer.len)) });
}

fn reply_text(reply: NativeReply) -> (u32, String) {
    // SAFETY: the module owns the reply until it is released.
    let text = unsafe { std::slice::from_raw_parts(reply.buffer.data, reply.buffer.len) };
    let text = String::from_utf8_lossy(text).into_owned();
    // SAFETY: released once with the module's own release.
    unsafe { (Export::<AiTools>::API.release)(reply.buffer) };
    (reply.status, text)
}

#[test]
fn the_exported_module_reaches_the_host_through_host_client() {
    let api = NativeHostApi {
        api_version: MODULE_API_VERSION,
        struct_size: std::mem::size_of::<NativeHostApi>(),
        context: std::ptr::dangling_mut(),
        request,
        release,
    };
    let environment = serde_json::to_vec(&ModuleEnvironment {
        package_dir: "package".into(),
        data_dir: "data".into(),
    })
    .unwrap();
    // SAFETY: the host record and buffers outlive the module.
    let module =
        unsafe { create_with_host::<AiTools>(environment.as_ptr(), environment.len(), &api) };
    assert!(!module.is_null());

    let invocation = serde_json::to_vec(&ModuleInvocation {
        command: "connections".into(),
        arguments: BTreeMap::new(),
        preferences: BTreeMap::new(),
    })
    .unwrap();
    // SAFETY: the context came from create_with_host and calls are serialized.
    let reply =
        unsafe { (Export::<AiTools>::API.invoke)(module, invocation.as_ptr(), invocation.len()) };
    let (status, text) = reply_text(reply);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("\"view\""), "{text}");
    let operations = HOST.get().unwrap().state().operations.clone();
    assert!(
        operations.contains(&"record_list".to_owned()),
        "{operations:?}"
    );

    for delivery in [
        ModuleDelivery::EndpointOpen {
            endpoint: "mcp".into(),
            connection: "c1".into(),
            privileged: false,
        },
        ModuleDelivery::EndpointClose {
            connection: "c1".into(),
        },
    ] {
        let bytes = serde_json::to_vec(&delivery).unwrap();
        // SAFETY: as above.
        let (status, text) =
            reply_text(unsafe { deliver::<AiTools>(module, bytes.as_ptr(), bytes.len()) });
        assert_eq!(status, 0, "{text}");
    }
    // SAFETY: destroyed exactly once, after the last call.
    unsafe { (Export::<AiTools>::API.destroy)(module) };
}
