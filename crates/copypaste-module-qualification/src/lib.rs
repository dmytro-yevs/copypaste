//! Native qualification of production-signed packages in an isolated host.
#![deny(unsafe_code)]

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod android;
mod semantic;
mod supabase;

use copypaste_modules::{ModuleManager, ModuleOutput, ModuleTarget, MODULE_RELEASE_PUBLIC_KEY};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

#[derive(Deserialize)]
struct Fixture {
    file: String,
    expected: String,
}

#[derive(Serialize)]
pub struct Receipt {
    pub schema_version: u32,
    pub commit: String,
    pub run_id: String,
    pub target: ModuleTarget,
    pub module_id: String,
    pub module_version: String,
    pub app_version: String,
    pub package_sha256: String,
    pub package_size_bytes: u64,
    pub cases_passed: usize,
    pub signature_verified: bool,
    pub restart_required: bool,
}

pub fn qualify(
    package: &Path,
    fixtures: &Path,
    data: &Path,
    app_version: &str,
    commit: &str,
    run_id: &str,
) -> Result<Receipt, String> {
    if commit.len() != 40
        || !commit.bytes().all(|value| value.is_ascii_hexdigit())
        || run_id.is_empty()
        || !run_id.bytes().all(|value| value.is_ascii_digit())
    {
        return Err("Qualification must identify its exact commit and workflow run.".into());
    }
    let target = ModuleTarget::current().ok_or("Unsupported native qualification target.")?;
    let manager = ModuleManager::open(data, app_version, target, MODULE_RELEASE_PUBLIC_KEY)
        .map_err(|error| error.to_string())?;
    let installed = manager
        .install(package)
        .map_err(|error| error.to_string())?;
    let cases_passed = match installed.id.as_str() {
        "copypaste.ocr" => qualify_ocr(&manager, fixtures, &installed.id)?,
        "copypaste.semantic-search" => semantic::qualify(&manager, fixtures, data, &installed.id)?,
        "copypaste.supabase" => supabase::qualify(&manager, &installed.id)?,
        _ => return Err("No native qualification scenario exists for this module.".into()),
    };
    manager
        .remove(&installed.id)
        .map_err(|error| error.to_string())?;
    let pending = manager.list().map_err(|error| error.to_string())?;
    let restart_required = installed.id != "copypaste.supabase";
    if restart_required {
        if pending.len() != 1 || !pending[0].restart_required || pending[0].enabled {
            return Err(
                "Process-scoped module removal must disable it and require restart.".into(),
            );
        }
    } else if !pending.is_empty()
        || data.join("packages").join(&installed.id).exists()
        || data.join("data").join(&installed.id).exists()
        || manager
            .invoke(&installed.id, "status", BTreeMap::new())
            .is_ok()
    {
        return Err("Instance-scoped sync module removal must finish immediately.".into());
    }
    let mut source = fs::File::open(package).map_err(|error| error.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(Receipt {
        schema_version: 1,
        commit: commit.into(),
        run_id: run_id.into(),
        target,
        module_id: installed.id,
        module_version: installed.version,
        app_version: app_version.into(),
        package_sha256: hex::encode(hash.finalize()),
        package_size_bytes: fs::metadata(package)
            .map_err(|error| error.to_string())?
            .len(),
        cases_passed,
        signature_verified: true,
        restart_required,
    })
}

fn qualify_ocr(manager: &ModuleManager, fixtures: &Path, id: &str) -> Result<usize, String> {
    manager
        .set_enabled(id, false)
        .map_err(|error| error.to_string())?;
    manager
        .set_enabled(id, true)
        .map_err(|error| error.to_string())?;
    let cases: Vec<Fixture> = serde_json::from_slice(
        &fs::read(fixtures.join("fixtures.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if cases.len() != 3 {
        return Err(
            "English, separate Ukrainian/English, and mixed-line fixtures are required.".into(),
        );
    }
    for case in &cases {
        if case.file.contains('/') || case.file.contains('\\') || case.expected.is_empty() {
            return Err("Invalid qualification fixture.".into());
        }
        let image =
            fs::canonicalize(fixtures.join(&case.file)).map_err(|error| error.to_string())?;
        let output = manager
            .invoke(
                id,
                "recognize-image",
                BTreeMap::from([("image_path".into(), json!(image.to_string_lossy()))]),
            )
            .map_err(|error| error.to_string())?;
        let ModuleOutput::Text { text } = output else {
            return Err("The OCR package did not return text.".into());
        };
        if !text.contains(&case.expected) {
            return Err(format!(
                "OCR fixture {} did not contain {:?}; output: {:?}",
                case.file, case.expected, text
            ));
        }
    }
    Ok(cases.len())
}

pub fn finish_after_restart(data: &Path, app_version: &str) -> Result<(), String> {
    let target = ModuleTarget::current().ok_or("Unsupported native qualification target.")?;
    let manager = ModuleManager::open(data, app_version, target, MODULE_RELEASE_PUBLIC_KEY)
        .map_err(|error| error.to_string())?;
    if !manager
        .list()
        .map_err(|error| error.to_string())?
        .is_empty()
    {
        return Err("Module removal did not finish after process restart.".into());
    }
    Ok(())
}
