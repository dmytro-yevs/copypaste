use super::{UiError, UpdateProgress, UpdateStatus};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::ipc::Channel;

const CASK: &str = "dmytro-yevs/copypaste/copypaste";
const BREW_PATHS: &[&str] = &["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];

pub(super) fn brew_path() -> Option<PathBuf> {
    BREW_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .map(Path::to_path_buf)
}

fn run(args: &[&str]) -> Result<std::process::Output, UiError> {
    let Some(path) = brew_path() else {
        return Err(UiError::new("update_unconfigured", false));
    };
    Command::new(path)
        .args(args)
        .output()
        .map_err(|_| UiError::new("update_check_failed", true))
        .and_then(|output| {
            output
                .status
                .success()
                .then_some(output)
                .ok_or_else(|| UiError::new("update_network_failed", true))
        })
}

fn parse(stdout: &[u8]) -> Result<UpdateStatus, UiError> {
    let json: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|_| UiError::new("update_check_failed", true))?;
    let Some(entry) = json
        .get("casks")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
    else {
        return Ok(UpdateStatus::UpToDate);
    };
    let version = entry
        .get("version")
        .or_else(|| entry.get("current_version"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| UiError::new("update_check_failed", true))?;
    Ok(UpdateStatus::Available {
        version: version.to_owned(),
    })
}

pub(super) async fn check() -> Result<UpdateStatus, UiError> {
    tokio::task::spawn_blocking(|| {
        run(&["update-if-needed"])?;
        parse(&run(&["outdated", "--cask", "--json=v2", CASK])?.stdout)
    })
    .await
    .map_err(|_| UiError::new("update_check_failed", true))?
}

pub(super) async fn install(
    app: tauri::AppHandle,
    expected: String,
    progress: Channel<UpdateProgress>,
) -> Result<UpdateStatus, UiError> {
    let status = check().await?;
    let UpdateStatus::Available { version } = status else {
        return Ok(status);
    };
    if version != expected {
        return Ok(UpdateStatus::Available { version });
    }
    tokio::task::spawn_blocking(|| {
        run(&[
            "upgrade",
            "--cask",
            "--no-ask",
            "--no-quit",
            "--require-sha",
            CASK,
        ])
    })
    .await
    .map_err(|_| UiError::new("update_install_failed", false))??;
    let _ = progress.send(UpdateProgress::Installing);
    app.restart()
}
