use super::{UiBoundaryErrorCode, UiError, UpdateProgress, UpdateStatus};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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

fn brew_path() -> Option<PathBuf> {
    BREW_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .map(Path::to_path_buf)
}

fn parse_installed_casks(output: &Output) -> Result<UpdateStatus, UiError> {
    if !output.status.success() {
        return Err(UiError::from_boundary(
            UiBoundaryErrorCode::UpdateCheckFailed,
        ));
    }
    let names = std::str::from_utf8(&output.stdout)
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))?;
    if names.lines().any(|name| name == CASK) {
        Ok(UpdateStatus::Ready)
    } else {
        Ok(UpdateStatus::Unconfigured)
    }
}

fn probe_status_with(
    path: Option<&Path>,
    list: impl FnOnce(&Path) -> std::io::Result<Output>,
) -> Result<UpdateStatus, UiError> {
    let Some(path) = path else {
        return Ok(UpdateStatus::Unconfigured);
    };
    let output =
        list(path).map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))?;
    parse_installed_casks(&output)
}

fn probe_status() -> Result<UpdateStatus, UiError> {
    let path = brew_path();
    probe_status_with(path.as_deref(), |path| {
        Command::new(path)
            .args(["list", "--cask", "--full-name", "-1"])
            .output()
    })
}

pub(super) async fn status() -> Result<UpdateStatus, UiError> {
    tokio::task::spawn_blocking(probe_status)
        .await
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))?
}

fn execute(args: &[&str]) -> Result<Output, UiError> {
    let Some(path) = brew_path() else {
        return Err(UiError::from_boundary(
            UiBoundaryErrorCode::UpdateUnconfigured,
        ));
    };
    Command::new(path)
        .args(args)
        .output()
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))
}

fn run(args: &[&str]) -> Result<Output, UiError> {
    let output = execute(args)?;
    output
        .status
        .success()
        .then_some(output)
        .ok_or_else(|| UiError::from_boundary(UiBoundaryErrorCode::UpdateNetworkFailed))
}

fn parse_outdated(output: &Output) -> Result<UpdateStatus, UiError> {
    let check_failed = || UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateCheckFailed))?;
    let casks = json
        .get("casks")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(check_failed)?;
    if !json
        .get("formulae")
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        return Err(check_failed());
    }
    if output.status.success() && casks.is_empty() {
        return Ok(UpdateStatus::UpToDate);
    }
    // Named `brew outdated` exits 1 when its JSON contains the outdated cask.
    if output.status.code() != Some(1) || casks.len() != 1 {
        return Err(check_failed());
    }
    let entry = &casks[0];
    if entry.get("name").and_then(serde_json::Value::as_str) != Some("copypaste") {
        return Err(check_failed());
    }
    if !entry
        .get("installed_versions")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|versions| {
            versions.len() == 1
                && versions[0]
                    .as_str()
                    .is_some_and(|version| !version.trim().is_empty())
        })
    {
        return Err(check_failed());
    }
    let version = entry
        .get("current_version")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(check_failed)?;
    Ok(UpdateStatus::Available {
        version: version.to_owned(),
    })
}

pub(super) async fn check() -> Result<UpdateStatus, UiError> {
    tokio::task::spawn_blocking(|| {
        if probe_status()? == UpdateStatus::Unconfigured {
            return Ok(UpdateStatus::Unconfigured);
        }
        run(&["update-if-needed"])?;
        parse_outdated(&execute(&["outdated", "--cask", "--json=v2", CASK])?)
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
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::sync::{Arc, Mutex};

    fn output(code: i32, stdout: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn brew_probe_distinguishes_missing_brew_unmanaged_cask_and_failure() {
        assert_eq!(
            probe_status_with(None, |_| panic!("missing brew must not run a process")),
            Ok(UpdateStatus::Unconfigured)
        );
        let path = Path::new("/test/brew");
        let installed = output(0, "other/tap/other\ndmytro-yevs/copypaste/copypaste\n");
        assert_eq!(
            probe_status_with(Some(path), |_| Ok(installed)),
            Ok(UpdateStatus::Ready)
        );
        let unmanaged = output(0, "other/tap/other\n");
        assert_eq!(
            probe_status_with(Some(path), |_| Ok(unmanaged)),
            Ok(UpdateStatus::Unconfigured)
        );
        let failed = output(1, "");
        assert_eq!(
            probe_status_with(Some(path), |_| Ok(failed)),
            Err(UiError::from_boundary(
                UiBoundaryErrorCode::UpdateCheckFailed
            ))
        );
        assert_eq!(
            probe_status_with(Some(path), |_| Err(std::io::Error::other("probe failed"))),
            Err(UiError::from_boundary(
                UiBoundaryErrorCode::UpdateCheckFailed
            ))
        );
    }

    #[test]
    fn named_outdated_exit_and_json_must_agree() {
        const AVAILABLE_JSON: &str = r#"{"formulae":[],"casks":[{"name":"copypaste","installed_versions":["2.0.0"],"current_version":"2.1.0","pinned":false,"pinned_version":null}]}"#;
        let current = output(0, r#"{"formulae":[],"casks":[]}"#);
        assert_eq!(parse_outdated(&current), Ok(UpdateStatus::UpToDate));

        let available = output(1, AVAILABLE_JSON);
        assert_eq!(
            parse_outdated(&available),
            Ok(UpdateStatus::Available {
                version: "2.1.0".to_owned()
            })
        );

        for invalid in [
            output(1, r#"{"formulae":[],"casks":[]}"#),
            output(1, "brew failed"),
            output(0, "not JSON"),
            output(
                1,
                r#"{"formulae":[],"casks":[{"name":"other","installed_versions":["2.0.0"],"current_version":"2.1.0"}]}"#,
            ),
            output(
                1,
                r#"{"formulae":[],"casks":[{"name":"copypaste","current_version":"2.1.0"}]}"#,
            ),
            output(0, AVAILABLE_JSON),
            output(
                2,
                r#"{"formulae":[],"casks":[{"name":"copypaste","installed_versions":["2.0.0"],"current_version":"2.1.0"}]}"#,
            ),
        ] {
            assert_eq!(
                parse_outdated(&invalid),
                Err(UiError::from_boundary(
                    UiBoundaryErrorCode::UpdateCheckFailed
                ))
            );
        }
    }

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
