use copypaste_module_sdk::{ModuleArchitecture, ModuleOutput, ModulePlatform, ModuleTarget};
use copypaste_modules::{ModuleError, ModuleManager};
use minisign::KeyPair;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};
use zip::{write::SimpleFileOptions, ZipWriter};

fn target() -> ModuleTarget {
    ModuleTarget::current().unwrap_or(ModuleTarget {
        platform: ModulePlatform::Macos,
        architecture: ModuleArchitecture::X86_64,
    })
}

struct Fixture {
    directory: tempfile::TempDir,
    key: KeyPair,
}
impl Fixture {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
            key: KeyPair::generate_unencrypted_keypair().unwrap(),
        }
    }
    fn manager(&self) -> ModuleManager {
        ModuleManager::open(
            &self.directory.path().join("modules"),
            "1.0.2",
            target(),
            &self.key.pk.to_base64(),
        )
        .unwrap()
    }
    fn package(
        &self,
        version: &str,
        library: &[u8],
        mutate: impl FnOnce(&mut serde_json::Value),
        extra: Option<(&str, &[u8])>,
    ) -> PathBuf {
        let mut manifest = json!({
            "schema_version": 1, "api_version": 1, "id": "copypaste.text-tools",
            "title": "Text Tools", "description": "Transform text.", "version": version,
            "app_versions": ">=1.0.0, <2.0.0", "target": target(), "entrypoint": "bin/module",
            "files": [{"path":"bin/module", "sha256":hex::encode(Sha256::digest(library)), "size_bytes":library.len()}],
            "commands": [{"id":"transform", "title":"Transform text", "description":"Transform text.",
                "arguments":[{"id":"text", "title":"Text", "kind":"text", "default":"", "required":true}]}],
            "preferences": [{"id":"uppercase", "title":"Uppercase", "kind":"boolean", "default":false}]
        });
        mutate(&mut manifest);
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let signature = minisign::sign(
            Some(&self.key.pk),
            &self.key.sk,
            Cursor::new(&bytes),
            Some("file:manifest.json"),
            None,
        )
        .unwrap()
        .to_string();
        let path = self.directory.path().join(format!(
            "{version}-{}.cpmodule",
            fs::read_dir(self.directory.path()).unwrap().count()
        ));
        let mut archive = ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, bytes) in [
            ("manifest.json", bytes.as_slice()),
            ("manifest.json.sig", signature.as_bytes()),
            ("bin/module", library),
        ] {
            archive
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        if let Some((name, bytes)) = extra {
            archive
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
        path
    }
}

fn native_library() -> &'static Path {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY
        .get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            let output = Command::new(env!("CARGO"))
                .current_dir(root)
                .args([
                    "build",
                    "--locked",
                    "-p",
                    "copypaste-module-text-tools",
                    "--message-format=json",
                ])
                .output()
                .expect("build the separately linked native module");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .filter(|v| {
                    v["reason"] == "compiler-artifact"
                        && v["target"]["name"] == "copypaste_module_text_tools"
                })
                .flat_map(|v| v["filenames"].as_array().cloned().unwrap_or_default())
                .filter_map(|v| v.as_str().map(PathBuf::from))
                .find(|p| {
                    matches!(
                        p.extension().and_then(|v| v.to_str()),
                        Some("so" | "dll" | "dylib")
                    )
                })
                .expect("the fixture must be a native dynamic library")
        })
        .as_path()
}

#[test]
fn signed_native_module_runs_offline_and_survives_restart_update_disable_and_removal() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    assert!(manager.list().unwrap().is_empty());
    let library = fs::read(native_library()).unwrap();
    let package = fixture.package("1.0.0", &library, |_| {}, None);
    let installed = manager.install(&package).unwrap();
    assert!(installed.enabled);
    let input = BTreeMap::from([("text".into(), json!("  Україна ї ґ є і  "))]);
    assert_eq!(
        manager
            .invoke(&installed.id, "transform", input.clone())
            .unwrap(),
        ModuleOutput::Text {
            text: "Україна ї ґ є і".into()
        }
    );
    manager
        .set_preferences(
            &installed.id,
            BTreeMap::from([("uppercase".into(), json!(true))]),
        )
        .unwrap();
    assert_eq!(
        manager
            .invoke(&installed.id, "transform", input.clone())
            .unwrap(),
        ModuleOutput::Text {
            text: "  УКРАЇНА Ї Ґ Є І  ".into()
        }
    );
    manager.set_enabled(&installed.id, false).unwrap();
    assert!(matches!(
        manager.invoke(&installed.id, "transform", input.clone()),
        Err(ModuleError::Disabled)
    ));
    drop(manager);
    let manager = fixture.manager();
    assert!(!manager.list().unwrap()[0].enabled);
    let package = fixture.package("1.1.0", &library, |_| {}, None);
    manager.install(&package).unwrap();
    let updated = &manager.list().unwrap()[0];
    assert_eq!(updated.version, "1.1.0");
    assert!(!updated.enabled);
    assert_eq!(updated.preferences["uppercase"], json!(true));
    manager.set_enabled(&installed.id, true).unwrap();
    manager.invoke(&installed.id, "transform", input).unwrap();
    let data = fixture
        .directory
        .path()
        .join("modules/data/copypaste.text-tools");
    fs::write(data.join("cache"), "private data").unwrap();
    manager.remove(&installed.id).unwrap();
    assert!(manager.list().unwrap().is_empty());
    assert!(!data.exists());
    assert!(!fixture
        .directory
        .path()
        .join("modules/packages/copypaste.text-tools")
        .exists());
    drop(manager);
    assert!(fixture.manager().list().unwrap().is_empty());
}

