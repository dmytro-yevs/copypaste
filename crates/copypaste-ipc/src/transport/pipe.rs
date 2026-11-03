//! The Windows endpoint: a named pipe whose name preserves per-data-directory
//! separation without disclosing the source path.

use std::ffi::OsString;
use std::io;
use std::path::Path;

use interprocess::os::windows::named_pipe::{
    pipe_mode::Bytes,
    tokio::{DuplexPipeStream, PipeListener},
    PipeListenerOptions,
};
use sha2::{Digest, Sha256};

const LOCAL_PIPES: &str = r"\\.\pipe\";

pub type Listener = PipeListener<Bytes, Bytes>;
pub type Stream = DuplexPipeStream<Bytes>;

/// The pipe that stands in for the socket at `path`.
///
/// The digest restores the separation a per-user data-directory path had
/// before it became a machine-global pipe name. It also keeps the username out
/// of pipe listings. Direct pipe names from `COPYPASTE_SOCKET` pass through.
pub fn name_for(path: &Path) -> OsString {
    let raw = path.as_os_str().to_string_lossy();
    if raw
        .get(..LOCAL_PIPES.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(LOCAL_PIPES))
    {
        return path.as_os_str().to_os_string();
    }
    let digest = Sha256::digest(raw.to_lowercase().as_bytes());
    OsString::from(format!(
        "{LOCAL_PIPES}copypaste.{}",
        hex::encode(&digest[..8])
    ))
}

/// Pipe names are machine-global, so another account can create ours first;
/// the server's process must run as this account before anything is sent.
pub async fn connect(path: &Path) -> io::Result<Stream> {
    let stream = Stream::connect_by_path(name_for(path)).await?;
    account::ensure_own_account(stream.server_process_id()?)?;
    Ok(stream)
}

/// The only `unsafe` in this crate: reading which account a process runs as.
#[allow(unsafe_code)]
mod account {
    use std::io;

    pub(super) fn ensure_own_account(pid: u32) -> io::Result<()> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Security::EqualSid;
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        // SAFETY: returns a pseudo-handle that needs no closing.
        let ours = token_user(unsafe { GetCurrentProcess() })?;
        // SAFETY: plain syscall; a null handle is checked below.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return Err(super::super::foreign_endpoint());
        }
        let theirs = token_user(process);
        // SAFETY: `process` is a handle this function owns.
        unsafe { CloseHandle(process) };
        let theirs = theirs.map_err(|_| super::super::foreign_endpoint())?;
        // SAFETY: both pointers come from live TOKEN_USER buffers.
        if unsafe { EqualSid(ours.sid(), theirs.sid()) } == 0 {
            return Err(super::super::foreign_endpoint());
        }
        Ok(())
    }

    /// A `TOKEN_USER` buffer, `u64`-backed for the struct's pointer alignment.
    struct TokenUser(Vec<u64>);

    impl TokenUser {
        fn sid(&self) -> windows_sys::Win32::Security::PSID {
            // SAFETY: the buffer was filled by GetTokenInformation(TokenUser).
            unsafe {
                (*self
                    .0
                    .as_ptr()
                    .cast::<windows_sys::Win32::Security::TOKEN_USER>())
                .User
                .Sid
            }
        }
    }

    fn token_user(process: windows_sys::Win32::Foundation::HANDLE) -> io::Result<TokenUser> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Security::{GetTokenInformation, TokenUser as Class, TOKEN_QUERY};
        use windows_sys::Win32::System::Threading::OpenProcessToken;

        let mut token = std::ptr::null_mut();
        // SAFETY: `token` is a valid out pointer.
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0u32;
        // SAFETY: a size query with no buffer.
        unsafe { GetTokenInformation(token, Class, std::ptr::null_mut(), 0, &mut len) };
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: `buf` holds at least `len` bytes.
        let ok =
            unsafe { GetTokenInformation(token, Class, buf.as_mut_ptr().cast(), len, &mut len) };
        let err = io::Error::last_os_error();
        // SAFETY: `token` is a handle this function owns.
        unsafe { CloseHandle(token) };
        if ok == 0 {
            return Err(err);
        }
        Ok(TokenUser(buf))
    }
}

/// A private connected pair for transport tests.
pub async fn pair() -> io::Result<(Stream, Stream)> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "{LOCAL_PIPES}copypaste.pair.{}.{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );

    let listener = PipeListenerOptions::new()
        .path(name.as_str())
        .create_tokio_duplex::<Bytes>()?;
    let client = Stream::connect_by_path(name.as_str()).await?;
    let server = listener.accept().await?;
    Ok((server, client))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn two_data_directories_never_share_one_pipe() {
        let a = name_for(Path::new(
            r"C:\Users\ann\AppData\Roaming\CopyPaste\daemon.sock",
        ));
        let b = name_for(Path::new(
            r"C:\Users\bob\AppData\Roaming\CopyPaste\daemon.sock",
        ));
        assert_ne!(a, b, "one pipe name for two users is one daemon for two");
        assert!(a.to_string_lossy().starts_with(LOCAL_PIPES), "{a:?}");
    }

    #[test]
    fn one_path_always_gives_one_name() {
        let path = PathBuf::from(r"C:\Users\ann\AppData\Roaming\CopyPaste\daemon.sock");
        assert_eq!(name_for(&path), name_for(&path));
        assert_eq!(
            name_for(&path),
            name_for(Path::new(
                r"c:\users\ANN\appdata\roaming\copypaste\DAEMON.SOCK"
            )),
            "Windows compares paths without case; the two ends must agree"
        );
    }

    #[test]
    fn a_pipe_name_is_taken_as_it_stands() {
        let named = Path::new(r"\\.\pipe\copypaste-test-verbatim");
        assert_eq!(name_for(named), named.as_os_str());
        assert_eq!(
            name_for(Path::new(r"\\.\PIPE\copypaste-test-verbatim")),
            Path::new(r"\\.\PIPE\copypaste-test-verbatim").as_os_str(),
            "the prefix is a Windows name, so its case cannot decide anything"
        );
    }

    #[test]
    fn the_name_does_not_carry_the_path_it_came_from() {
        let name = name_for(Path::new(
            r"C:\Users\ann\AppData\Roaming\CopyPaste\daemon.sock",
        ));
        let name = name.to_string_lossy().to_lowercase();
        assert!(!name.contains("ann"), "{name}");
        assert!(!name.contains("appdata"), "{name}");
    }

    #[tokio::test]
    async fn a_pair_carries_bytes_both_ways() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut server, mut client) = pair().await.expect("a pipe pair");
        client.write_all(b"ping\n").await.unwrap();
        let mut got = [0u8; 5];
        server.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"ping\n");

        server.write_all(b"pong\n").await.unwrap();
        let mut got = [0u8; 5];
        client.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"pong\n");
    }

    #[tokio::test]
    async fn dialling_a_pipe_nobody_bound_fails_rather_than_waiting() {
        let err = connect(Path::new(r"C:\nowhere\copypaste-unbound\daemon.sock"))
            .await
            .expect_err("no daemon, no pipe");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
