//! App-owned daemon lifecycle for the desktop Flutter application.
//!
//! The application is explicit about its data directory. The manager rejects
//! the standalone CLI directory, removes endpoint and cloud overrides from the
//! child, and makes the daemon observe the app-parent pipe. Development and
//! production therefore use real platform adapters without sharing ambiguous
//! process or storage ownership with the standalone CLI.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::time::sleep;

use crate::api::RuntimeError;

struct RunningRuntime {
    socket_path: PathBuf,
    #[allow(dead_code)]
    child: Child,
}

static RUNTIME: OnceLock<Mutex<Option<RunningRuntime>>> = OnceLock::new();
static WATCHES: OnceLock<Mutex<HashMap<u64, tokio::sync::watch::Sender<bool>>>> = OnceLock::new();
static NEXT_WATCH_ID: AtomicU64 = AtomicU64::new(1);

fn slot() -> &'static Mutex<Option<RunningRuntime>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}

fn watches() -> &'static Mutex<HashMap<u64, tokio::sync::watch::Sender<bool>>> {
    WATCHES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn allocate_watch() -> u64 {
    let id = NEXT_WATCH_ID.fetch_add(1, Ordering::Relaxed);
    watches()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(id, tokio::sync::watch::channel(false).0);
    id
}

pub(crate) fn watch_stop_rx(id: u64) -> Result<tokio::sync::watch::Receiver<bool>, RuntimeError> {
    watches()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&id)
        .map(tokio::sync::watch::Sender::subscribe)
        .ok_or_else(RuntimeError::watch_not_found)
}

pub(crate) fn cancel_watch(id: u64) {
    if let Some(sender) = watches()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&id)
    {
        let _ = sender.send(true);
    }
}

pub(crate) fn cancel_all_watches() {
    let active = std::mem::take(
        &mut *watches()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    );
    for sender in active.into_values() {
        let _ = sender.send(true);
    }
}

pub(crate) fn socket_path() -> Result<PathBuf, RuntimeError> {
    slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map(|runtime| runtime.socket_path.clone())
        .ok_or_else(RuntimeError::not_initialized)
}

pub(crate) async fn start(daemon_executable: String, data_dir: String) -> Result<(), RuntimeError> {
    let data_dir = validate_data_dir(&data_dir)?;
    let socket_path = isolated_socket_path(&data_dir)?;
    let startup_log = open_startup_log(&data_dir)?;
    let mut startup_diagnostic = startup_log
        .try_clone()
        .map_err(|_| RuntimeError::daemon_start_failed())?;
    let exited_child = {
        let mut guard = slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.as_mut().map(|runtime| runtime.child.try_wait()) {
            Some(Ok(None)) => return Ok(()),
            Some(Ok(Some(_)) | Err(_)) => guard.take().map(|runtime| runtime.child),
            None => None,
        }
    };
    if let Some(child) = exited_child {
        reap(child);
    }
    {
        let mut guard = slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let child = Command::new(daemon_executable)
            .args([
                "--data-dir",
                data_dir.to_string_lossy().as_ref(),
                "--port",
                "0",
                "--foreground",
                "--app-parent",
            ])
            // Child startup must not inherit endpoint or cloud configuration
            // that could redirect the app-owned runtime.
            .env_remove("COPYPASTE_SOCKET")
            .env_remove("COPYPASTE_DATA_DIR")
            .env_remove("COPYPASTE_CLOUD_URL")
            .env_remove("COPYPASTE_CLOUD_ANON_KEY")
            .env("COPYPASTE_SOCKET", &socket_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(startup_log)
            .spawn()
            .map_err(|error| {
                use std::io::Write as _;
                let _ = writeln!(startup_diagnostic, "spawn failed: {error}");
                RuntimeError::daemon_spawn_failed()
            })?;
        *guard = Some(RunningRuntime {
            socket_path: socket_path.clone(),
            child,
        });
    }

    // Keychain access on macOS is user-mediated and may legitimately take much
    // longer than an arbitrary startup deadline. The Flutter shell remains
    // available while this future waits, so keep observing the owned child until
    // it either exposes its socket, exits, or the application stops it.
    loop {
        if copypaste_ipc::transport::connect(&socket_path)
            .await
            .is_ok()
        {
            return Ok(());
        }
        match owned_child_state() {
            OwnedChildState::Running => sleep(Duration::from_millis(100)).await,
            OwnedChildState::Exited(status) => {
                append_startup_diagnostic(&data_dir, "child exited", status);
                stop();
                return Err(RuntimeError::daemon_exited_early());
            }
            OwnedChildState::Stopped => return Err(RuntimeError::daemon_start_failed()),
        }
    }
}

/// A macOS application-support path can exceed the Unix socket pathname limit.
/// Keep the endpoint in the app's private temporary directory while durable
/// history remains in the explicit application-support directory.
fn isolated_socket_path(_data_dir: &Path) -> Result<PathBuf, RuntimeError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        let path = std::env::temp_dir().join(format!("cp-{}.sock", std::process::id()));
        if path.as_os_str().as_bytes().len() >= 104 {
            return Err(RuntimeError::daemon_start_failed());
        }
        return Ok(path);
    }
    #[cfg(not(unix))]
    {
        Ok(_data_dir.join("daemon.sock"))
    }
}