#[test]
fn unsigned_wrong_target_and_tampered_packages_cannot_replace_an_installed_version() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    manager
        .install(&fixture.package("1.0.0", b"fixture", |_| {}, None))
        .unwrap();
    let other = Fixture::new();
    assert!(manager
        .install(&other.package("1.1.0", b"fixture", |_| {}, None))
        .is_err());
    assert!(manager
        .install(&fixture.package(
            "1.1.0",
            b"fixture",
            |m| m["target"]["architecture"] = json!("arm"),
            None
        ))
        .is_err());
    assert!(manager
        .install(&fixture.package(
            "1.1.0",
            b"fixture",
            |m| m["files"][0]["sha256"] = json!("0".repeat(64)),
            None
        ))
        .is_err());
    assert!(manager
        .install(&fixture.package("1.0.0", b"fixture", |_| {}, None))
        .is_err());
    assert_eq!(manager.list().unwrap()[0].version, "1.0.0");
}

#[test]
fn archive_inventory_and_path_escape_are_rejected_before_activation() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    for path in ["rogue.bin", "../outside", "bin/../../outside"] {
        let package = fixture.package("1.0.0", b"fixture", |_| {}, Some((path, b"rogue")));
        assert!(manager.install(&package).is_err(), "{path}");
    }
    assert!(manager.list().unwrap().is_empty());
    assert!(!fixture.directory.path().join("outside").exists());
}

#[test]
fn unsupported_api_preferences_and_commands_are_rejected() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    assert!(manager
        .install(&fixture.package("1.0.0", b"fixture", |m| m["api_version"] = json!(99), None))
        .is_err());
    assert!(manager
        .install(&fixture.package(
            "1.0.0",
            b"fixture",
            |m| m["commands"][0]["arguments"].as_array_mut().unwrap().push(
                json!({"id":"text","title":"Duplicate","kind":"text","default":"","required":true})
            ),
            None
        ))
        .is_err());
    let module = manager
        .install(&fixture.package("1.0.0", b"fixture", |_| {}, None))
        .unwrap();
    assert!(manager
        .set_preferences(
            &module.id,
            BTreeMap::from([("uppercase".into(), json!("wrong type"))])
        )
        .is_err());
    assert!(manager
        .set_preferences(
            &module.id,
            BTreeMap::from([("unknown".into(), json!(true))])
        )
        .is_err());
    assert!(manager
        .invoke(&module.id, "unknown", BTreeMap::new())
        .is_err());
    assert!(manager
        .invoke(&module.id, "transform", BTreeMap::new())
        .is_err());
}

#[test]
fn installed_code_tampering_cannot_execute() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    let module = manager
        .install(&fixture.package("1.0.0", b"fixture", |_| {}, None))
        .unwrap();
    fs::write(
        fixture
            .directory
            .path()
            .join("modules/packages/copypaste.text-tools/1.0.0/bin/module"),
        "tampered",
    )
    .unwrap();
    assert!(manager
        .invoke(
            &module.id,
            "transform",
            BTreeMap::from([("text".into(), json!("text"))])
        )
        .is_err());
    manager.remove(&module.id).unwrap();
    assert!(manager.list().unwrap().is_empty());
}

#[test]
fn corrupt_registry_fails_closed_and_does_not_delete_packages() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    manager
        .install(&fixture.package("1.0.0", b"fixture", |_| {}, None))
        .unwrap();
    drop(manager);
    fs::write(
        fixture.directory.path().join("modules/registry.json"),
        "corrupt",
    )
    .unwrap();
    assert!(ModuleManager::open(
        &fixture.directory.path().join("modules"),
        "1.0.2",
        target(),
        &fixture.key.pk.to_base64()
    )
    .is_err());
    assert!(fixture
        .directory
        .path()
        .join("modules/packages/copypaste.text-tools/1.0.0")
        .exists());
}

#[test]
fn interrupted_activation_reuses_an_authenticated_unreferenced_version() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    let package = fixture.package("1.0.0", b"fixture", |_| {}, None);
    manager.install(&package).unwrap();
    drop(manager);
    // Model a crash after package rename but before the active registry commit.
    fs::write(
        fixture.directory.path().join("modules/registry.json"),
        br#"{"schema_version":1,"modules":{}}"#,
    )
    .unwrap();
    let manager = fixture.manager();
    manager.install(&package).unwrap();
    assert_eq!(manager.list().unwrap()[0].version, "1.0.0");
}

