//! The local IPC endpoint, as a type the daemon, the CLI and the app can all
//! name without knowing what it is underneath.
//!
//! Unix is a `0600` socket at [`socket_path`](crate::socket_path); Windows is a
//! named pipe, and [`pipe`] is where the two differ.
//!
//! **Binding is not here.** The daemon owns that policy — refusing to steal a
//! live endpoint, the `flock` critical section, the `0600` mode, and on Windows
//! the access list every pipe instance is created with. This module owns only
//! what a bound endpoint and a dialled connection *are*.

use std::io;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(windows)]
pub mod pipe;

/// What a caller is told when the platform has no transport.
///
/// It names no path: the socket path discloses the local username
/// (CLAUDE.md rule 4), and this string reaches a user through the CLI's and the
/// app's connect-failure paths.
pub const MSG_UNSUPPORTED: &str = "local IPC is not implemented on this platform yet";

/// The one failure a platform with no transport returns.
///
/// `io::ErrorKind::Unsupported` rather than a new error enum: every caller of
/// this module already handles `io::Error`, and a bespoke type would have to be
/// threaded through three crates to say one thing.
pub fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, MSG_UNSUPPORTED)
}

#[cfg(unix)]
mod imp {
    pub type Listener = tokio::net::UnixListener;
    pub type Stream = tokio::net::UnixStream;
    pub type ReadHalf = tokio::net::unix::OwnedReadHalf;
    pub type WriteHalf = tokio::net::unix::OwnedWriteHalf;
}

// A pipe has no `into_split` of its own, so the halves are `tokio::io::split`'s
// and the two platforms' half types are not the same shape.
#[cfg(windows)]
mod imp {
    pub type Listener = super::pipe::Listener;
    pub type Stream = super::pipe::Stream;
    pub type ReadHalf = tokio::io::ReadHalf<super::pipe::Stream>;
    pub type WriteHalf = tokio::io::WriteHalf<super::pipe::Stream>;
}

// `Infallible` is uninhabited, so no value of any type below can exist on a
// third platform and `match self.0 {}` proves each body dead rather than
// panicking in one.
#[cfg(not(any(unix, windows)))]
mod imp {
    pub type Listener = std::convert::Infallible;
    pub type Stream = std::convert::Infallible;
    pub type ReadHalf = std::convert::Infallible;
    pub type WriteHalf = std::convert::Infallible;
}

/// A bound endpoint, accepting connections.
#[derive(Debug)]
pub struct Listener(imp::Listener);

/// One accepted or dialled connection.
#[derive(Debug)]
pub struct Stream(imp::Stream);

/// The read half of a [`Stream`], owned rather than borrowed so it can move
/// into the task serving it.
#[derive(Debug)]
pub struct OwnedReadHalf(imp::ReadHalf);

/// The write half of a [`Stream`].
#[derive(Debug)]
pub struct OwnedWriteHalf(imp::WriteHalf);

/// Wrap a listener the daemon has already bound and locked down.
#[cfg(unix)]
impl From<tokio::net::UnixListener> for Listener {
    fn from(listener: tokio::net::UnixListener) -> Self {
        Self(listener)
    }
}

/// As above: the daemon has already created the first instance, which is what
/// makes it the only daemon.
#[cfg(windows)]
impl From<pipe::Listener> for Listener {
    fn from(listener: pipe::Listener) -> Self {
        Self(listener)
    }
}

impl Listener {
    /// Wait for the next connection.
    pub async fn accept(&self) -> io::Result<Stream> {
        #[cfg(unix)]
        {
            self.0.accept().await.map(|(stream, _)| Stream(stream))
        }
        #[cfg(windows)]
        {
            self.0.accept().await.map(Stream)
        }
        #[cfg(not(any(unix, windows)))]
        {
            match self.0 {}
        }
    }
}

/// Dial the endpoint at `path`.
pub async fn connect(path: &Path) -> io::Result<Stream> {
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(path).await.map(Stream)
    }
    #[cfg(windows)]
    {
        pipe::connect(path).await.map(Stream)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(unsupported())
    }
}

