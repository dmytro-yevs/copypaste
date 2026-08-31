#[cfg(target_os = "windows")]
use super::UpdateProgress;
#[cfg(any(target_os = "windows", test))]
use super::{UiBoundaryErrorCode, UiError, UpdateStatus};
#[cfg(target_os = "windows")]
use tauri::{ipc::Channel, AppHandle};

#[cfg(any(target_os = "windows", test))]
async fn hand_off_verified_update<T, V, D, F, DF, P, I>(
    download: D,
    drain: F,
    installing: P,
    install: I,
) -> Result<tokio::task::JoinHandle<Result<(), UiError>>, UiError>
where
    D: std::future::Future<Output = Result<V, UiError>>,
    F: FnOnce() -> DF,
    DF: std::future::Future<Output = Result<T, UiError>>,
    P: FnOnce(),
    I: FnOnce(T, V) -> tokio::task::JoinHandle<Result<(), UiError>>,
{
    let bytes = download.await?;
    let permit = drain().await?;
    installing();
    Ok(install(permit, bytes))
}

#[cfg(any(target_os = "windows", test))]
async fn finish_handoff(
    install: tokio::task::JoinHandle<Result<(), UiError>>,
) -> Result<UpdateStatus, UiError> {
    install
        .await
        .map_err(|_| UiError::from_boundary(UiBoundaryErrorCode::UpdateInstallFailed))??;
    Err(UiError::from_boundary(
        UiBoundaryErrorCode::UpdateInstallFailed,
    ))
}

#[cfg(target_os = "windows")]
pub(super) fn status(app: &AppHandle) -> UpdateStatus {
    if super::config::configured_for_app(app) {
        UpdateStatus::Ready
    } else {
        UpdateStatus::Unconfigured
    }
}

#[cfg(target_os = "windows")]
pub(super) async fn check(app: &AppHandle) -> Result<UpdateStatus, UiError> {
    let Some(updater) = super::config::updater(app, Some(std::time::Duration::from_secs(300)))?
    else {
        return Ok(UpdateStatus::Unconfigured);
    };
    Ok(
        match updater.check().await.map_err(|error| {
            super::config::plugin_error(error, UiBoundaryErrorCode::UpdateCheckFailed)
        })? {
            Some(update) => UpdateStatus::Available {
                version: update.version.to_string(),
            },
            None => UpdateStatus::UpToDate,
        },
    )
}

