use copypaste_core::device_name::SystemDeviceName;
use jni::{
    objects::{JObject, JString, JValue},
    JNIEnv, JavaVM,
};

pub fn system_device_name() -> SystemDeviceName {
    let context = ndk_context::android_context();
    // KeystoreContext installs process-lifetime VM and application references
    // before the backend opens; local references stay inside this JNI frame.
    let Ok(vm) = (unsafe { JavaVM::from_raw(context.vm().cast()) }) else {
        return SystemDeviceName::from_sources(None, None);
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return SystemDeviceName::from_sources(None, None);
    };
    let result = env.with_local_frame(16, |env| {
        let app = unsafe { JObject::from_raw(context.context().cast()) };
        let system = read_system_name(env, &app);
        if system.is_err() {
            env.exception_clear()?;
        }
        let model = read_model(env);
        if model.is_err() {
            env.exception_clear()?;
        }
        Ok::<_, jni::errors::Error>(SystemDeviceName::from_sources(
            system.ok().flatten().as_deref(),
            model.ok().flatten().as_deref(),
        ))
    });
    result.unwrap_or_else(|_| SystemDeviceName::from_sources(None, None))
}

fn read_system_name(
    env: &mut JNIEnv<'_>,
    app: &JObject<'_>,
) -> jni::errors::Result<Option<String>> {
    let resolver = env
        .call_method(
            app,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )?
        .l()?;
    let key = env.new_string("device_name")?;
    let value = env
        .call_static_method(
            "android/provider/Settings$Global",
            "getString",
            "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&resolver), JValue::Object(&key)],
        )?
        .l()?;
    if value.is_null() {
        return Ok(None);
    }
    let name: String = env.get_string(&JString::from(value))?.into();
    Ok(Some(name))
}

fn read_model(env: &mut JNIEnv<'_>) -> jni::errors::Result<Option<String>> {
    let value = env
        .get_static_field("android/os/Build", "MODEL", "Ljava/lang/String;")?
        .l()?;
    if value.is_null() {
        return Ok(None);
    }
    let model: String = env.get_string(&JString::from(value))?.into();
    Ok(Some(model))
}
