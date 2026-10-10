use super::protocol::{read_frame, write_frame, InferenceReply, InferenceRequest};
use crate::{native::ModuleInstance, package::PackageVerifier, ModuleError};
use copypaste_module_sdk::resolve_fields;
use std::io::{Read, Write};

/// Serves only authenticated search modules over an inherited private channel.
/// EOF ends the worker; no database, key store, clipboard, or listener is opened.
pub fn run_inference_worker(
    mut reader: impl Read,
    mut writer: impl Write,
) -> Result<(), ModuleError> {
    let instance = ModuleInstance::new();
    let mut context_key = None;
    while let Some(request) = read_frame::<InferenceRequest>(&mut reader)? {
        let key = serde_json::to_vec(&(
            &request.package_dir,
            &request.data_dir,
            &request.id,
            &request.invocation.command,
            &request.invocation.preferences,
        ))
        .map_err(|_| ModuleError::State)?;
        if context_key
            .as_ref()
            .is_some_and(|previous| previous != &key)
        {
            return Err(ModuleError::State);
        }
        context_key = Some(key);
        let reply: InferenceReply = invoke(&instance, &request)
            .map_err(|_| "The search worker could not complete the request.".into());
        write_frame(&mut writer, &reply)?;
    }
    Ok(())
}

fn invoke(
    instance: &ModuleInstance,
    request: &InferenceRequest,
) -> Result<copypaste_module_sdk::ModuleOutput, ModuleError> {
    let verifier = PackageVerifier::new(
        &request.identity.public_key,
        &request.identity.app_version,
        request.identity.target,
    )?;
    let manifest = verifier.installed(&request.package_dir, false)?;
    let provider = manifest
        .search_provider
        .as_ref()
        .ok_or(ModuleError::State)?;
    if manifest.id != request.id || provider.command != request.invocation.command {
        return Err(ModuleError::State);
    }
    let command = manifest
        .commands
        .iter()
        .find(|command| command.id == provider.command)
        .ok_or(ModuleError::State)?;
    resolve_fields(&command.arguments, &request.invocation.arguments)?;
    resolve_fields(&manifest.preferences, &request.invocation.preferences)?;
    instance.invoke(
        &verifier,
        &request.package_dir,
        &request.data_dir,
        &request.invocation,
        None,
        &request.id,
    )
}
