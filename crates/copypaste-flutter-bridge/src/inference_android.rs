//! JNI entrypoint for the dedicated inference service, without Runtime bootstrap.
use jni::{objects::JClass, sys::jint, JNIEnv};
use std::os::{fd::FromRawFd, unix::net::UnixStream};

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_app_InferenceWorkerService_runWorker(
    _env: JNIEnv,
    _class: JClass,
    descriptor: jint,
) {
    if descriptor < 0 {
        return;
    }
    // The bound service transfers its peer descriptor and never initializes
    // Runtime or the Android key store in this dedicated process.
    let stream = unsafe { UnixStream::from_raw_fd(descriptor) };
    if let Ok(writer) = stream.try_clone() {
        let _ = copypaste_modules::run_inference_worker(stream, writer);
    }
}
