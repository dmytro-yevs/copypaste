//! Bound Android processes implement the shared private inference channel.
use super::{InferenceConnection, InferenceLauncher};
use crate::ModuleError;
use jni::objects::JLongArray;
use std::{
    os::{fd::FromRawFd, unix::net::UnixStream},
    sync::Arc,
};

pub struct AndroidInferenceLauncher {
    vm: Arc<jni::JavaVM>,
    host: jni::objects::GlobalRef,
}

impl AndroidInferenceLauncher {
    pub fn new(vm: Arc<jni::JavaVM>, host: jni::objects::GlobalRef) -> Self {
        Self { vm, host }
    }
}

impl InferenceLauncher for AndroidInferenceLauncher {
    fn launch(&self) -> Result<InferenceConnection, ModuleError> {
        let vm = Arc::clone(&self.vm);
        let class = self.host.clone();
        let mut env = vm.attach_current_thread().map_err(|_| ModuleError::Load)?;
        let result = env.call_static_method(&class, "openWorker", "()[J", &[]);
        let result = match result {
            Ok(result) => result.l().map_err(|_| ModuleError::Load)?,
            Err(_) => {
                let _ = env.exception_clear();
                return Err(ModuleError::Load);
            }
        };
        let result = JLongArray::from(result);
        let mut values = [0; 2];
        if env.get_array_length(&result).ok() != Some(2) {
            return Err(ModuleError::Load);
        }
        env.get_long_array_region(&result, 0, &mut values)
            .map_err(|_| ModuleError::Load)?;
        let [descriptor, session] = values;
        if descriptor < 0 || descriptor > i32::MAX as i64 || session <= 0 {
            return Err(ModuleError::Load);
        }
        // Kotlin detaches this owned socket descriptor exactly once. The service
        // owns the peer, so shutdown interrupts IPC without accessing its model.
        let stream = unsafe { UnixStream::from_raw_fd(descriptor as i32) };
        let vm = Arc::clone(&vm);
        let stop = Arc::new(move || {
            if let Ok(mut env) = vm.attach_current_thread() {
                if env
                    .call_static_method(
                        &class,
                        "closeWorker",
                        "(J)V",
                        &[jni::objects::JValue::Long(session)],
                    )
                    .is_err()
                {
                    let _ = env.exception_clear();
                }
            }
        });
        let reader = match stream.try_clone() {
            Ok(reader) => reader,
            Err(error) => {
                stop();
                return Err(error.into());
            }
        };
        let control = match stream.try_clone() {
            Ok(control) => control,
            Err(error) => {
                stop();
                return Err(error.into());
            }
        };
        let terminate = Arc::new(move || {
            let _ = control.shutdown(std::net::Shutdown::Both);
            stop();
        });
        InferenceConnection::new(reader, stream, terminate)
    }
}
