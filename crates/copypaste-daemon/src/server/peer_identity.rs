//! Whether the peer on an IPC connection is the CopyPaste app.
//!
//! Privileged ops (`OpGates::privileged`) are served only to the app. The peer
//! handle is taken at accept; the check runs off the reactor on the first
//! privileged request and holds for the connection, so CLI connections pay
//! one cheap syscall. macOS: the audit token's code must meet the daemon's own
//! designated requirement with the app identifier. Windows: the client image
//! must be the app next to the daemon, signed by the daemon's signer. Linux:
//! `/proc/<pid>/exe` must be the app next to the daemon (weaker: no signature,
//! pids are reused). Unsigned daemons fall back to that path check, plus
//! `COPYPASTE_DEV_APP_PATH` in debug. Not a boundary against same-user code.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use copypaste_ipc::transport::Stream;
use copypaste_ipc::Method;
use tracing::debug;

use super::dispatch::CallerIdentity;

#[cfg(windows)]
use self::windows as platform;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
use other as platform;

/// The app executable's file name, next to the daemon in every install.
#[cfg(target_os = "macos")]
const APP_BINARY: &str = "CopyPaste";
#[cfg(windows)]
const APP_BINARY: &str = "CopyPaste.exe";
#[cfg(not(any(target_os = "macos", windows)))]
const APP_BINARY: &str = "copypaste";

/// One connection's peer and, once asked, whether it is the app.
pub(super) struct PeerIdentity {
    handle: Option<platform::PeerHandle>,
    verdict: Option<bool>,
}

impl PeerIdentity {
    /// Take the peer handle before the stream is split. Never fails: a peer
    /// that cannot be identified is simply not the app.
    pub(super) fn capture(stream: &Stream) -> Self {
        Self {
            handle: platform::capture(stream),
            verdict: None,
        }
    }

    /// The identity to gate `method` with.
    pub(super) async fn caller(&mut self, method: &Method) -> CallerIdentity {
        self.caller_with(method, |handle| handle.is_some_and(platform::is_app))
            .await
    }

    async fn caller_with(
        &mut self,
        method: &Method,
        verify: fn(Option<platform::PeerHandle>) -> bool,
    ) -> CallerIdentity {
        if self.verdict.is_none() && method.privileged() {
            let handle = self.handle.take();
            let is_app = tokio::task::spawn_blocking(move || verify(handle))
                .await
                .unwrap_or(false);
            debug!(is_app, "ipc peer identity settled");
            self.verdict = Some(is_app);
        }
        CallerIdentity {
            is_app: self.verdict == Some(true),
        }
    }
}

/// What a peer must be to count as the app, settled once per process.
enum Expected {
    #[cfg(target_os = "macos")]
    Requirement(String),
    #[cfg(windows)]
    Signer(Vec<u8>),
    Path(Vec<PathBuf>),
    Nobody,
}

fn expected() -> &'static Expected {
    static EXPECTED: OnceLock<Expected> = OnceLock::new();
    EXPECTED.get_or_init(platform::expected)
}

/// The app executable beside the daemon, and in a debug build the one a
/// developer named. Canonical, so a symlinked install still compares equal.
fn app_paths(allow_dev: bool) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if allow_dev && cfg!(debug_assertions) {
        paths.extend(std::env::var_os("COPYPASTE_DEV_APP_PATH").map(PathBuf::from));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(dir.join(APP_BINARY));
        #[cfg(target_os = "macos")]
        paths.extend(macos::bundle_of(&dir));
    }
    paths
        .into_iter()
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .collect()
}

