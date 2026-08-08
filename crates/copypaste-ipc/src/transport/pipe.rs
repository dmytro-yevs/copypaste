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

pub async fn connect(path: &Path) -> io::Result<Stream> {
    Stream::connect_by_path(name_for(path)).await
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