impl Stream {
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        #[cfg(unix)]
        {
            let (reader, writer) = self.0.into_split();
            (OwnedReadHalf(reader), OwnedWriteHalf(writer))
        }
        #[cfg(windows)]
        {
            let (reader, writer) = tokio::io::split(self.0);
            (OwnedReadHalf(reader), OwnedWriteHalf(writer))
        }
        #[cfg(not(any(unix, windows)))]
        {
            match self.0 {}
        }
    }

    /// A connected pair, for tests that need both ends without a bound
    /// endpoint.
    #[cfg(unix)]
    pub fn pair() -> io::Result<(Self, Self)> {
        let (a, b) = tokio::net::UnixStream::pair()?;
        Ok((Self(a), Self(b)))
    }

    /// As above. Async because a pipe pair is only connected once
    /// `ConnectNamedPipe` has been through the reactor, and `socketpair` needs
    /// no such step.
    #[cfg(windows)]
    pub async fn pair() -> io::Result<(Self, Self)> {
        let (a, b) = pipe::pair().await?;
        Ok((Self(a), Self(b)))
    }
}

macro_rules! delegate_read {
    ($t:ty) => {
        impl AsyncRead for $t {
            fn poll_read(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut ReadBuf<'_>,
            ) -> Poll<io::Result<()>> {
                #[cfg(any(unix, windows))]
                {
                    Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
                }
                #[cfg(not(any(unix, windows)))]
                {
                    let _ = (cx, buf);
                    match self.0 {}
                }
            }
        }
    };
}

macro_rules! delegate_write {
    ($t:ty) => {
        impl AsyncWrite for $t {
            fn poll_write(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<io::Result<usize>> {
                #[cfg(any(unix, windows))]
                {
                    Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
                }
                #[cfg(not(any(unix, windows)))]
                {
                    let _ = (cx, buf);
                    match self.0 {}
                }
            }

            fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                #[cfg(any(unix, windows))]
                {
                    Pin::new(&mut self.get_mut().0).poll_flush(cx)
                }
                #[cfg(not(any(unix, windows)))]
                {
                    let _ = cx;
                    match self.0 {}
                }
            }

            fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                #[cfg(any(unix, windows))]
                {
                    Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
                }
                #[cfg(not(any(unix, windows)))]
                {
                    let _ = cx;
                    match self.0 {}
                }
            }
        }
    };
}

delegate_read!(Stream);
delegate_write!(Stream);
delegate_read!(OwnedReadHalf);
delegate_write!(OwnedWriteHalf);

#[cfg(test)]
mod tests {
    use super::*;

    /// The message reaches users, so rule 4 applies to it whether or not a
    /// platform is currently returning it.
    #[test]
    fn the_unsupported_failure_names_no_path() {
        let err = unsupported();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
        let shown = err.to_string();
        assert!(!shown.contains('/') && !shown.contains('\\'), "{shown}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_bound_endpoint_round_trips_bytes_through_the_seam() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sock");
        let listener: Listener = tokio::net::UnixListener::bind(&path).unwrap().into();

        let dialled = tokio::spawn(async move {
            let mut client = connect(&path).await.unwrap();
            client.write_all(b"ping\n").await.unwrap();
        });

        let accepted = listener.accept().await.unwrap();
        let (mut reader, _writer) = accepted.into_split();
        let mut got = [0u8; 5];
        reader.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"ping\n");
        dialled.await.unwrap();
    }

    /// The halves are `tokio::io::split`'s here rather than the stream's own,
    /// so that they still carry bytes is worth one test of its own.
    #[cfg(windows)]
    #[tokio::test]
    async fn a_pair_round_trips_bytes_through_the_seam() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (server, client) = Stream::pair().await.unwrap();
        let (_reader, mut writer) = client.into_split();
        let (mut reader, _writer) = server.into_split();

        writer.write_all(b"ping\n").await.unwrap();
        let mut got = [0u8; 5];
        reader.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"ping\n");
    }

    #[cfg(not(any(unix, windows)))]
    #[tokio::test]
    async fn dialling_reports_the_platform_rather_than_a_missing_endpoint() {
        let err = connect(Path::new("anything")).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }
}
