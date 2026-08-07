//! Binding the daemon's pipe, and the list that decides who may open it.
//!
//! What the Unix half of [`super::listener`] does before it can accept — a
//! `0700` directory, a `0600` mode set before the path is reachable, an
//! `flock` around probe-remove-bind — a pipe reaches otherwise. Its ACL is
//! fixed at creation, so there is no window at a wider one; and
//! `FILE_FLAG_FIRST_PIPE_INSTANCE` *is* the exclusion, refused by the kernel
//! rather than agreed between two starters through a lock file. Nothing
//! survives the process either, so there is no stale endpoint to clear.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

use copypaste_ipc::transport::{pipe, Listener};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tracing::info;
use windows_sys::core::PWSTR;
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Create the pipe, restricted to this account, and refuse to be the second
/// daemon on it.
pub fn bind(path: &Path) -> anyhow::Result<Listener> {
    let name = pipe::name_for(path);
    let security = Arc::new(owner_only()?);

    let first = instance(&name, &security, true).map_err(|e| {
        // The kernel refuses a second `FIRST_PIPE_INSTANCE` with
        // `ERROR_ACCESS_DENIED`, which is this platform's spelling of what the
        // Unix side concludes when it finds a live socket: somebody else has
        // the endpoint, and two daemons on one database is a data-loss shape.
        if e.kind() == io::ErrorKind::PermissionDenied {
            anyhow::anyhow!("another copypaste daemon is already listening")
        } else {
            anyhow::Error::new(e).context("create the daemon pipe")
        }
    })?;

    let next: pipe::Instances = {
        let name = name.clone();
        let security = Arc::clone(&security);
        Arc::new(move || instance(&name, &security, false))
    };

    info!("ipc pipe listening");
    Ok(pipe::Listener::new(first, next).into())
}

/// One instance of the pipe.
fn instance(name: &OsStr, security: &OwnerOnly, first: bool) -> io::Result<NamedPipeServer> {
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };

    // SAFETY: `attributes` is fully initialised, outlives the call, and its
    // descriptor is owned by `security`, which outlives every instance.
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            // tokio's default, stated because it is load-bearing: a remote
            // caller is one the ACL below was never asked about.
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(name, ptr::from_mut(&mut attributes).cast())
    }
}

/// The descriptor every instance is created with, and the `LocalFree` it needs.
struct OwnerOnly(PSECURITY_DESCRIPTOR);

// SAFETY: built once and only read afterwards — Windows reads it while creating
// an instance and never writes through the pointer, and the `Arc` sharing it
// hands out no `&mut`.
unsafe impl Send for OwnerOnly {}
unsafe impl Sync for OwnerOnly {}

impl Drop for OwnerOnly {
    fn drop(&mut self) {
        // SAFETY: the pointer came from the SDDL parser, which allocates with
        // `LocalAlloc`, and nothing else frees it.
        unsafe { LocalFree(self.0.cast()) };
    }
}

fn owner_only() -> anyhow::Result<OwnerOnly> {
    let text: Vec<u16> = sddl(&current_user_sid()?)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();

    // SAFETY: `text` is NUL-terminated and outlives the call; `descriptor` is
    // the out-parameter the parser allocates into. The size out-parameter is
    // optional and null.
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    };
    if ok == 0 || descriptor.is_null() {
        anyhow::bail!("could not build the daemon pipe's access list");
    }
    Ok(OwnerOnly(descriptor))
}

/// One allow-everything entry for the account the daemon runs as, in a DACL
/// marked protected so nothing is inherited into it.
///
/// Neither `SY` nor `BA` appears, deliberately. This is the `0600` mode's
/// meaning rather than root's: what a machine administrator can reach anyway is
/// the platform's business, and naming them here would only widen what an
/// unelevated process in another account may open. The pipe is the only
/// authentication boundary (manifest 04 I14), so nothing else grants entry.
fn sddl(sid: &str) -> String {
    format!("D:P(A;;GA;;;{sid})")
}

fn current_user_sid() -> anyhow::Result<String> {
    let token = ProcessToken::open()?;
    let user = token.user()?;

    // SAFETY: `user` holds a `TOKEN_USER` written by `GetTokenInformation`, is
    // aligned for it, and the SID it names lives in the same buffer.
    let sid = unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid };

    let mut raw: PWSTR = ptr::null_mut();
    // SAFETY: `sid` points into `user`, which outlives the call.
    let ok = unsafe { ConvertSidToStringSidW(sid, &mut raw) };
    if ok == 0 || raw.is_null() {
        anyhow::bail!("could not read this account's identifier");
    }

    // SAFETY: the API wrote a NUL-terminated wide string it owns until
    // `LocalFree`, and the copy is taken before that.
    let sid = unsafe {
        let mut len = 0;
        while *raw.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(raw, len))
    };
    // SAFETY: freeing what the call above allocated, once, after the copy.
    unsafe { LocalFree(raw.cast()) };
    Ok(sid)
}

struct ProcessToken(HANDLE);

