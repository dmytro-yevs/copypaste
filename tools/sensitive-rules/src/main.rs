mod model;
mod render;

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use model::{load_inputs, read_selection, sha256};

#[tokio::main]
async fn main() -> Result<()> {
    let root = workspace_root();
    match std::env::args().nth(1).as_deref() {
        Some("update") => update(&root).await,
        Some("generate") => generate(&root),
        Some("check") => check(&root),
        _ => bail!("usage: cargo run -p copypaste-sensitive-rules -- <update|generate|check>"),
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tool lives at tools/sensitive-rules")
        .to_path_buf()
}

async fn update(root: &Path) -> Result<()> {
    let selection = read_selection(root)?;
    let config = download(&selection.source.config_url).await?;
    let license = download(&selection.source.license_url).await?;
    verify_bytes("config", &config, &selection.source.config_sha256)?;
    verify_bytes("license", &license, &selection.source.license_sha256)?;
    write_atomic(&root.join(&selection.source.vendored_config), &config)?;
    write_atomic(&root.join(&selection.source.vendored_license), &license)?;
    println!("updated Gitleaks {} snapshot", selection.source.release);
    Ok(())
}

async fn download(url: &str) -> Result<Vec<u8>> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("download {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?;
    Ok(response.bytes().await?.to_vec())
}

fn generate(root: &Path) -> Result<()> {
    let inputs = load_inputs(root)?;
    let output = render_generated(&inputs)?;
    write_atomic(
        &root.join(&inputs.selection.source.generated_rust),
        output.as_bytes(),
    )?;
    println!(
        "generated {} from Gitleaks {}",
        inputs.selection.source.generated_rust, inputs.selection.source.release
    );
    Ok(())
}

fn check(root: &Path) -> Result<()> {
    let inputs = load_inputs(root)?;
    let expected = render_generated(&inputs)?;
    let path = root.join(&inputs.selection.source.generated_rust);
    let actual = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    if actual != expected {
        bail!(
            "{} is stale; run cargo run -p copypaste-sensitive-rules -- generate",
            path.display()
        );
    }
    println!("Gitleaks snapshot checksum and generated rules are current");
    Ok(())
}

fn render_generated(inputs: &model::Inputs) -> Result<String> {
    let rendered = render::render(inputs)?;
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2021", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("start rustfmt")?;
    child
        .stdin
        .as_mut()
        .context("open rustfmt stdin")?
        .write_all(rendered.as_bytes())
        .context("write generated Rust to rustfmt")?;
    let output = child.wait_with_output().context("wait for rustfmt")?;
    if !output.status.success() {
        bail!("rustfmt rejected generated sensitive rules");
    }
    String::from_utf8(output.stdout).context("rustfmt returned non-UTF-8 output")
}

fn verify_bytes(label: &str, bytes: &[u8], expected: &str) -> Result<()> {
    let actual = sha256(bytes);
    if actual != expected {
        bail!("{label} SHA-256 mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("output path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes).with_context(|| format!("write {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_snapshot_and_generated_module_are_current() {
        let root = workspace_root();
        check(&root).unwrap();
        let inputs = load_inputs(&root).unwrap();
        assert!(inputs.global_allowlists.is_empty());
        assert_eq!(inputs.selected_ids.len(), 27);
        assert_eq!(inputs.rules.len(), 48);
    }

    #[test]
    fn regeneration_is_deterministic() {
        let inputs = load_inputs(&workspace_root()).unwrap();
        assert_eq!(
            render_generated(&inputs).unwrap(),
            render_generated(&inputs).unwrap()
        );
    }
}
