use super::{UiBoundaryErrorCode, UiError, UpdateProgress, UpdateStatus};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{ipc::Channel, Manager as _};

const CASK: &str = "dmytro-yevs/copypaste/copypaste";
const BREW_PATHS: &[&str] = &["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];

#[cfg(any(target_os = "macos", test))]
async fn restart_after_brew_update<T, B, D, P, R>(
    brew: B,
    drain: D,
    installing: P,
    restart: R,
) -> Result<(), UiError>
where
    B: std::future::Future<Output = Result<(), UiError>>,
    D: std::future::Future<Output = Result<T, UiError>>,
    P: FnOnce(),
    R: FnOnce(T) -> Result<(), UiError>,
{
    brew.await?;
    let permit = drain.await?;
    installing();
    restart(permit)
}

pub(super) fn brew_path() -> Option<PathBuf> {
    BREW_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .map(Path::to_path_buf)
}

fn run(args: &[&str]) -> Result<std::process::Output, UiError> {
    let Some(path) = brew_path() else {
        return Err(UiError::from_boundary(
            UiBoundaryErrorCode::UpdateUnconfigured,
        ));
    };
    Command::new(path)
        .args(args)
        .output()
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))
        .and_then(|output| {
            output
                .status
                .success()
                .then_some(output)
                .ok_or_else(|| UiError::from_boundary(UiBoundaryErrorCode::UpdateNetworkFailed))
        })
}

fn parse(stdout: &[u8]) -> Result<UpdateStatus, UiError> {
    let json: serde_json::Value = serde_json::from_slice(stdout)
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))?;
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
        .ok_or_else(|| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))?;
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
    .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))?
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
    let restart = app.clone();
    let supervisor = app.state::<crate::service::Supervisor>();
    let backend = app.state::<crate::backend::SelectedBackend>();
    restart_after_brew_update(
        async {
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
            .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateInstallFailed))??;
            Ok(())
        },
        async {
            supervisor
                .install_after_update_drain(backend.inner(), |permit| permit)
                .await
                .map_err(|error| error.ui_error())
        },
        move || {
            let _ = progress.send(UpdateProgress::Installing);
        },
        move |permit| {
            let _permit = permit;
            restart.restart()
        },
    )
    .await?;
    unreachable!("a successful macOS update restarts the app")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn successful_brew_update_drains_before_announcing_and_restarting() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let brew_events = Arc::clone(&events);
        let drain_events = Arc::clone(&events);
        let restart_events = Arc::clone(&events);
        restart_after_brew_update(
            async move {
                brew_events.lock().unwrap().push("brew");
                Ok::<_, UiError>(())
            },
            async move {
                drain_events.lock().unwrap().push("drain");
                Ok::<_, UiError>(())
            },
            || events.lock().unwrap().push("installing"),
            move |_| {
                restart_events.lock().unwrap().push("restart");
                Ok(())
            },
        )
        .await
        .expect("confirmed drain restarts");

        assert_eq!(
            *events.lock().unwrap(),
            ["brew", "drain", "installing", "restart"]
        );
    }

    #[tokio::test]
    async fn failed_brew_or_refused_drain_never_announces_or_restarts() {
        let brew_events = Arc::new(Mutex::new(Vec::new()));
        let brew_result = restart_after_brew_update(
            async {
                Err::<(), _>(UiError::from_boundary(
                    UiBoundaryErrorCode::UpdateInstallFailed,
                ))
            },
            async { Ok::<_, UiError>(()) },
            || brew_events.lock().unwrap().push("installing"),
            |_| {
                brew_events.lock().unwrap().push("restart");
                Ok(())
            },
        )
        .await;
        assert!(brew_result.is_err());
        assert!(brew_events.lock().unwrap().is_empty());

        let drain_events = Arc::new(Mutex::new(Vec::new()));
        let drain_result = restart_after_brew_update(
            async { Ok::<_, UiError>(()) },
            async {
                Err::<(), _>(UiError::from_boundary(
                    UiBoundaryErrorCode::UpdateInstallFailed,
                ))
            },
            || drain_events.lock().unwrap().push("installing"),
            |_| {
                drain_events.lock().unwrap().push("restart");
                Ok(())
            },
        )
        .await;
        assert!(drain_result.is_err());
        assert!(drain_events.lock().unwrap().is_empty());
    }
}