enum OwnedChildState {
    Running,
    Exited(String),
    Stopped,
}

fn owned_child_state() -> OwnedChildState {
    let mut guard = slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(runtime) = guard.as_mut() else {
        return OwnedChildState::Stopped;
    };
    match runtime.child.try_wait() {
        Ok(None) => OwnedChildState::Running,
        Ok(Some(status)) => OwnedChildState::Exited(status.to_string()),
        Err(_) => OwnedChildState::Exited("status unavailable".to_string()),
    }
}

fn append_startup_diagnostic(data_dir: &Path, stage: &str, detail: impl std::fmt::Display) {
    use std::io::Write as _;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .append(true)
        .open(data_dir.join("daemon-startup.log"))
    {
        let _ = writeln!(file, "{stage}: {detail}");
    }
}

/// Captures only daemon startup diagnostics inside the isolated data directory.
/// Public bridge errors remain pathless.
fn open_startup_log(data_dir: &Path) -> Result<std::fs::File, RuntimeError> {
    std::fs::create_dir_all(data_dir).map_err(|_| RuntimeError::daemon_start_failed())?;
    let path = data_dir.join("daemon-startup.log");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|_| RuntimeError::daemon_start_failed())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    Ok(file)
}

pub(crate) fn stop() {
    cancel_all_watches();
    let child = slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
        .map(|mut runtime| {
            // Closing this pipe is the daemon's app-parent shutdown signal.
            // Take it before the child moves to the reaper so EOF is prompt.
            let _ = runtime.child.stdin.take();
            runtime.child
        });
    if let Some(child) = child {
        reap(child);
    }
}

/// Waits for an owned daemon after its app-parent pipe closes.
///
/// `Child::drop` does not reap a process. Use Tokio when the bridge is running
/// under FRB and a plain thread only for synchronous test/process teardown.
fn reap(mut child: Child) {
    let wait = move || {
        let _ = child.wait();
    };
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn_blocking(wait);
    } else {
        let _ = std::thread::Builder::new()
            .name("copypaste-daemon-reaper".into())
            .spawn(wait);
    }
}

fn validate_data_dir(value: &str) -> Result<PathBuf, RuntimeError> {
    validate_data_dir_against(value, &copypaste_ipc::data_dir())
}

fn validate_data_dir_against(value: &str, production: &Path) -> Result<PathBuf, RuntimeError> {
    if value.trim().is_empty() {
        return Err(RuntimeError::unsafe_data_directory());
    }
    let path = canonical_target(Path::new(value))?;
    is_isolated_from(&path, &canonical_target(production)?)
        .then_some(path)
        .ok_or_else(RuntimeError::unsafe_data_directory)
}