#[test]
fn interrupted_removal_stays_visible_and_can_finish_after_restart() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    let module = manager
        .install(&fixture.package("1.0.0", b"fixture", |_| {}, None))
        .unwrap();
    drop(manager);
    let path = fixture.directory.path().join("modules/registry.json");
    let mut registry: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    registry["modules"][&module.id]["removing"] = json!(true);
    registry["modules"][&module.id]["enabled"] = json!(false);
    fs::write(&path, serde_json::to_vec(&registry).unwrap()).unwrap();
    fs::remove_dir_all(
        fixture
            .directory
            .path()
            .join("modules/packages")
            .join(&module.id),
    )
    .unwrap();
    let manager = fixture.manager();
    assert!(manager.list().unwrap().is_empty());
}

#[test]
fn invalid_preferences_are_recoverable_but_incomplete_required_preferences_are_editable() {
    let fixture = Fixture::new();
    let manager = fixture.manager();
    let module = manager
        .install(&fixture.package("1.0.0", b"fixture", |_| {}, None))
        .unwrap();
    drop(manager);
    let path = fixture.directory.path().join("modules/registry.json");
    let mut registry: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    registry["modules"][&module.id]["preferences"]["uppercase"] = json!("corrupt");
    fs::write(path, serde_json::to_vec(&registry).unwrap()).unwrap();
    let manager = fixture.manager();
    assert!(manager.list().unwrap()[0].error.is_some());
    manager
        .set_preferences(
            &module.id,
            BTreeMap::from([("uppercase".into(), json!(true))]),
        )
        .unwrap();
    assert!(manager.list().unwrap()[0].error.is_none());
    let package = fixture.package("1.1.0", b"fixture", |manifest| {
        manifest["preferences"] = json!([{"id":"name", "title":"Name", "kind":"text", "required":true, "default":""}]);
    }, None);
    manager.install(&package).unwrap();
    let entry = manager.list().unwrap().remove(0);
    assert!(entry.error.is_none());
    assert_eq!(entry.preference_fields.len(), 1);
    assert!(manager
        .invoke(
            &entry.id,
            "transform",
            BTreeMap::from([("text".into(), json!("text"))])
        )
        .is_err());
    manager
        .set_preferences(
            &entry.id,
            BTreeMap::from([("name".into(), json!("Configured"))]),
        )
        .unwrap();
}

#[test]
fn manifest_paths_are_portable_across_supported_platforms() {
    for path in [
        "/absolute",
        "../parent",
        "C:/path",
        "bin\\module",
        "assets/CON",
        "bin/trailing.",
        "assets/file?",
    ] {
        assert!(!copypaste_module_sdk::valid_relative_path(path), "{path}");
    }
    assert!(copypaste_module_sdk::valid_relative_path(
        "assets/model.onnx"
    ));
}

#[test]
fn process_scoped_module_removal_finishes_after_its_host_process_exits() {
    if let Ok(root) = std::env::var("COPYPASTE_PIN_TEST_ROOT") {
        let manager = ModuleManager::open(
            Path::new(&root),
            "1.0.2",
            target(),
            &std::env::var("COPYPASTE_PIN_TEST_KEY").unwrap(),
        )
        .unwrap();
        let module = manager
            .install(Path::new(
                &std::env::var("COPYPASTE_PIN_TEST_PACKAGE").unwrap(),
            ))
            .unwrap();
        manager
            .invoke(
                &module.id,
                "transform",
                BTreeMap::from([("text".into(), json!("text"))]),
            )
            .unwrap();
        manager.remove(&module.id).unwrap();
        let modules = manager.list().unwrap();
        assert!(modules[0].restart_required);
        assert!(!modules[0].enabled);
        assert!(matches!(
            manager.invoke(
                &module.id,
                "transform",
                BTreeMap::from([("text".into(), json!("text"))])
            ),
            Err(ModuleError::Disabled)
        ));
        return;
    }
    let fixture = Fixture::new();
    let library = fs::read(native_library()).unwrap();
    let package = fixture.package(
        "1.0.0",
        &library,
        |manifest| manifest["unload_policy"] = json!("process"),
        None,
    );
    let root = fixture.directory.path().join("modules");
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process_scoped_module_removal_finishes_after_its_host_process_exits",
            "--nocapture",
        ])
        .env("COPYPASTE_PIN_TEST_ROOT", &root)
        .env("COPYPASTE_PIN_TEST_KEY", fixture.key.pk.to_base64())
        .env("COPYPASTE_PIN_TEST_PACKAGE", package)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    let manager = fixture.manager();
    assert!(manager.list().unwrap().is_empty());
    assert!(!root.join("packages/copypaste.text-tools").exists());
    assert!(!root.join("data/copypaste.text-tools").exists());
}
