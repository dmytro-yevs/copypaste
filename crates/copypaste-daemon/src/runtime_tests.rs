use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use copypaste_core::storage::open_validated;
use copypaste_core::Store;
use rusqlite::TransactionBehavior;

use crate::runtime::run_with_bounded_shutdown;

const ACTION: &str = "COPYPASTE_RUNTIME_TEST_ACTION";
const STARTED: &str = "COPYPASTE_RUNTIME_TEST_STARTED";
const MARKER: &str = "COPYPASTE_RUNTIME_TEST_MARKER";
const DATABASE: &str = "COPYPASTE_RUNTIME_TEST_DATABASE";
const DROP_CHILD: &str = "runtime_tests::runtime_drop_child_waits_forever";
const SHUTDOWN_CHILD: &str = "runtime_tests::runtime_shutdown_child_returns";
const DB_CHILD: &str = "runtime_tests::runtime_shutdown_rolls_back_child_transaction";
const WAIT: Duration = Duration::from_secs(5);
const KEY: [u8; 32] = [0x5a; 32];

#[test]
fn runtime_drop_child_waits_forever() {
    if std::env::var(ACTION).ok().as_deref() != Some("drop") {
        return;
    }

    let started = env_path(STARTED);
    let marker = env_path(MARKER);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(start_never_returning_blocking_work(started))
        .unwrap();
    drop(runtime);
    fs::write(marker, "runtime drop returned").unwrap();
}

#[test]
fn runtime_shutdown_child_returns() {
    if std::env::var(ACTION).ok().as_deref() != Some("shutdown") {
        return;
    }

    let started = env_path(STARTED);
    let marker = env_path(MARKER);
    run_with_bounded_shutdown(start_never_returning_blocking_work(started)).unwrap();
    fs::write(marker, "runtime shutdown returned").unwrap();
}

#[test]
fn runtime_shutdown_rolls_back_child_transaction() {
    if std::env::var(ACTION).ok().as_deref() != Some("database") {
        return;
    }

    let started = env_path(STARTED);
    let database = env_path(DATABASE);
    run_with_bounded_shutdown(async move {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        tokio::task::spawn_blocking(move || {
            let mut connection = open_validated(&database, &KEY).unwrap();
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2)",
                    ["w6b_uncommitted", "must not persist"],
                )
                .unwrap();
            fs::write(started, "transaction started").unwrap();
            ready_tx.send(()).unwrap();
            std::thread::park();
            drop(transaction);
        });
        ready_rx.await.unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn runtime_drop_waits_for_started_blocking_work() {
    let dir = tempfile::tempdir().unwrap();
    let started = dir.path().join("drop-started");
    let marker = dir.path().join("drop-marker");
    let mut child = OwnedChild::new(spawn_child("drop", DROP_CHILD, &started, &marker, None));

    wait_for_file(&started);
    assert!(
        child.child.try_wait().unwrap().is_none(),
        "runtime drop returned"
    );
    assert!(
        !marker.exists(),
        "runtime drop reached its completion marker"
    );
    child.reap();
}

#[test]
fn owned_runtime_returns_after_the_daemon_teardown_budget() {
    let dir = tempfile::tempdir().unwrap();
    let started = dir.path().join("shutdown-started");
    let marker = dir.path().join("shutdown-marker");
    let mut child = OwnedChild::new(spawn_child(
        "shutdown",
        SHUTDOWN_CHILD,
        &started,
        &marker,
        None,
    ));

    wait_for_file(&started);
    wait_for_file(&marker);
    assert_child_exits(&mut child.child);
}

#[test]
fn process_exit_preserves_committed_history_and_rolls_back_open_work() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("copypaste-v2.db");
    let store = Store::open(&database, &KEY).unwrap();
    store
        .set_state("w6b_committed", "survives process exit")
        .unwrap();
    drop(store);

    let started = dir.path().join("transaction-started");
    let marker = dir.path().join("unused-marker");
    let mut child = OwnedChild::new(spawn_child(
        "database",
        DB_CHILD,
        &started,
        &marker,
        Some(&database),
    ));

    wait_for_file(&started);
    assert_child_exits(&mut child.child);

    let reopened = Store::open(&database, &KEY).unwrap();
    assert_eq!(
        reopened.state("w6b_committed").unwrap().as_deref(),
        Some("survives process exit")
    );
    assert_eq!(reopened.state("w6b_uncommitted").unwrap(), None);
}

async fn start_never_returning_blocking_work(started: PathBuf) -> anyhow::Result<()> {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    tokio::task::spawn_blocking(move || {
        fs::write(started, "blocking work started").unwrap();
        ready_tx.send(()).unwrap();
        std::thread::park();
    });
    ready_rx.await.unwrap();
    Ok(())
}

fn spawn_child(
    action: &str,
    test: &str,
    started: &Path,
    marker: &Path,
    database: Option<&Path>,
) -> Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(test)
        .arg("--nocapture")
        .env(ACTION, action)
        .env(STARTED, started)
        .env(MARKER, marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(database) = database {
        command.env(DATABASE, database);
    }
    command.spawn().unwrap()
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + WAIT;
    while !path.exists() {
        assert!(Instant::now() < deadline, "child did not signal readiness");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_child_exits(child: &mut Child) {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "child failed: {status}");
            return;
        }
        assert!(Instant::now() < deadline, "child did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct OwnedChild {
    child: Child,
}

impl OwnedChild {
    fn new(child: Child) -> Self {
        Self { child }
    }

    fn reap(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
        }
        self.child.wait().unwrap();
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn env_path(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap()
}
