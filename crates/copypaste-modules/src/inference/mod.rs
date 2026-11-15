//! Process-owned inference. The host retains storage and module admission.

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod android;
mod client;
mod protocol;
mod worker;
#[cfg(target_os = "android")]
pub use android::AndroidInferenceLauncher;

pub(crate) use client::InferenceClient;
pub use client::{DesktopInferenceLauncher, InferenceConnection, InferenceLauncher};
pub(crate) use protocol::{InferenceRequest, WorkerIdentity};
pub use worker::run_inference_worker;

pub(crate) fn is_ocr_command(id: &str, command: &str) -> bool {
    id == "copypaste.ocr" && command == "recognize-image"
}

#[cfg(test)]
pub(crate) use client::tests::launcher_fixture;
