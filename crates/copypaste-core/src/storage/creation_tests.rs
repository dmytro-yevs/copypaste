use std::fs;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use super::creation::{exit_at, fail_next, pause_before_publish};
use super::test_support::{KEY, OTHER_KEY};
use super::{Store, StoreError};

const CHILD_ACTION: &str = "COPYPASTE_W5_CHILD_ACTION";
const CHILD_DATABASE: &str = "COPYPASTE_W5_CHILD_DATABASE";
const CHILD_READY: &str = "COPYPASTE_W5_CHILD_READY";
const CHILD_RELEASE: &str = "COPYPASTE_W5_CHILD_RELEASE";
const CHILD_TEST: &str = "storage::creation_tests::child_first_open";
const CHILD_WAIT_LIMIT: usize = 2_000;

#[test]
fn failed_first_open_before_schema_does_not_publish_an_invalid_final_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");

    // This hook is at the defect boundary. With the former create_new(final)
    // sequence, the same failure left `path` as an empty, permanently refused DB.
    fail_next("before-schema");
    assert!(Store::open(&path, &KEY).is_err());
    assert!(!path.exists());
    assert_no_staging_sidecars(dir.path());

    Store::open(&path, &KEY).expect("a later first open publishes a valid database");
}

#[test]
fn failed_first_open_after_staging_does_not_publish_a_final_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");

    fail_next("before-publish");
    assert!(Store::open(&path, &KEY).is_err());
    assert!(!path.exists());
    assert_no_staging_sidecars(dir.path());
}

#[test]
fn concurrent_first_opens_accept_the_valid_published_winner() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = Arc::new(dir.path().join("copypaste-v2.db"));
    let start = Arc::new(Barrier::new(2));
    let mut opens = Vec::new();

    for _ in 0..2 {
        let path = Arc::clone(&path);
        let start = Arc::clone(&start);
        opens.push(thread::spawn(move || {
            start.wait();
            Store::open(&path, &KEY)
        }));
    }
    for open in opens {
        open.join()
            .expect("first-open thread panicked")
            .expect("valid winner");
    }

    Store::open(&path, &KEY).expect("published database reopens");
    assert!(matches!(
        Store::open(&path, &OTHER_KEY),
        Err(StoreError::InvalidKey)
    ));
    assert_no_staging_sidecars(dir.path());
}

#[test]
fn child_crashes_before_publication_leave_no_final_file() {
    for stage in ["before-schema", "before-publish"] {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");

        assert_crashed(child_command(stage, &path));
        assert!(!path.exists(), "{stage} crash published a final file");
    }
}

#[test]
fn child_crash_after_publication_leaves_a_valid_final_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");

    assert_crashed(child_command("after-parent-sync", &path));
    Store::open(&path, &KEY).expect("published database reopens after child exit");
    assert!(matches!(
        Store::open(&path, &OTHER_KEY),
        Err(StoreError::InvalidKey)
    ));
    assert_no_staging_sidecars(dir.path());
}

#[test]
fn two_child_first_openers_publish_one_valid_database() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");
    let release = dir.path().join("release");
    let first_ready = dir.path().join("first-ready");
    let second_ready = dir.path().join("second-ready");

    let mut first = OwnedChild::spawn(
        child_command("pause-before-publish", &path)
            .env(CHILD_READY, &first_ready)
            .env(CHILD_RELEASE, &release)
            .spawn()
            .unwrap(),
    );
    let mut second = OwnedChild::spawn(
        child_command("pause-before-publish", &path)
            .env(CHILD_READY, &second_ready)
            .env(CHILD_RELEASE, &release)
            .spawn()
            .unwrap(),
    );
    wait_for_marker(&first_ready);
    wait_for_marker(&second_ready);
    fs::write(&release, []).unwrap();

    assert!(first.wait().success());
    assert!(second.wait().success());
    Store::open(&path, &KEY).expect("child-process winner is valid");
    assert_no_staging_sidecars(dir.path());
}