impl ProcessToken {
    fn open() -> anyhow::Result<Self> {
        let mut handle: HANDLE = ptr::null_mut();
        // SAFETY: `handle` is the out-parameter. `GetCurrentProcess` returns a
        // pseudo-handle that needs no closing.
        let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) };
        if ok == 0 || handle.is_null() {
            anyhow::bail!("could not open this process's token");
        }
        Ok(Self(handle))
    }

    /// The `TOKEN_USER` and the SID it points at, in one buffer.
    ///
    /// `Vec<u64>` rather than `Vec<u8>`: the cast in [`current_user_sid`] needs
    /// the buffer aligned for a structure of pointers, and a byte vector
    /// promises only alignment 1.
    fn user(&self) -> anyhow::Result<Vec<u64>> {
        let mut needed: u32 = 0;
        // SAFETY: a null buffer of length zero is how this API is asked for the
        // size it wants; it fails, and `needed` is the answer.
        unsafe { GetTokenInformation(self.0, TokenUser, ptr::null_mut(), 0, &mut needed) };
        if needed == 0 {
            anyhow::bail!("could not size this account's identifier");
        }

        let mut buffer = vec![0u64; (needed as usize).div_ceil(size_of::<u64>())];
        // SAFETY: `buffer` is at least `needed` bytes and aligned for the
        // structure the API writes into it.
        let ok = unsafe {
            GetTokenInformation(
                self.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        };
        if ok == 0 {
            anyhow::bail!("could not read this account's identifier");
        }
        Ok(buffer)
    }
}

impl Drop for ProcessToken {
    fn drop(&mut self) {
        // SAFETY: opened by `OpenProcessToken` above and closed once.
        unsafe { CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_ipc::{transport, Method, Request, Response, PROTOCOL_VERSION};
    use futures_util::StreamExt;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio_util::codec::{FramedRead, LinesCodec};

    /// The whole of the access decision, in one string. A later "let the
    /// service reach it too" is a change to this test first.
    #[test]
    fn the_pipe_admits_the_owner_and_nobody_else() {
        let rule = sddl("S-1-5-21-1-2-3-1001");
        assert_eq!(rule, "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)");
        for wider in ["WD", "BA", "SY", "AU", "IU"] {
            assert!(!rule.contains(wider), "{rule} admits {wider}");
        }
    }

    #[test]
    fn this_account_has_an_identifier_the_parser_accepts() {
        let sid = current_user_sid().expect("a token this process owns");
        assert!(sid.starts_with("S-1-"), "{sid}");
        owner_only().expect("the descriptor must parse");
    }

    /// `CopyPaste-ah1m` on this platform. Two daemons on one database is the
    /// data-loss shape; here the kernel is what refuses, so what is under test
    /// is that the refusal is recognised rather than reported as a crash.
    // `bind` hands back a tokio listener, which needs a reactor on the thread
    // that creates it — as the socket's own race test says of `UnixListener`.
    #[tokio::test]
    async fn a_second_daemon_on_one_pipe_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("daemon.sock");

        let first = bind(&path).expect("the first daemon binds");
        let second = bind(&path).expect_err("the second must not");

        let shown = format!("{second:#}");
        assert!(shown.contains("already listening"), "{shown}");
        assert!(!shown.contains('\\'), "rule 4: no path in a user message");

        drop(first);
        // And the name frees with the process that held it, so a restart needs
        // no stale-endpoint cleanup at all.
        bind(&path).expect("the name is free once the listener is dropped");
    }

    /// Two data directories are two daemons, exactly as they are on Unix.
    #[tokio::test]
    async fn two_data_directories_bind_at_the_same_time() {
        let dir = tempfile::tempdir().unwrap();
        let one = bind(&dir.path().join("a/daemon.sock")).expect("bind");
        let two = bind(&dir.path().join("b/daemon.sock")).expect("bind");
        drop((one, two));
    }

    /// The point of the whole file: a real request over a real pipe, framed by
    /// the same codec the socket uses.
    #[tokio::test]
    async fn a_request_round_trips_over_the_pipe() {
        let (state, dir) = test_state("pipe");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        let stream = transport::connect(&path).await.expect("connect");
        let (reader, mut writer) = stream.into_split();
        let mut lines = FramedRead::new(reader, LinesCodec::new());
        let request = Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Status,
        };
        let mut line = serde_json::to_string(&request).unwrap();
        line.push('\n');
        writer.write_all(line.as_bytes()).await.expect("send");

        let line = tokio::time::timeout(Duration::from_secs(5), lines.next())
            .await
            .expect("a reply")
            .expect("a frame")
            .expect("a valid frame");
        let response: Response = serde_json::from_str(&line).unwrap();
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(response.id, 1);

        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }

    /// A second client while the first is still connected: the accept loop has
    /// to have replaced the instance it handed out, or this hangs.
    #[tokio::test]
    async fn a_second_client_connects_while_the_first_is_open() {
        let (state, dir) = test_state("pipe-second");
        let path = dir.path().join("daemon.sock");
        let listener = bind(&path).expect("bind");
        let server = tokio::spawn(super::super::run(
            listener,
            Arc::clone(&state),
            state.shutdown_rx(),
        ));

        let first = transport::connect(&path).await.expect("connect");
        let second = tokio::time::timeout(Duration::from_secs(5), transport::connect(&path))
            .await
            .expect("the next instance must already be listening")
            .expect("connect");

        drop((first, second));
        state.request_shutdown();
        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }
}
