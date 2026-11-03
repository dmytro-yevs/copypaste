//! The Windows half of the owner-only helpers: one protected DACL entry that
//! grants the current user everything, the daemon pipe's access list.
#![allow(unsafe_code)]

use std::fs::File;
use std::io;
use std::os::windows::io::FromRawHandle as _;
use std::path::Path;

use interprocess::os::windows::security_descriptor::{
    AsSecurityDescriptor as _, AsSecurityDescriptorExt as _, SecurityDescriptor,
};
use widestring::U16CString;
use win_security_identifier::{GetCurrentSid as _, SecurityIdentifier};
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::Authorization::{SetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE,
};

fn owner_only() -> io::Result<SecurityDescriptor> {
    let sid = SecurityIdentifier::get_current_user_sid()
        .map_err(|_| io::Error::other("could not read this account's identifier"))?;
    let text = U16CString::from_str(format!("D:P(A;;GA;;;{sid})"))
        .map_err(|_| io::Error::other("could not build the owner-only access list"))?;
    SecurityDescriptor::deserialize(&text)
}

fn wide(path: &Path) -> io::Result<U16CString> {
    U16CString::from_os_str(path.as_os_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL"))
}

/// The same access and sharing `OpenOptions::new().write(true).create_new(true)`
/// asks for, plus the descriptor, which `std` has no way to pass.
pub(crate) fn create_new(path: &Path) -> io::Result<File> {
    let security = owner_only()?;
    let path = wide(path)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.as_sd().cast_mut(),
        bInheritHandle: 0,
    };
    // SAFETY: `path` is NUL-terminated and `attributes` points at a valid
    // descriptor; both outlive the call, which retains neither.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `handle` is a freshly opened file handle owned by nothing else.
    Ok(unsafe { File::from_raw_handle(handle) })
}

pub(crate) fn restrict(path: &Path) -> io::Result<()> {
    let security = owner_only()?;
    let (dacl, _) = security
        .dacl()?
        .ok_or_else(|| io::Error::other("the owner-only access list has no DACL"))?;
    let path = wide(path)?;
    // SAFETY: `path` is NUL-terminated and `dacl` points into `security`, which
    // outlives the call. Owner, group and SACL are not being set, so null.
    let status = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            dacl,
            std::ptr::null(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}
