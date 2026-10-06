//! Explicit real-model qualification of the signed package and native ABI.
use copypaste_modules::{ModuleManager, ModuleOutput, ModuleTarget};
use minisign::KeyPair;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, ZipWriter};

#[test]
#[ignore = "requires a built OCR module and prepared offline model/runtime assets"]
fn signed_ocr_package_uses_shared_install_invoke_and_removal_contracts() {
    let source = PathBuf::from(
        std::env::var("COPYPASTE_OCR_EVIDENCE_PACKAGE").expect("prepared package root"),
    );
    let library =
        PathBuf::from(std::env::var("COPYPASTE_OCR_EVIDENCE_LIBRARY").expect("built OCR cdylib"));
    let image = std::env::var("COPYPASTE_OCR_EVIDENCE_IMAGE").expect("evidence image");
    let expected =
        std::env::var("COPYPASTE_OCR_EVIDENCE_TEXT").expect("expected complete OCR output");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(root.join("modules/ocr/module.json")).unwrap()).unwrap();
    manifest["target"] =
        serde_json::to_value(ModuleTarget::current().expect("supported native target")).unwrap();
    manifest["entrypoint"] = json!("bin/module.dylib");
    let mut files = vec![("bin/module.dylib".to_owned(), library)];
    collect_files(&source.join("assets"), &source, &mut files);
    collect_files(&source.join("bin"), &source, &mut files);
    manifest["files"] = json!(files.iter().map(|(name,path)| {
        let bytes = fs::read(path).unwrap();
        json!({"path":name,"size_bytes":bytes.len(),"sha256":hex::encode(Sha256::digest(&bytes))})
    }).collect::<Vec<_>>());
    let key = KeyPair::generate_unencrypted_keypair().unwrap();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let signature = minisign::sign(
        Some(&key.pk),
        &key.sk,
        Cursor::new(&bytes),
        Some("file:manifest.json"),
        None,
    )
    .unwrap()
    .to_string();
    let directory = tempfile::tempdir().unwrap();
    let package = directory.path().join("ocr.cpmodule");
    let mut archive = ZipWriter::new(fs::File::create(&package).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in [
        ("manifest.json", bytes.as_slice()),
        ("manifest.json.sig", signature.as_bytes()),
    ] {
        archive.start_file(name, options).unwrap();
        archive.write_all(bytes).unwrap();
    }
    for (name, path) in files {
        archive.start_file(name, options).unwrap();
        std::io::copy(&mut fs::File::open(path).unwrap(), &mut archive).unwrap();
    }
    archive.finish().unwrap();
    let data = directory.path().join("host");
    let manager = ModuleManager::open(
        &data,
        "1.0.2",
        ModuleTarget::current().unwrap(),
        &key.pk.to_base64(),
    )
    .unwrap();
    manager.install(&package).unwrap();
    let output = manager
        .invoke(
            "copypaste.ocr",
            "recognize-image",
            BTreeMap::from([("image_path".into(), json!(image))]),
        )
        .unwrap();
    assert_eq!(output, ModuleOutput::Text { text: expected });
    manager.remove("copypaste.ocr").unwrap();
    assert!(manager.list().unwrap()[0].restart_required);
}

fn collect_files(directory: &Path, root: &Path, output: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files(&path, root, output);
        } else {
            output.push((
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                path,
            ));
        }
    }
}
