//! First publication of a keyed v2 database.

use std::io;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::connection::{apply_connection_pragmas, apply_key, validate_key};
use super::model::StoreError;

enum Publication {
    New,
    ExistingWinner,
}

#[cfg(test)]
thread_local! {
    static TEST_FAULT: std::cell::RefCell<Option<TestFault>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq)]
enum TestStage {
    BeforeSchema,
    AfterSchema,
    AfterCheckpoint,
    AfterClose,
    AfterStagedSync,
    BeforePublish,
    AfterParentSync,
}

#[cfg(test)]
#[derive(Clone)]
enum TestFault {
    Fail(TestStage),
    Exit(TestStage),
    PauseBeforePublish {
        ready: std::path::PathBuf,
        release: std::path::PathBuf,
    },
}

/// Build and publish a new database without reserving the user-visible name
/// until it is keyed, schema-validated, checkpointed, and closed.
pub(super) fn create_and_publish(path: &Path, db_key: &[u8; 32]) -> Result<(), StoreError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let staged = tempfile::Builder::new()
        .prefix(".copypaste-create-")
        .tempfile_in(parent)?;
    let staged_path = staged.path().to_owned();

    initialise_staged(&staged_path, db_key)?;
    staged.as_file().sync_all()?;
    fail_after_staged_sync()?;
    fail_before_publish()?;

    match publish_staged(staged, path)? {
        Publication::New => {
            #[cfg(unix)]
            sync_published_parent(parent)?;
            fail_after_parent_sync()?;
            Ok(())
        }
        Publication::ExistingWinner => Ok(()),
    }
}

#[cfg(not(target_os = "windows"))]
fn publish_staged(staged: tempfile::NamedTempFile, path: &Path) -> Result<Publication, StoreError> {
    match staged.persist_noclobber(path) {
        Ok(_) => Ok(Publication::New),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            Ok(Publication::ExistingWinner)
        }
        Err(error) => Err(error.error.into()),
    }
}

#[cfg(target_os = "windows")]
fn publish_staged(staged: tempfile::NamedTempFile, path: &Path) -> Result<Publication, StoreError> {
    use winsafe::{co, MoveFileEx, SetFileAttributes};

    let staged = staged.into_temp_path();
    let staged_name = path_as_utf8(&staged)?;
    let final_name = path_as_utf8(path)?;
    SetFileAttributes(staged_name, co::FILE_ATTRIBUTE::NORMAL).map_err(windows_io_error)?;
    match MoveFileEx(staged_name, Some(final_name), co::MOVEFILE::WRITE_THROUGH) {
        Ok(()) => Ok(Publication::New),
        Err(error) if windows_io_error(error).kind() == io::ErrorKind::AlreadyExists => {
            Ok(Publication::ExistingWinner)
        }
        Err(error) => Err(windows_io_error(error).into()),
    }
}

#[cfg(target_os = "windows")]
fn path_as_utf8(path: &Path) -> Result<&str, StoreError> {
    path.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "database path is not UTF-8").into()
    })
}

#[cfg(target_os = "windows")]
fn windows_io_error(error: winsafe::co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}

#[cfg(unix)]
fn sync_published_parent(parent: &Path) -> Result<(), StoreError> {
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn initialise_staged(path: &Path, db_key: &[u8; 32]) -> Result<(), StoreError> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    apply_key(&conn, db_key)?;
    validate_key(&conn)?;
    apply_connection_pragmas(&conn)?;
    fail_before_schema()?;
    super::schema::create(&mut conn)?;
    fail_after_schema()?;
    checkpoint_staged(&conn)?;
    fail_after_checkpoint()?;
    conn.close().map_err(|(_, error)| StoreError::from(error))?;
    fail_after_close()
}

fn checkpoint_staged(conn: &Connection) -> Result<(), StoreError> {
    let (busy, written, checkpointed) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
    if busy == 0 && written == checkpointed {
        Ok(())
    } else {
        Err(io::Error::other("staged database checkpoint did not complete").into())
    }
}

#[cfg(test)]
fn fail_at(stage: TestStage) -> Result<(), StoreError> {
    let fault = TEST_FAULT.with(|fault| {
        let mut fault = fault.borrow_mut();
        match fault.as_ref() {
            Some(TestFault::Fail(expected)) | Some(TestFault::Exit(expected))
                if *expected == stage =>
            {
                fault.take()
            }
            Some(TestFault::PauseBeforePublish { .. }) if stage == TestStage::BeforePublish => {
                fault.clone()
            }
            _ => None,
        }
    });

    match fault {
        Some(TestFault::Fail(_)) => Err(io::Error::other("injected first-open failure").into()),
        Some(TestFault::Exit(_)) => std::process::exit(86),
        Some(TestFault::PauseBeforePublish { ready, release }) => {
            std::fs::write(ready, [])?;
            while !release.try_exists()? {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok(())
        }
        None => Ok(()),
    }
}

#[cfg(not(test))]
fn fail_before_schema() -> Result<(), StoreError> {
    Ok(())
}

#[cfg(test)]
fn fail_before_schema() -> Result<(), StoreError> {
    fail_at(TestStage::BeforeSchema)
}

#[cfg(test)]
fn fail_before_publish() -> Result<(), StoreError> {
    fail_at(TestStage::BeforePublish)
}

#[cfg(not(test))]
fn fail_before_publish() -> Result<(), StoreError> {
    Ok(())
}

macro_rules! test_fault_stage {
    ($name:ident, $stage:ident) => {
        #[cfg(test)]
        fn $name() -> Result<(), StoreError> {
            fail_at(TestStage::$stage)
        }

        #[cfg(not(test))]
        fn $name() -> Result<(), StoreError> {
            Ok(())
        }
    };
}

test_fault_stage!(fail_after_schema, AfterSchema);
test_fault_stage!(fail_after_checkpoint, AfterCheckpoint);
test_fault_stage!(fail_after_close, AfterClose);
test_fault_stage!(fail_after_staged_sync, AfterStagedSync);
test_fault_stage!(fail_after_parent_sync, AfterParentSync);

#[cfg(test)]
fn test_stage(stage: &str) -> TestStage {
    let stage = match stage {
        "before-schema" => TestStage::BeforeSchema,
        "after-schema" => TestStage::AfterSchema,
        "after-checkpoint" => TestStage::AfterCheckpoint,
        "after-close" => TestStage::AfterClose,
        "after-staged-sync" => TestStage::AfterStagedSync,
        "before-publish" => TestStage::BeforePublish,
        "after-parent-sync" => TestStage::AfterParentSync,
        _ => panic!("unknown first-open exit stage"),
    };
    stage
}

#[cfg(test)]
pub(super) fn fail_next(stage: &str) {
    let stage = test_stage(stage);
    TEST_FAULT.with(|fault| *fault.borrow_mut() = Some(TestFault::Fail(stage)));
}

#[cfg(test)]
pub(super) fn exit_at(stage: &str) {
    let stage = test_stage(stage);
    TEST_FAULT.with(|fault| *fault.borrow_mut() = Some(TestFault::Exit(stage)));
}

#[cfg(test)]
pub(super) fn pause_before_publish(ready: std::path::PathBuf, release: std::path::PathBuf) {
    TEST_FAULT.with(|fault| {
        *fault.borrow_mut() = Some(TestFault::PauseBeforePublish { ready, release });
    });
}
