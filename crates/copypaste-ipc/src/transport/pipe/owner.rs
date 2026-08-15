//! Prove the pipe on the other end belongs to this account.
//!
//! `\\.\pipe\` is machine-global and any authenticated local user may create a
//! name in it, while the endpoint name is a digest of a predictable path. A
//! second account that binds the name first receives everything the client
//! sends — including `Method::CloudSignIn`'s password and sync passphrase. On
//! Unix the `0600` socket carries this guarantee; on Windows only the server
//! half was ported (ADR-0013:40, manifest 04 I14), so the client checks the
//! owner itself before writing a byte.

use std::io;
use std::os::windows::io::{AsHandle, AsRawHandle};
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, TokenOwner, TokenUser, OWNER_SECURITY_INFORMATION, PSID,
    TOKEN_OWNER, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Names no path and no account: it reaches the user through the CLI's and the
/// app's connect-failure paths, and the socket path discloses the username.
pub const MSG_FOREIGN_OWNER: &str =
    "the local IPC endpoint belongs to another account and was not trusted";

/// Every failure answers the same way, because none of them proves the peer is
/// ours. A query that cannot run is not permission to talk (rule 4).
fn refused() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, MSG_FOREIGN_OWNER)
}

/// `Ok` only when the pipe's owner is an identity this process would itself
/// create objects under.
pub fn verify(stream: &impl AsHandle) -> io::Result<()> {
    let handle = stream.as_handle().as_raw_handle() as HANDLE;
    let ours = mine()?;

    let mut owner: PSID = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: `handle` outlives the call through the borrow. `owner` is a
    // pointer into `descriptor`, so it is read only while that allocation is
    // live and never after the `LocalFree` below.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 || owner.is_null() {
        if !descriptor.is_null() {
            unsafe { LocalFree(descriptor.cast()) };
        }
        return Err(refused());
    }

    let matches = ours.iter().any(|&sid| unsafe { EqualSid(owner, sid) } != 0);
    unsafe { LocalFree(descriptor.cast()) };

    if matches {
        Ok(())
    } else {
        Err(refused())
    }
}

/// The SIDs an object this process created could legitimately carry.
///
/// Both, not just `TokenUser`: the owner stamped on a new kernel object comes
/// from `TokenOwner`, and for a member of the Administrators group that can be
/// the group rather than the user. Comparing against `TokenUser` alone would
/// refuse an elevated user's own daemon. Neither SID belongs to another
/// account, so accepting both narrows nothing that matters here.
struct Sids {
    _user: Buffer,
    _owner: Buffer,
    sids: Vec<PSID>,
}

impl Sids {
    fn iter(&self) -> impl Iterator<Item = &PSID> {
        self.sids.iter()
    }
}

fn mine() -> io::Result<Sids> {
    let token = Token::open()?;
    let user = token.information(TokenUser)?;
    let owner = token.information(TokenOwner)?;

    // SAFETY: each buffer holds the struct the class names, and the `Sid`
    // pointer inside it addresses the same allocation.
    let user_sid = unsafe { (*(user.as_ptr() as *const TOKEN_USER)).User.Sid };
    let owner_sid = unsafe { (*(owner.as_ptr() as *const TOKEN_OWNER)).Owner };
    if user_sid.is_null() && owner_sid.is_null() {
        return Err(refused());
    }

    let sids = [user_sid, owner_sid]
        .into_iter()
        .filter(|sid| !sid.is_null())
        .collect();
    Ok(Sids {
        _user: user,
        _owner: owner,
        sids,
    })
}

/// `u64` rather than `u8`: the buffer is read back as a struct of pointers, and
/// a `Vec<u8>` carries no alignment promise strong enough for that.
struct Buffer(Vec<u64>);

impl Buffer {
    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr().cast()
    }
}

struct Token(HANDLE);

impl Token {
    fn open() -> io::Result<Self> {
        let mut handle: HANDLE = ptr::null_mut();
        // SAFETY: the pseudo-handle from `GetCurrentProcess` needs no closing,
        // and `handle` is owned by the returned `Token` from here.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
            return Err(refused());
        }
        Ok(Self(handle))
    }

    fn information(&self, class: i32) -> io::Result<Buffer> {
        let mut needed = 0u32;
        // The first call is expected to fail; it is how the size is asked for.
        unsafe { GetTokenInformation(self.0, class, ptr::null_mut(), 0, &mut needed) };
        if needed == 0 {
            return Err(refused());
        }
        let mut buffer = Buffer(vec![0u64; needed.div_ceil(8) as usize]);
        let capacity = (buffer.0.len() * 8) as u32;
        // SAFETY: the buffer is `capacity` bytes and outlives the call.
        let ok = unsafe {
            GetTokenInformation(
                self.0,
                class,
                buffer.0.as_mut_ptr().cast(),
                capacity,
                &mut needed,
            )
        };
        if ok == 0 {
            return Err(refused());
        }
        Ok(buffer)
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: opened by `Token::open` and not handed out anywhere.
        unsafe { CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_has_an_identity_to_compare_against() {
        let ours = mine().expect("a token this process can read");
        assert!(
            ours.iter().count() >= 1,
            "no SID to compare a pipe's owner with"
        );
    }

    #[test]
    fn the_refusal_names_no_path_and_no_account() {
        let text = refused().to_string();
        assert!(!text.contains('\\'), "rule 4: no path in a user message");
        assert!(!text.contains('/'), "rule 4: no path in a user message");
        assert_eq!(refused().kind(), io::ErrorKind::PermissionDenied);
    }

    /// A pipe this process created is owned by this process's account, so the
    /// check must accept it. The negative case needs a second account and is
    /// named in DMY-179 as unrunnable in CI.
    #[tokio::test]
    async fn a_pipe_this_account_owns_is_accepted() {
        let (server, client) = super::super::pair().await.expect("a pipe pair");
        verify(&server).expect("this account's own pipe was refused");
        verify(&client).expect("this account's own pipe was refused");
    }
}
