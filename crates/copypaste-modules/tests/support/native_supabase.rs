use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

pub fn library() -> &'static Path {
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
                    "--manifest-path",
                    "modules/supabase/Cargo.toml",
                    "--locked",
                    "--features",
                    "test-endpoints",
                    "--message-format=json",
                ])
                .output()
                .expect("build the independent sync package");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|event| {
                    event["reason"] == "compiler-artifact"
                        && event["target"]["name"] == "copypaste_module_supabase"
                })
                .flat_map(|event| event["filenames"].as_array().unwrap().clone())
                .filter_map(|name| name.as_str().map(PathBuf::from))
                .find(|path| {
                    matches!(
                        path.extension().and_then(|e| e.to_str()),
                        Some("dylib" | "dll" | "so")
                    )
                })
                .expect("native library artifact")
        })
        .as_path()
}