#[test]
fn existing_non_database_bytes_are_not_repaired_or_replaced() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");
    let bytes = b"not a CopyPaste database";
    fs::write(&path, bytes).unwrap();

    assert!(Store::open(&path, &KEY).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn wrong_key_does_not_change_existing_database_bytes() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");
    drop(Store::open(&path, &KEY).unwrap());
    let bytes = fs::read(&path).unwrap();

    assert!(matches!(
        Store::open(&path, &OTHER_KEY),
        Err(StoreError::InvalidKey)
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn corrupt_keyed_database_does_not_change_bytes() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");
    drop(Store::open(&path, &KEY).unwrap());
    let mut corrupted = fs::read(&path).unwrap();
    let middle = corrupted.len() / 2;
    corrupted[middle] ^= 0x80;
    corrupted.truncate(corrupted.len() - 1);
    fs::write(&path, &corrupted).unwrap();

    assert!(Store::open(&path, &KEY).is_err());
    assert_eq!(fs::read(&path).unwrap(), corrupted);
}

#[test]
fn faulted_first_open_stages_leave_the_final_name_absent_or_valid() {
    for stage in [
        "after-schema",
        "after-checkpoint",
        "after-close",
        "after-staged-sync",
    ] {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");

        fail_next(stage);
        assert!(
            Store::open(&path, &KEY).is_err(),
            "{stage} fault returned success"
        );
        assert!(!path.exists(), "{stage} fault published a final file");
    }

    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("copypaste-v2.db");
    fail_next("after-parent-sync");
    assert!(Store::open(&path, &KEY).is_err());
    Store::open(&path, &KEY).expect("post-publication fault left a valid final database");
}

fn assert_no_staging_sidecars(parent: &std::path::Path) {
    let left_behind: Vec<_> = fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().starts_with(".copypaste-create-"))
        .collect();
    assert!(left_behind.is_empty(), "staging artifacts: {left_behind:?}");
}

#[test]
fn child_first_open() {
    let Ok(action) = std::env::var(CHILD_ACTION) else {
        return;
    };
    let path = std::env::var_os(CHILD_DATABASE)
        .map(std::path::PathBuf::from)
        .expect("child database path");

    match action.as_str() {
        "before-schema" | "before-publish" | "after-parent-sync" => {
            exit_at(&action);
            let _ = Store::open(&path, &KEY);
            panic!("faulted first open returned instead of exiting");
        }
        "pause-before-publish" => {
            let ready = std::env::var_os(CHILD_READY)
                .map(std::path::PathBuf::from)
                .expect("child ready marker");
            let release = std::env::var_os(CHILD_RELEASE)
                .map(std::path::PathBuf::from)
                .expect("child release marker");
            pause_before_publish(ready, release);
            Store::open(&path, &KEY).expect("child first open");
        }
        _ => panic!("unknown child action"),
    }
}

fn child_command(action: &str, path: &std::path::Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(CHILD_TEST)
        .arg("--test-threads=1")
        .env(CHILD_ACTION, action)
        .env(CHILD_DATABASE, path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn assert_crashed(mut command: Command) {
    let status = OwnedChild::spawn(command.spawn().unwrap()).wait();
    assert_eq!(status.code(), Some(86), "unexpected child status: {status}");
}

fn wait_for_marker(marker: &std::path::Path) {
    for _ in 0..CHILD_WAIT_LIMIT {
        if marker.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("child did not reach publication barrier");
}

struct OwnedChild {
    child: Option<Child>,
}

impl OwnedChild {
    fn spawn(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn wait(&mut self) -> ExitStatus {
        for _ in 0..CHILD_WAIT_LIMIT {
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                self.child.take();
                return status;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("child did not exit before the bounded wait elapsed");
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        let _ = child.kill();
        for _ in 0..CHILD_WAIT_LIMIT {
            if child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