/// Resolves aliases for an existing directory and for a not-yet-created child.
///
/// The latter case canonicalizes the nearest existing parent and only then
/// appends ordinary path components, so `..` and a symlink parent cannot name
/// the retained production directory indirectly.
fn canonical_target(path: &Path) -> Result<PathBuf, RuntimeError> {
    let mut candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| RuntimeError::unsafe_data_directory())?
            .join(path)
    };
    let mut suffix = Vec::new();
    loop {
        match std::fs::canonicalize(&candidate) {
            Ok(mut canonical) => {
                for component in suffix.iter().rev() {
                    match Path::new(component).components().next() {
                        Some(Component::ParentDir) => {
                            if !canonical.pop() {
                                return Err(RuntimeError::unsafe_data_directory());
                            }
                        }
                        Some(Component::CurDir) => {}
                        Some(Component::Normal(name)) => canonical.push(name),
                        Some(Component::RootDir | Component::Prefix(_)) | None => {
                            return Err(RuntimeError::unsafe_data_directory())
                        }
                    }
                }
                return Ok(canonical);
            }
            Err(_) => {
                let Some(name) = candidate.file_name() else {
                    return Err(RuntimeError::unsafe_data_directory());
                };
                suffix.push(name.to_os_string());
                if !candidate.pop() {
                    return Err(RuntimeError::unsafe_data_directory());
                }
            }
        }
    }
}

fn is_isolated_from(candidate: &Path, production: &Path) -> bool {
    candidate != production && !candidate.starts_with(production)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_data_directory_is_rejected() {
        let production = copypaste_ipc::data_dir();
        let error = validate_data_dir(production.to_string_lossy().as_ref()).unwrap_err();
        assert_eq!(error.code, "unsafe_data_directory");
    }

    #[test]
    fn isolated_socket_is_inside_the_explicit_directory() {
        let dir = tempfile::tempdir().unwrap();
        let isolated = validate_data_dir(dir.path().to_string_lossy().as_ref()).unwrap();
        assert_eq!(
            isolated.join("daemon.sock"),
            canonical_target(dir.path()).unwrap().join("daemon.sock")
        );
    }

    #[test]
    fn parent_segment_alias_of_production_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let production = root.path().join("production");
        std::fs::create_dir(&production).unwrap();
        let alias = root.path().join("other").join("..").join("production");
        assert!(matches!(
            validate_data_dir_against(alias.to_string_lossy().as_ref(), &production),
            Err(error) if error.code == "unsafe_data_directory"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_alias_of_production_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let production = root.path().join("production");
        std::fs::create_dir(&production).unwrap();
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&production, &alias).unwrap();
        assert!(matches!(
            validate_data_dir_against(alias.to_string_lossy().as_ref(), &production),
            Err(error) if error.code == "unsafe_data_directory"
        ));
    }

    #[tokio::test]
    async fn cancelling_one_quiet_watch_keeps_another_active() {
        let first = allocate_watch();
        let second = allocate_watch();
        let mut first_cancelled = watch_stop_rx(first).unwrap();
        let mut second_cancelled = watch_stop_rx(second).unwrap();
        let waiting = tokio::spawn(async move {
            first_cancelled
                .changed()
                .await
                .expect("watch sender stays available");
        });
        tokio::task::yield_now().await;
        cancel_watch(first);
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("first quiet watch cancellation returns")
            .expect("watch task does not panic");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), second_cancelled.changed())
                .await
                .is_err(),
            "cancelling one watch must not stop another"
        );
        cancel_watch(second);
    }

    #[tokio::test]
    async fn isolated_test_child_uses_the_explicit_fake_clipboard_build() {
        let Ok(daemon) = std::env::var("COPYPASTE_TEST_DAEMON_BIN") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        start(daemon, dir.path().to_string_lossy().into_owned())
            .await
            .expect("the isolated daemon starts");
        let response = crate::client::request(copypaste_ipc::Method::Status)
            .await
            .expect("the isolated daemon answers status");
        let Some(copypaste_ipc::ResponseData::Status(status)) = response.data else {
            panic!("the runtime returned a status payload");
        };
        assert!(
            status.clipboard_backend.starts_with("fake"),
            "the test daemon must use its explicit fake-clipboard feature"
        );
        stop();
    }

    #[tokio::test]
    async fn stopping_and_restarting_releases_the_owned_test_daemon() {
        let Ok(daemon) = std::env::var("COPYPASTE_TEST_DAEMON_BIN") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().to_string_lossy().into_owned();
        start(daemon.clone(), data_dir.clone()).await.unwrap();
        // A live owned daemon is reused rather than duplicated.
        start(daemon.clone(), data_dir.clone()).await.unwrap();
        stop();
        for _ in 0..40 {
            if socket_path().is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(
            socket_path().is_err(),
            "stop removes the owned runtime slot"
        );
        start(daemon, data_dir).await.unwrap();
        stop();
    }
}
