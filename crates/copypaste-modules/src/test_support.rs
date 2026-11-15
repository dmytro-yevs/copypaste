//! Module packages for hosts under test: a module built from source and
//! signed with a developer key the host trusts through
//! `ModuleHost::trust_package_key` or `ModuleManager::open`.
use copypaste_module_sdk::ModuleTarget;
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub struct PackageSigner {
    key: minisign::KeyPair,
}

impl Default for PackageSigner {
    fn default() -> Self {
        Self {
            key: minisign::KeyPair::generate_unencrypted_keypair()
                .expect("generate a package signing key"),
        }
    }
}

impl PackageSigner {
    pub fn public_key(&self) -> String {
        self.key.pk.to_base64()
    }

    /// Writes a signed package of `library` at `path`. The manifest's
    /// `files` entry for its `entrypoint` is filled in here.
    pub fn package(&self, mut manifest: serde_json::Value, library: &[u8], path: &Path) {
        let entrypoint = manifest["entrypoint"]
            .as_str()
            .expect("the manifest names its entrypoint")
            .to_owned();
        manifest["files"] = serde_json::json!([{
            "path": entrypoint,
            "sha256": hex::encode(Sha256::digest(library)),
            "size_bytes": library.len(),
        }]);
        let bytes = serde_json::to_vec(&manifest).expect("encode the manifest");
        let signature = minisign::sign(
            Some(&self.key.pk),
            &self.key.sk,
            Cursor::new(&bytes),
            Some("file:manifest.json"),
            None,
        )
        .expect("sign the manifest")
        .to_string();
        let mut archive =
            zip::ZipWriter::new(std::fs::File::create(path).expect("create the package"));
        for (name, content) in [
            ("manifest.json", bytes.as_slice()),
            ("manifest.json.sig", signature.as_bytes()),
            (entrypoint.as_str(), library),
        ] {
            archive
                .start_file(name, zip::write::SimpleFileOptions::default())
                .expect("add a package file");
            archive.write_all(content).expect("write a package file");
        }
        archive.finish().expect("finish the package");
    }

    /// Builds the module crate at `directory` (its `Cargo.toml` and
    /// `module.json`) and packages it for this machine.
    pub fn package_module(&self, directory: &Path, path: &Path) {
        let target = ModuleTarget::current().expect("modules run on this platform");
        let mut manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("module.json")).expect("read the module manifest"),
        )
        .expect("decode the module manifest");
        manifest["target"] = serde_json::to_value(target).expect("encode the target");
        manifest["entrypoint"] =
            format!("bin/module.{}", target.platform.dynamic_library_extension()).into();
        let library = std::fs::read(build_library(&directory.join("Cargo.toml")))
            .expect("read the module library");
        self.package(manifest, &library, path);
    }
}

/// Builds a module crate's native library like its release job does, into
/// the caller's target directory, and returns its path.
pub fn build_library(manifest: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--locked",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .expect("build the module");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = std::fs::canonicalize(manifest).expect("the module manifest exists");
    String::from_utf8(output.stdout)
        .expect("cargo reports UTF-8")
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-artifact"
                && message["manifest_path"].as_str().map(Path::new) == Some(manifest.as_path())
        })
        .flat_map(|message| message["filenames"].as_array().cloned().unwrap_or_default())
        .filter_map(|path| path.as_str().map(PathBuf::from))
        .find(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("so" | "dll" | "dylib")
            )
        })
        .expect("the module is a native dynamic library")
}