fn is_app_path(path: &Path) -> bool {
    let Expected::Path(paths) = expected() else {
        return false;
    };
    std::fs::canonicalize(path).is_ok_and(|path| paths.contains(&path))
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos {
    use std::ffi::c_void;
    use std::os::fd::{AsFd, AsRawFd};
    use std::path::{Path, PathBuf};

    use copypaste_ipc::transport::Stream;
    use core_foundation::base::TCFType;
    use core_foundation::data::CFData;
    use core_foundation::string::CFString;
    use security_framework::os::macos::code_signing::{
        Flags, GuestAttributes, SecCode, SecRequirement,
    };

    use super::{app_paths, is_app_path, Expected};

    /// The bundle identifier the app is signed with (`selfsign.sh`).
    const APP_IDENTIFIER: &str = "com.copypaste.app";
    const SOL_LOCAL: i32 = 0;
    const LOCAL_PEERTOKEN: i32 = 6;
    const ERR_SEC_CS_UNSIGNED: i32 = -67062;

    /// The peer's `audit_token_t`: unlike a pid, it cannot name a later process.
    pub(super) struct PeerHandle([u32; 8]);

    extern "C" {
        fn getsockopt(fd: i32, level: i32, name: i32, value: *mut c_void, len: *mut u32) -> i32;
    }

    #[link(name = "Security", kind = "framework")]
    extern "C" {
        fn SecCodeCopyDesignatedRequirement(
            code: *const c_void,
            flags: u32,
            requirement: *mut *const c_void,
        ) -> i32;
        fn SecRequirementCopyString(
            requirement: *const c_void,
            flags: u32,
            text: *mut *const c_void,
        ) -> i32;
    }

    pub(super) fn capture(stream: &Stream) -> Option<PeerHandle> {
        let mut token = [0u32; 8];
        let mut len = std::mem::size_of_val(&token) as u32;
        // SAFETY: `token` is a writable buffer of `len` bytes and the fd is live
        // for the duration of the borrow.
        let status = unsafe {
            getsockopt(
                stream.as_fd().as_raw_fd(),
                SOL_LOCAL,
                LOCAL_PEERTOKEN,
                token.as_mut_ptr().cast(),
                &mut len,
            )
        };
        (status == 0 && len as usize == std::mem::size_of_val(&token)).then_some(PeerHandle(token))
    }

    pub(super) fn guest(handle: &PeerHandle) -> Option<SecCode> {
        let bytes: Vec<u8> = handle
            .0
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        let token = CFData::from_buffer(&bytes);
        let mut attributes = GuestAttributes::new();
        attributes.set_audit_token(token.as_concrete_TypeRef());
        SecCode::copy_guest_with_attribues(None, &attributes, Flags::NONE).ok()
    }

    pub(super) fn is_app(handle: PeerHandle) -> bool {
        let Some(code) = guest(&handle) else {
            return false;
        };
        match super::expected() {
            Expected::Requirement(text) => text
                .parse::<SecRequirement>()
                .and_then(|requirement| code.check_validity(Flags::NONE, &requirement))
                .is_ok(),
            // Ad-hoc identifiers prove nothing alone; with the path they keep
            // other executables inside the bundle out.
            Expected::Path(_) => {
                format!("identifier \"{APP_IDENTIFIER}\"")
                    .parse::<SecRequirement>()
                    .and_then(|requirement| code.check_validity(Flags::NONE, &requirement))
                    .is_ok()
                    && code
                        .path(Flags::NONE)
                        .ok()
                        .and_then(|url| url.to_path())
                        .is_some_and(|path| is_app_path(&path))
            }
            Expected::Nobody => false,
        }
    }

    pub(super) fn expected() -> Expected {
        match own_requirement() {
            Ok(Some(own)) if !own.starts_with("cdhash ") => {
                super::app_requirement(&own, APP_IDENTIFIER)
                    .map_or(Expected::Nobody, Expected::Requirement)
            }
            Ok(_) => Expected::Path(app_paths(true)),
            Err(()) => Expected::Nobody,
        }
    }

    /// `CopyPaste.app` when the daemon runs from its `Contents/MacOS`: the
    /// code-signing path of a running app is its bundle.
    pub(super) fn bundle_of(dir: &Path) -> Option<PathBuf> {
        let contents = dir.parent().filter(|_| dir.ends_with("Contents/MacOS"))?;
        contents.parent().map(Path::to_path_buf)
    }

    /// The daemon's own designated requirement; `None` when it is unsigned.
    pub(super) fn own_requirement() -> Result<Option<String>, ()> {
        let code = SecCode::for_self(Flags::NONE).map_err(drop)?;
        let mut requirement = std::ptr::null();
        // SAFETY: a `SecCodeRef` is accepted where a static code is expected,
        // and the out pointer is written only on success.
        let status = unsafe {
            SecCodeCopyDesignatedRequirement(
                code.as_concrete_TypeRef().cast_const().cast(),
                0,
                &mut requirement,
            )
        };
        if status == ERR_SEC_CS_UNSIGNED {
            return Ok(None);
        }
        if status != 0 || requirement.is_null() {
            return Err(());
        }
        // SAFETY: a +1 reference returned by a Copy function.
        let requirement =
            unsafe { SecRequirement::wrap_under_create_rule(requirement.cast_mut().cast()) };
        let mut text = std::ptr::null();
        // SAFETY: as above; the requirement outlives the call.
        let status = unsafe {
            SecRequirementCopyString(
                requirement.as_concrete_TypeRef().cast_const().cast(),
                0,
                &mut text,
            )
        };
        if status != 0 || text.is_null() {
            return Err(());
        }
        // SAFETY: a +1 reference returned by a Copy function.
        let text = unsafe { CFString::wrap_under_create_rule(text.cast()) };
        Ok(Some(text.to_string()))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use copypaste_ipc::transport::Stream;

    use super::{app_paths, is_app_path, Expected};

    pub(super) struct PeerHandle(i32);

    pub(super) fn capture(stream: &Stream) -> Option<PeerHandle> {
        stream.peer_pid().ok().flatten().map(PeerHandle)
    }

    pub(super) fn is_app(handle: PeerHandle) -> bool {
        std::fs::read_link(format!("/proc/{}/exe", handle.0)).is_ok_and(|exe| is_app_path(&exe))
    }

    pub(super) fn expected() -> Expected {
        let paths = app_paths(true);
        if paths.is_empty() {
            Expected::Nobody
        } else {
            Expected::Path(paths)
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::{Path, PathBuf};

    use copypaste_ipc::transport::Stream;
    use windows_sys::Win32::Security::WinTrust::{
        WTHelperGetProvCertFromChain, WTHelperGetProvSignerFromChain,
        WTHelperProvDataFromStateData, WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2,
        WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL,
        WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_NONE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    use super::{app_paths, is_app_path, Expected};

    /// `TRUST_E_NOSIGNATURE`: the file carries no Authenticode signature.
    const TRUST_E_NOSIGNATURE: i32 = 0x800B_0100_u32 as i32;

    /// An open handle to the client process, so its pid cannot be reused
    /// between accept and verification.
    pub(super) struct PeerHandle(OwnedHandle);

    pub(super) fn capture(stream: &Stream) -> Option<PeerHandle> {
        let pid = stream.client_process_id().ok()?;
        // SAFETY: plain syscall; a null handle is checked below.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        // SAFETY: a fresh handle this function owns from here on.
        (!process.is_null()).then(|| PeerHandle(unsafe { OwnedHandle::from_raw_handle(process) }))
    }

    pub(super) fn is_app(handle: PeerHandle) -> bool {
        let Some(image) = image_path(&handle) else {
            return false;
        };
        match super::expected() {
            Expected::Signer(subject) => {
                signer_subject(&image).is_ok_and(|theirs| theirs == *subject)
                    && std::fs::canonicalize(&image)
                        .is_ok_and(|image| app_paths(false).contains(&image))
            }
            Expected::Path(_) => is_app_path(&image),
            Expected::Nobody => false,
        }
    }

    pub(super) fn expected() -> Expected {
        let Ok(own) = std::env::current_exe() else {
            return Expected::Nobody;
        };
        match signer_subject(&own) {
            Ok(subject) => Expected::Signer(subject),
            Err(TRUST_E_NOSIGNATURE) => Expected::Path(app_paths(true)),
            Err(_) => Expected::Nobody,
        }
    }

    fn image_path(handle: &PeerHandle) -> Option<PathBuf> {
        let mut buffer = vec![0u16; 32_768];
        let mut len = buffer.len() as u32;
        // SAFETY: `buffer` holds `len` UTF-16 units and the handle is live.
        let ok = unsafe {
            QueryFullProcessImageNameW(
                handle.0.as_raw_handle(),
                PROCESS_NAME_WIN32,
                buffer.as_mut_ptr(),
                &mut len,
            )
        };
        (ok != 0).then(|| PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
    }

    /// The DER subject of the certificate that signed `path`, once
    /// `WinVerifyTrust` accepts its signature; the `HRESULT` otherwise.
    fn signer_subject(path: &Path) -> Result<Vec<u8>, i32> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let mut file = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: wide.as_ptr(),
            ..Default::default()
        };
        let mut data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
            dwStateAction: WTD_STATEACTION_VERIFY,
            dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL | WTD_REVOCATION_CHECK_NONE,
            ..Default::default()
        };
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        // SAFETY: `data` and the file info it points to outlive both calls; the
        // state opened by VERIFY is closed below on every path.
        let status =
            unsafe { WinVerifyTrust(std::ptr::null_mut(), &mut action, (&raw mut data).cast()) };
        let subject = (status == 0).then(|| leaf_subject(&data)).flatten();
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        // SAFETY: as above.
        unsafe { WinVerifyTrust(std::ptr::null_mut(), &mut action, (&raw mut data).cast()) };
        match (status, subject) {
            (0, Some(subject)) => Ok(subject),
            (0, None) => Err(-1),
            (status, _) => Err(status),
        }
    }

    fn leaf_subject(data: &WINTRUST_DATA) -> Option<Vec<u8>> {
        // SAFETY: every pointer comes from the provider state of a successful
        // verification that is still open, and each is null-checked.
        unsafe {
            let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
            if provider.is_null() {
                return None;
            }
            let signer = WTHelperGetProvSignerFromChain(provider, 0, 0, 0);
            if signer.is_null() {
                return None;
            }
            let cert = WTHelperGetProvCertFromChain(signer, 0);
            if cert.is_null() || (*cert).pCert.is_null() || (*(*cert).pCert).pCertInfo.is_null() {
                return None;
            }
            let subject = (*(*(*cert).pCert).pCertInfo).Subject;
            if subject.pbData.is_null() {
                return None;
            }
            Some(std::slice::from_raw_parts(subject.pbData, subject.cbData as usize).to_vec())
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod other {
    use copypaste_ipc::transport::Stream;

    use super::Expected;

    pub(super) struct PeerHandle;

    pub(super) fn capture(_stream: &Stream) -> Option<PeerHandle> {
        None
    }

    pub(super) fn is_app(_handle: PeerHandle) -> bool {
        false
    }

    pub(super) fn expected() -> Expected {
        Expected::Nobody
    }
}

/// `own` with its `identifier` clause naming `identifier` instead: the same
/// anchor and certificate constraints, for the app rather than the daemon.
#[cfg(any(target_os = "macos", test))]
fn app_requirement(own: &str, identifier: &str) -> Option<String> {
    const KEYWORD: &str = "identifier ";
    let start = own.match_indices(KEYWORD).find_map(|(at, _)| {
        let boundary = own[..at]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace() || c == '(');
        boundary.then_some(at)
    })?;
    let value = &own[start + KEYWORD.len()..];
    let consumed = if let Some(quoted) = value.strip_prefix('"') {
        quoted.find('"')? + 2
    } else {
        value
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(value.len())
    };
    if consumed == 0 {
        return None;
    }
    let rest = &value[consumed..];
    Some(format!("{}{KEYWORD}\"{identifier}\"{rest}", &own[..start]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn the_app_requirement_keeps_every_constraint_but_the_identifier() {
        let own = "identifier \"com.copypaste.copypaste-daemon\" and certificate root = H\"ef8395c27da7ea04aff3c2e06d3f57b6c7bbdc14\"";
        assert_eq!(
            app_requirement(own, "com.copypaste.app").as_deref(),
            Some("identifier \"com.copypaste.app\" and certificate root = H\"ef8395c27da7ea04aff3c2e06d3f57b6c7bbdc14\"")
        );
        let bare = "(identifier copypaste-daemon) and anchor apple generic";
        assert_eq!(
            app_requirement(bare, "com.copypaste.app").as_deref(),
            Some("(identifier \"com.copypaste.app\") and anchor apple generic")
        );
        #[cfg(target_os = "macos")]
        for text in [own, bare] {
            let app = app_requirement(text, "com.copypaste.app").unwrap();
            let parsed = app.parse::<security_framework::os::macos::code_signing::SecRequirement>();
            assert!(parsed.is_ok(), "{app}");
        }
        assert_eq!(app_requirement("anchor apple", "com.copypaste.app"), None);
        assert_eq!(app_requirement("myidentifier x", "com.copypaste.app"), None);
    }

    /// The check is paid once per connection, and only by one that asks for a
    /// privileged operation.
    #[tokio::test]
    async fn identity_is_settled_once_per_connection() {
        static CHECKS: AtomicUsize = AtomicUsize::new(0);
        fn verify(_: Option<platform::PeerHandle>) -> bool {
            CHECKS.fetch_add(1, Ordering::SeqCst);
            true
        }
        let unhide = Method::Clips {
            op: copypaste_ipc::clips::ClipOp::SetHiddenFromIntegrations {
                ids: vec!["a".into()],
                hidden: false,
            },
        };
        let mut peer = PeerIdentity {
            handle: None,
            verdict: None,
        };
        assert!(!peer.caller_with(&Method::Status, verify).await.is_app);
        assert_eq!(CHECKS.load(Ordering::SeqCst), 0);
        assert!(peer.caller_with(&unhide, verify).await.is_app);
        assert!(peer.caller_with(&unhide, verify).await.is_app);
        assert!(peer.caller_with(&Method::Status, verify).await.is_app);
        assert_eq!(CHECKS.load(Ordering::SeqCst), 1);
    }

    /// The test binary dials itself: a real peer, and not the app.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_peer_that_is_not_the_app_is_refused() {
        let (server, _client) = Stream::pair().unwrap();
        let mut peer = PeerIdentity::capture(&server);
        assert!(peer.handle.is_some(), "the peer handle is readable");
        let unhide = Method::Clips {
            op: copypaste_ipc::clips::ClipOp::SetHiddenFromIntegrations {
                ids: vec!["a".into()],
                hidden: false,
            },
        };
        assert!(!peer.caller(&unhide).await.is_app);
    }

    /// The audit token names running code that a requirement can be checked
    /// against: the process meets its own designated requirement.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn the_audit_token_resolves_to_checkable_code() {
        use security_framework::os::macos::code_signing::{Flags, SecRequirement};
        let (server, _client) = Stream::pair().unwrap();
        let handle = macos::capture(&server).expect("an audit token");
        let code = macos::guest(&handle).expect("the peer's code");
        let Ok(Some(own)) = macos::own_requirement() else {
            return;
        };
        let requirement: SecRequirement = own.parse().unwrap();
        code.check_validity(Flags::NONE, &requirement)
            .expect("the process meets its own requirement");
    }
}
