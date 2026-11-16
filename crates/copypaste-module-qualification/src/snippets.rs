//! The snippets package inside the application runtime, which serves its
//! synced records and snippet services exactly as every platform host does.
use copypaste_core::Keyring;
use copypaste_ipc::{Method, ResponseData};
use copypaste_module_sdk::{ModuleOperation, ModuleOutput};
use copypaste_modules::InstalledModule;
use copypaste_runtime::Runtime;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

const BODY: &str = "Qualified snippet body";

pub(super) fn qualify(
    package: &Path,
    data: &Path,
    installed: &InstalledModule,
) -> Result<usize, String> {
    exercise(&open(&data.join("runtime"))?, package, installed)
}

fn open(directory: &Path) -> Result<Arc<Runtime>, String> {
    let keyring = Arc::new(Keyring::from_secret(&[43; 32]));
    let runtime = Arc::new(
        Runtime::open_isolated(directory, "qualification", keyring).map_err(|e| e.to_string())?,
    );
    runtime
        .attach_module_services()
        .map_err(|e| e.to_string())?;
    Ok(runtime)
}

fn exercise(
    runtime: &Arc<Runtime>,
    package: &Path,
    installed: &InstalledModule,
) -> Result<usize, String> {
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let request = |operation| {
        let response = executor.block_on(runtime.request(0, Method::Modules { operation }));
        match response.data {
            Some(ResponseData::Modules { json }) if response.ok => {
                serde_json::from_str::<Value>(&json).map_err(|e| e.to_string())
            }
            _ => Err(response
                .error
                .unwrap_or_else(|| "The runtime refused a module request.".into())),
        }
    };
    let id = installed.id.clone();
    let invoke = |command: &str, arguments: Value| -> Result<ModuleOutput, String> {
        let output = request(ModuleOperation::Invoke {
            id: id.clone(),
            command: command.into(),
            arguments_json: arguments.to_string(),
        })?;
        serde_json::from_value(output).map_err(|e| e.to_string())
    };

    let package_path = package.to_str().ok_or("The package path is not UTF-8.")?;
    request(ModuleOperation::Install {
        package_path: package_path.into(),
    })?;
    request(ModuleOperation::SetEnabled {
        id: id.clone(),
        enabled: true,
        capabilities: installed.capabilities.clone(),
    })?;
    let saved = invoke(
        "save",
        json!({"values": {"title": "Qualification", "abbreviation": "qsn", "body": BODY}}),
    )?;
    if !matches!(saved, ModuleOutput::Message { .. }) {
        return Err(format!(
            "The snippets package did not save a snippet: {saved:?}"
        ));
    }
    let ModuleOutput::Results { items } = invoke("search", json!({"query": "qsn"}))? else {
        return Err("The snippets package did not answer a Quick Paste search.".into());
    };
    let snippet = match items.as_slice() {
        [item] if item.preselect => item.id.clone(),
        _ => return Err("The saved snippet must be the one preselected match.".into()),
    };
    let ModuleOutput::Deliver { delivery } = invoke("use", json!({"id": snippet}))? else {
        return Err("The snippets package did not deliver the snippet.".into());
    };
    if delivery.text != BODY || delivery.secret {
        return Err("The delivered snippet does not match what was saved.".into());
    }
    invoke("delete", json!({"ids": [snippet]}))?;
    if !matches!(
        invoke("search", json!({"query": "qsn"}))?,
        ModuleOutput::Results { items } if items.is_empty()
    ) {
        return Err("A deleted snippet is still offered in Quick Paste.".into());
    }
    request(ModuleOperation::SetEnabled {
        id: id.clone(),
        enabled: false,
        capabilities: Vec::new(),
    })?;
    if invoke("library", json!({})).is_ok() {
        return Err("A disabled snippets module accepted commands.".into());
    }
    request(ModuleOperation::Remove { id })?;
    Ok(4)
}

#[cfg(test)]
mod tests {
    use copypaste_modules::test_support::PackageSigner;
    use std::path::Path;

    #[test]
    fn the_snippets_package_qualifies() {
        let directory = tempfile::tempdir().unwrap();
        let signer = PackageSigner::default();
        let package = directory.path().join("snippets.cpmodule");
        signer.package_module(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules/snippets"),
            &package,
        );
        let manager = copypaste_modules::ModuleManager::open(
            &directory.path().join("modules"),
            env!("CARGO_PKG_VERSION"),
            copypaste_modules::ModuleTarget::current().unwrap(),
            &signer.public_key(),
        )
        .unwrap();
        let installed = manager.install(&package).unwrap();
        let runtime = super::open(&directory.path().join("runtime")).unwrap();
        runtime.modules.trust_package_key(&signer.public_key());
        assert_eq!(super::exercise(&runtime, &package, &installed), Ok(4));
    }
}