#[cfg(target_os = "windows")]
pub(super) async fn install(
    app: &AppHandle,
    expected: String,
    progress: Channel<UpdateProgress>,
) -> Result<UpdateStatus, UiError> {
    let Some(updater) = super::config::updater(app, Some(std::time::Duration::from_secs(300)))?
    else {
        return Ok(UpdateStatus::Unconfigured);
    };
    let Some(update) = updater.check().await.map_err(|error| {
        super::config::plugin_error(error, UiBoundaryErrorCode::UpdateCheckFailed)
    })?
    else {
        return Ok(UpdateStatus::UpToDate);
    };
    let version = update.version.to_string();
    if version != expected {
        return Ok(UpdateStatus::Available { version });
    }
    let download_progress = progress.clone();
    let verifying_progress = progress.clone();
    let installing_progress = progress;
    let install = {
        use tauri::Manager as _;

        let supervisor = app.state::<crate::service::Supervisor>();
        let backend = app.state::<crate::backend::SelectedBackend>();
        hand_off_verified_update(
            async move {
                let mut downloaded = 0_u64;
                let bytes = update
                    .download(
                        move |chunk, total| {
                            downloaded = downloaded.saturating_add(chunk as u64);
                            let _ = download_progress
                                .send(UpdateProgress::Downloading { downloaded, total });
                        },
                        move || {
                            let _ = verifying_progress.send(UpdateProgress::Verifying);
                        },
                    )
                    .await
                    .map_err(|error| {
                        super::config::plugin_error(error, UiBoundaryErrorCode::UpdateInstallFailed)
                    })?;
                Ok((update, bytes))
            },
            || {
                Box::pin(async {
                    supervisor
                        .install_after_update_drain(backend.inner(), |permit| permit)
                        .await
                        .map_err(|error| error.ui_error())
                })
            },
            move || {
                let _ = installing_progress.send(UpdateProgress::Installing);
            },
            move |permit, (update, bytes)| {
                tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    update.install(bytes).map_err(|error| {
                        super::config::plugin_error(error, UiBoundaryErrorCode::UpdateInstallFailed)
                    })
                })
            },
        )
        .await?
    };
    finish_handoff(install).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn verified_update_drains_before_installing_and_installer() {
        struct InstallerRelease(Option<std::sync::mpsc::Sender<()>>);
        impl InstallerRelease {
            fn release(&mut self) {
                if let Some(release) = self.0.take() {
                    let _ = release.send(());
                }
            }
        }
        impl Drop for InstallerRelease {
            fn drop(&mut self) {
                self.release();
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let drain_events = Arc::clone(&events);
        let install_events = Arc::clone(&events);
        let (release, wait_for_release) = std::sync::mpsc::channel();
        let mut release = InstallerRelease(Some(release));
        let handoff = hand_off_verified_update(
            async { Ok::<_, UiError>(()) },
            move || {
                Box::pin(async move {
                    drain_events.lock().unwrap().push("drain");
                    Ok::<_, UiError>(())
                })
            },
            || events.lock().unwrap().push("installing"),
            move |_, _| {
                tokio::task::spawn_blocking(move || {
                    let _ = wait_for_release.recv();
                    install_events.lock().unwrap().push("installer");
                    Err(UiError::from_boundary(
                        UiBoundaryErrorCode::UpdateInstallFailed,
                    ))
                })
            },
        )
        .await
        .expect("verified drain hands off");

        assert_eq!(*events.lock().unwrap(), ["drain", "installing"]);
        release.release();
        assert!(finish_handoff(handoff).await.is_err());
        assert_eq!(
            *events.lock().unwrap(),
            ["drain", "installing", "installer"]
        );
    }

    #[tokio::test]
    async fn drain_failure_does_not_announce_or_start_installation() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let install_events = Arc::clone(&events);
        let result = hand_off_verified_update(
            async { Ok::<_, UiError>(()) },
            || async {
                Err::<(), _>(UiError::from_boundary(
                    UiBoundaryErrorCode::UpdateInstallFailed,
                ))
            },
            || events.lock().unwrap().push("installing"),
            move |_, _| {
                install_events.lock().unwrap().push("installer");
                tokio::task::spawn_blocking(|| Ok(()))
            },
        )
        .await;

        assert!(result.is_err());
        assert!(events.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_download_or_signature_never_drains_or_installs() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let drain_events = Arc::clone(&events);
        let install_events = Arc::clone(&events);
        let result = hand_off_verified_update(
            async {
                Err::<(), _>(UiError::from_boundary(
                    UiBoundaryErrorCode::UpdateInstallFailed,
                ))
            },
            move || {
                Box::pin(async move {
                    drain_events.lock().unwrap().push("drain");
                    Ok::<_, UiError>(())
                })
            },
            || events.lock().unwrap().push("installing"),
            move |_, _| {
                install_events.lock().unwrap().push("installer");
                tokio::task::spawn_blocking(|| Ok(()))
            },
        )
        .await;

        assert!(result.is_err());
        assert!(events.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn installer_error_preserves_the_mapped_sdk_error() {
        let handoff = hand_off_verified_update(
            async { Ok::<_, UiError>(()) },
            || Box::pin(async { Ok::<_, UiError>(()) }),
            || {},
            |_, _| {
                tokio::task::spawn_blocking(|| {
                    Err(UiError::from_boundary(
                        UiBoundaryErrorCode::UpdateNetworkFailed,
                    ))
                })
            },
        )
        .await
        .expect("handoff starts installer");

        assert_eq!(
            finish_handoff(handoff)
                .await
                .expect_err("SDK error is returned")
                .code,
            UiBoundaryErrorCode::UpdateNetworkFailed.as_str()
        );
    }

    #[tokio::test]
    async fn cancelling_before_handoff_never_drains_or_starts_the_installer() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let drain_events = Arc::clone(&events);
        let install_events = Arc::clone(&events);
        let (started, download_started) = tokio::sync::oneshot::channel();
        let mut operation = Box::pin(hand_off_verified_update(
            async move {
                let _ = started.send(());
                std::future::pending::<Result<(), UiError>>().await
            },
            move || {
                Box::pin(async move {
                    drain_events.lock().unwrap().push("drain");
                    Ok::<_, UiError>(())
                })
            },
            || events.lock().unwrap().push("installing"),
            move |_, _| {
                install_events.lock().unwrap().push("installer");
                tokio::task::spawn_blocking(|| Ok(()))
            },
        ));
        assert!(matches!(
            futures_util::poll!(&mut operation),
            std::task::Poll::Pending
        ));
        download_started.await.expect("download started");
        drop(operation);

        assert!(events.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancelling_after_installer_starts_keeps_its_permit_until_the_worker_ends() {
        struct Permit(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Permit {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (entered, entered_at_installer) = tokio::sync::oneshot::channel();
        let (finished, installer_finished) = tokio::sync::oneshot::channel();
        let (release, wait_for_release) = std::sync::mpsc::channel();
        let permit = Arc::clone(&dropped);
        let mut operation = Box::pin(async move {
            let handoff = hand_off_verified_update(
                async { Ok::<_, UiError>(()) },
                move || Box::pin(async move { Ok::<_, UiError>(Permit(permit)) }),
                || {},
                move |permit, _| {
                    tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        let _ = entered.send(());
                        let _ = wait_for_release.recv();
                        let _ = finished.send(());
                        Ok(())
                    })
                },
            )
            .await?;
            finish_handoff(handoff).await
        });
        assert!(matches!(
            futures_util::poll!(&mut operation),
            std::task::Poll::Pending
        ));
        entered_at_installer.await.expect("installer started");
        drop(operation);
        assert!(!dropped.load(std::sync::atomic::Ordering::SeqCst));

        release.send(()).expect("release installer");
        installer_finished.await.expect("installer finished");
        while !dropped.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn owning_download_hands_its_update_to_the_post_drain_installer() {
        struct Update(Arc<std::sync::atomic::AtomicBool>);
        impl Update {
            async fn download(&self) -> Result<(), UiError> {
                Ok(())
            }
        }
        impl Drop for Update {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let update = Update(Arc::clone(&dropped));
        let handoff = hand_off_verified_update(
            async move {
                update.download().await?;
                Ok::<_, UiError>((update, ()))
            },
            || async { Ok::<_, UiError>(()) },
            || {},
            |_, (update, _)| {
                assert!(!dropped.load(std::sync::atomic::Ordering::SeqCst));
                tokio::task::spawn_blocking(move || {
                    let _update = update;
                    Ok(())
                })
            },
        )
        .await
        .expect("verified download hands its update to the installer");

        assert!(!dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert!(finish_handoff(handoff).await.is_err());
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    }
}
