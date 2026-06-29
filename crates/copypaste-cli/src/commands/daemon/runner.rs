//! Abstract I/O interfaces used by the daemon subcommand.
//!
//! Splitting these into a dedicated module keeps `platform.rs` free of
//! concrete std-I/O boilerplate and lets unit tests substitute mock
//! implementations without touching the real filesystem or spawning processes.

use anyhow::Result;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Shared output type
// ---------------------------------------------------------------------------

pub(crate) struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

pub(crate) trait CommandRunner {
    fn run(&mut self, program: &str, args: &[OsString]) -> Result<CommandOutput>;
}

pub(crate) trait FsOps {
    fn home_dir(&self) -> Option<PathBuf>;
    fn current_dir(&self) -> Result<PathBuf>;
    fn current_exe(&self) -> Result<PathBuf>;
    fn exists(&self, path: &Path) -> bool;
    fn create_dir_all(&mut self, path: &Path) -> Result<()>;
    fn remove_file(&mut self, path: &Path) -> Result<()>;
    fn read_to_string(&self, path: &Path) -> Result<String>;
    fn write(&mut self, path: &Path, content: &str) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Production (system) implementations
// ---------------------------------------------------------------------------

/// Runs real child processes via [`std::process::Command`].
#[derive(Default)]
pub(super) struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&mut self, program: &str, args: &[OsString]) -> Result<CommandOutput> {
        let out = std::process::Command::new(program).args(args).output()?;
        Ok(CommandOutput {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// Delegates all filesystem operations to `std::fs` / `std::env`.
pub(super) struct SystemFs;

impl FsOps for SystemFs {
    fn home_dir(&self) -> Option<PathBuf> {
        home::home_dir()
    }
    fn current_dir(&self) -> Result<PathBuf> {
        Ok(std::env::current_dir()?)
    }
    fn current_exe(&self) -> Result<PathBuf> {
        Ok(std::env::current_exe()?)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn create_dir_all(&mut self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }
    fn remove_file(&mut self, path: &Path) -> Result<()> {
        std::fs::remove_file(path)?;
        Ok(())
    }
    fn read_to_string(&self, path: &Path) -> Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }
    fn write(&mut self, path: &Path, content: &str) -> Result<()> {
        std::fs::write(path, content)?;
        Ok(())
    }
}
