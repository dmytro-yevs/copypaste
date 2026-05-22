use anyhow::Result;
use std::path::Path;

pub fn run(_socket_path: &Path, file: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;

    let items = match data["items"].as_array() {
        Some(a) => a,
        None => return Err(anyhow::anyhow!("invalid format: expected {{\"items\": [...]}}"))
    };

    println!("found {} items in {}", items.len(), file);
    println!("note: full import requires daemon with decryption support (Phase 4)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, &str) -> Result<()> = run;
    }
}
