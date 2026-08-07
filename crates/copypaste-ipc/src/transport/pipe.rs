//! The Windows half of the endpoint: a named pipe.
//!
//! Three of the socket's guarantees are reached by other means here.
//! Owner-only access is a DACL rather than a `0600` mode, and it is not built
//! in this crate — see [`Instances`]. One-daemon exclusion is
//! `FILE_FLAG_FIRST_PIPE_INSTANCE`, which the kernel enforces at creation, so
//! there is no probe, no critical section, and nothing stale to clear: no pipe
//! outlives the process that made it. And the endpoint has a name rather than
//! a path, so `\\.\pipe\` is machine-global where the data directory was
//! per-user — [`name_for`] is what puts that separation back.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer};
use tokio::sync::Mutex;
use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;

/// Where every local named pipe lives.
const LOCAL_PIPES: &str = r"\\.\pipe\";

/// How long a dialler waits before retrying a pipe whose instances are all
/// taken. The accept loop replaces an instance as it hands one out, so this is
/// a window of microseconds; the caller owns the deadline that bounds the loop.
const BUSY_RETRY: Duration = Duration::from_millis(50);

/// The pipe that stands in for the socket at `path`.
///
/// `\\.\pipe\` is machine-global where the data directory is per-user, so the
/// name has to re-separate what the path already separated: two users on one
/// machine, and the relocated directory `--data-dir` and the tests use. The
/// digest does that and nothing else. It is not a secret and does not try to
/// be — any local process can list `\\.\pipe\` — the DACL is the boundary
/// here, exactly as the `0600` mode is on Unix.
///
/// Lowercased first because Windows paths compare case-insensitively and the
/// two ends have to agree. A name already under `\\.\pipe\` is used verbatim,
/// which is how `COPYPASTE_SOCKET` names a pipe directly.
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

/// How the listener creates each further instance of its pipe.
///
/// The daemon supplies it because what an instance may be opened by is its
/// policy, and building the descriptor that says so needs `unsafe`, which this
/// crate forbids.
pub type Instances = Arc<dyn Fn() -> io::Result<NamedPipeServer> + Send + Sync>;

pub struct Listener {
    pending: Mutex<NamedPipeServer>,
    next: Instances,
}

/// Deliberately opaque: the name is derived from the socket path, and a `Debug`
/// that prints it puts the path back into logs (CLAUDE.md rule 4).
impl fmt::Debug for Listener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Listener").finish_non_exhaustive()
    }
}

impl Listener {
    pub fn new(first: NamedPipeServer, next: Instances) -> Self {
        Self {
            pending: Mutex::new(first),
            next,
        }
    }

    pub async fn accept(&self) -> io::Result<Stream> {
        let mut pending = self.pending.lock().await;
        pending.connect().await?;

        // The replacement is made *before* the connected instance is handed
        // out. A moment with no instance at all frees the name, and the next
        // process to create it gets `FIRST_PIPE_INSTANCE` — two daemons on one
        // database, which is the shape `flock` exists to prevent on Unix.
        match (self.next)() {
            Ok(replacement) => Ok(Stream::Server(std::mem::replace(
                &mut *pending,
                replacement,
            ))),
            Err(e) => {
                // Rather than serve a connection with no successor: the client
                // sees the pipe close and retries, which is what it already
                // does for a daemon that is not there.
                let _ = pending.disconnect();
                Err(e)
            }
        }
    }
}

/// One connection. Which end of the pipe it is decides its type, so this is two
/// types and not one.
#[derive(Debug)]
pub enum Stream {
    Server(NamedPipeServer),
    Client(NamedPipeClient),
}

/// Dial the pipe standing in for `path`.
pub async fn connect(path: &Path) -> io::Result<Stream> {
    let name = name_for(path);
    loop {
        match ClientOptions::new().open(&name) {
            Ok(client) => return Ok(Stream::Client(client)),
            // Every instance is busy for the instant between the accept loop
            // taking one and creating the next. Failing here would turn that
            // into an unreachable daemon; the caller's deadline bounds the wait.
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                tokio::time::sleep(BUSY_RETRY).await;
            }
            Err(e) => return Err(e),
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_read(cx, buf),
            Self::Client(pipe) => Pin::new(pipe).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_write(cx, buf),
            Self::Client(pipe) => Pin::new(pipe).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_flush(cx),
            Self::Client(pipe) => Pin::new(pipe).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(pipe) => Pin::new(pipe).poll_shutdown(cx),
            Self::Client(pipe) => Pin::new(pipe).poll_shutdown(cx),
        }
    }
}

/// A connected pair over a private pipe, for tests that need both ends without
/// a daemon.
///
/// Async where the Unix `socketpair` is not: the server end reads nothing until
/// `ConnectNamedPipe` has been through the reactor, even though the client has
/// already opened the pipe and the kernel calls the connection good.
pub async fn pair() -> io::Result<(Stream, Stream)> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::net::windows::named_pipe::ServerOptions;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "{LOCAL_PIPES}copypaste.pair.{}.{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );

    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)?;
    let client = ClientOptions::new().open(&name)?;
    server.connect().await?;
    Ok((Stream::Server(server), Stream::Client(client)))
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

    /// The name is what both ends compute independently, so it has to be a
    /// function of the path and nothing else — not of the process that asked.
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

    /// The digest is for separation, not secrecy — but it must not hand the
    /// username to anything that lists `\\.\pipe\` either.
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
