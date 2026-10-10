use jni::{
    objects::{JClass, JString},
    sys::jstring,
    JNIEnv,
};
use std::path::Path;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_qualification_MainActivity_qualify(
    mut environment: JNIEnv,
    class: JClass,
    package: JString,
    fixtures: JString,
    data: JString,
    app_version: JString,
    commit: JString,
    run_id: JString,
    phase: JString,
) -> jstring {
    let result = (|| -> Result<String, String> {
        let package: String = environment
            .get_string(&package)
            .map_err(|error| error.to_string())?
            .into();
        let fixtures: String = environment
            .get_string(&fixtures)
            .map_err(|error| error.to_string())?
            .into();
        let data: String = environment
            .get_string(&data)
            .map_err(|error| error.to_string())?
            .into();
        let app_version: String = environment
            .get_string(&app_version)
            .map_err(|error| error.to_string())?
            .into();
        let commit: String = environment
            .get_string(&commit)
            .map_err(|error| error.to_string())?
            .into();
        let run_id: String = environment
            .get_string(&run_id)
            .map_err(|error| error.to_string())?
            .into();
        let phase: String = environment
            .get_string(&phase)
            .map_err(|error| error.to_string())?
            .into();
        if phase == "cleanup" {
            super::finish_after_restart(Path::new(&data), &app_version)?;
            return Ok("{\"removal_completed_after_restart\":true}".into());
        }
        let vm = std::sync::Arc::new(
            environment
                .get_java_vm()
                .map_err(|error| error.to_string())?,
        );
        let host = environment
            .new_global_ref(class)
            .map_err(|error| error.to_string())?;
        let launcher =
            std::sync::Arc::new(copypaste_modules::AndroidInferenceLauncher::new(vm, host));
        let receipt = super::qualify_with_inference(
            Path::new(&package),
            Path::new(&fixtures),
            Path::new(&data),
            &app_version,
            &commit,
            &run_id,
            Some(launcher),
        )?;
        serde_json::to_string(&receipt).map_err(|error| error.to_string())
    })();
    match result {
        Ok(receipt) => match environment.new_string(receipt) {
            Ok(value) => value.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(error) => {
            let _ = environment.throw_new("java/lang/IllegalStateException", error);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_copypaste_qualification_InferenceWorkerService_runWorker(
    _environment: JNIEnv,
    _class: JClass,
    descriptor: jni::sys::jint,
) {
    use std::os::{fd::FromRawFd, unix::net::UnixStream};
    if descriptor < 0 {
        return;
    }
    let reader = unsafe { UnixStream::from_raw_fd(descriptor) };
    if let Ok(writer) = reader.try_clone() {
        let _ = copypaste_modules::run_inference_worker(reader, writer);
    }
}
