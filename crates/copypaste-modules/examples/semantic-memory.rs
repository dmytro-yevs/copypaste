//! Isolated macOS evidence using installed signed assets and a candidate worker.
use copypaste_module_sdk::validate_embeddings;
use copypaste_modules::{
    DesktopInferenceLauncher, ModuleManager, ModuleOutput, ModuleTarget, MODULE_RELEASE_PUBLIC_KEY,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

fn copy_tree(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let output = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &output)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), output)?;
        } else {
            return Err(std::io::Error::other("Unexpected fixture asset type"));
        }
    }
    Ok(())
}

fn embed(manager: &ModuleManager, role: &str, text: &str) -> Vec<f32> {
    let output = manager
        .invoke(
            "copypaste.semantic-search",
            "embed-text",
            BTreeMap::from([("role".into(), role.into()), ("text".into(), text.into())]),
        )
        .expect("candidate worker must return embeddings");
    let ModuleOutput::Embeddings { vectors, .. } = output else {
        panic!("embedding output required")
    };
    assert_eq!(vectors.len(), 1);
    assert!(validate_embeddings(&vectors, 384));
    vectors.into_iter().next().unwrap()
}

fn similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn children(worker: &Path) -> Vec<u32> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,comm="])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let parent: u32 = fields.next()?.parse().ok()?;
            let name = fields.collect::<Vec<_>>().join(" ");
            (parent == std::process::id() && Path::new(&name) == worker).then_some(pid)
        })
        .collect()
}

fn measure(pid: u32, directory: &Path, stage: &str) -> String {
    let output = Command::new("/usr/bin/vmmap")
        .args(["-summary", &pid.to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    fs::write(directory.join(format!("{stage}-{pid}-vmmap.txt")), &text).unwrap();
    text.lines()
        .find_map(|line| line.strip_prefix("Physical footprint:"))
        .unwrap()
        .trim()
        .to_owned()
}

fn stage(name: &str, worker: &Path, receipt: &Path, rows: &mut Vec<Value>) {
    let directory = receipt.parent().unwrap();
    let pids = children(worker);
    let parent = measure(std::process::id(), directory, &format!("{name}-parent"));
    let workers: Vec<_> = pids
        .iter()
        .map(|pid| json!({"pid":pid,"footprint":measure(*pid,directory,name)}))
        .collect();
    let row = json!({"stage":name,"parent_footprint":parent,"workers":workers});
    println!("{row}");
    rows.push(row);
    fs::write(receipt, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

fn main() {
    if std::env::consts::OS != "macos" {
        panic!("This maintained memory scenario requires macOS vmmap");
    }
    let arguments: Vec<_> = std::env::args_os().collect();
    assert_eq!(
        arguments.len(),
        5,
        "semantic-memory <signed-package-dir> <semantic-data-dir> <candidate-worker> <receipt>"
    );
    let package = Path::new(&arguments[1]);
    let data = Path::new(&arguments[2]);
    let worker = fs::canonicalize(&arguments[3]).unwrap();
    let receipt = PathBuf::from(&arguments[4]);
    fs::create_dir_all(receipt.parent().unwrap()).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(package.join("manifest.json")).unwrap()).unwrap();
    let version = manifest["version"].as_str().unwrap();
    copy_tree(
        package,
        &fixture
            .path()
            .join("packages/copypaste.semantic-search")
            .join(version),
    )
    .unwrap();
    copy_tree(data, &fixture.path().join("data/copypaste.semantic-search")).unwrap();
    fs::write(fixture.path().join("registry.json"), serde_json::to_vec(&json!({
        "schema_version":1,"modules":{"copypaste.semantic-search":{"version":version,"enabled":true,"preferences":{"languages":["uk","en"]}}}
    })).unwrap()).unwrap();
    let manager = ModuleManager::open(
        fixture.path(),
        env!("CARGO_PKG_VERSION"),
        ModuleTarget::current().unwrap(),
        MODULE_RELEASE_PUBLIC_KEY,
    )
    .unwrap();
    manager.set_inference_launcher(Arc::new(DesktopInferenceLauncher::new(worker.clone())));
    let mut rows = Vec::new();
    stage("cold", &worker, &receipt, &mut rows);
    let original = embed(&manager, "query", "оплата житла");
    let relevant = embed(&manager, "passage", "Apartment lease rent for October");
    let unrelated = embed(&manager, "passage", "Chocolate cake recipe with walnuts");
    assert!(similarity(&original, &relevant) >= 0.8);
    assert!(similarity(&original, &unrelated) < 0.8);
    stage("loaded", &worker, &receipt, &mut rows);
    assert_eq!(children(&worker).len(), 1);
    // The production idle timer runs independently of index/UI polling.
    let started = Instant::now();
    while !children(&worker).is_empty() {
        assert!(
            started.elapsed() < Duration::from_secs(65),
            "idle worker must exit after 60 seconds"
        );
        std::thread::sleep(Duration::from_secs(1));
    }
    stage("idle", &worker, &receipt, &mut rows);
    let reloaded = embed(&manager, "query", "оплата житла");
    assert!(original
        .iter()
        .zip(&reloaded)
        .all(|(a, b)| (a - b).abs() < 0.00001));
    stage("reloaded", &worker, &receipt, &mut rows);
    manager
        .set_enabled("copypaste.semantic-search", false)
        .unwrap();
    assert!(children(&worker).is_empty());
    stage("disabled", &worker, &receipt, &mut rows);
    manager
        .set_enabled("copypaste.semantic-search", true)
        .unwrap();
    let final_vector = embed(&manager, "query", "оплата житла");
    assert!(original
        .iter()
        .zip(&final_vector)
        .all(|(a, b)| (a - b).abs() < 0.00001));
    stage("reenabled", &worker, &receipt, &mut rows);
    drop(manager);
    assert!(children(&worker).is_empty());
    stage("shutdown", &worker, &receipt, &mut rows);
}
