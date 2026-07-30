//! Where things live on disk.
//!
//! One resolver per path, in one place: v1 duplicated the socket path in three
//! files and its own module doc admitted it.

use std::path::PathBuf;

/// Where the daemon socket lives.
///
/// One definition, used by the daemon and the CLI. v1 duplicated this logic in
/// three places and the module doc admitted it.
pub fn socket_path() -> PathBuf {
    data_dir().join("daemon.sock")
}

/// v2 database filename. Deliberately distinct from v1's, so an existing v0.4.x
/// database is never opened, modified, or reported as corrupt — see CLAUDE.md
/// rule 3.
pub fn database_path() -> PathBuf {
    data_dir().join("copypaste-v2.db")
}

/// Where CopyPaste 0.4 kept its data, so a v0.4 history can be *looked for*.
///
/// A second resolver rather than a variant of [`data_dir`], because the two
/// genuinely differ and that difference is deliberate: v0.4.x built
/// `~/Library/Application Support/CopyPaste` by hand (port manifest 04 §3.1),
/// while [`data_dir`]'s `ProjectDirs::from("com", "copypaste", "CopyPaste")`
/// resolves `~/Library/Application Support/com.copypaste.CopyPaste`. Never
/// touching the old file is half of CLAUDE.md rule 3; knowing where it is, so a
/// v2 build can say it found one, is the other half.
///
/// `copypaste_core::v1_database_in` takes a directory rather than resolving one
/// precisely so that this decision lives here, with the rest of path
/// resolution, instead of being made twice.
///
/// **Read-only, by contract.** Nothing may open, create or write anything under
/// the returned path: a user who downgrades must find their history exactly as
/// they left it.
///
/// `None` when there is no home directory to resolve from. Android is not
/// covered — v0.4's Android history lived in the app's own sandbox, which this
/// process cannot address by a user-level path.
#[must_use]
pub fn v1_data_dir() -> Option<PathBuf> {
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    #[cfg(target_os = "macos")]
    {
        Some(
            home.join("Library")
                .join("Application Support")
                .join("CopyPaste"),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux and the rest of Unix, which is where the tests run. v0.4.x used
        // `$XDG_DATA_HOME/copypaste`, and so does `ProjectDirs` here — the two
        // coincide, and the distinct *filename* is what keeps them apart.
        Some(
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"))
                .join("copypaste"),
        )
    }
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "copypaste", "CopyPaste")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".copypaste"))
}
