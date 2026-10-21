//! Authenticate and reap the stable, short-lived login-Keychain accessor.
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Instant,
};

use core_foundation::url::CFURL;
use security_framework::os::macos::code_signing::{Flags, SecRequirement, SecStaticCode};
use zeroize::Zeroizing;

use super::super::keys::KEYSTORE_LOAD_TIMEOUT;
use super::{CryptoError, DeviceSecret, Lookup, KEY_LEN};

fn unavailable() -> CryptoError {
    CryptoError::KeystoreUnavailable("the authenticated keychain helper is unavailable")
}

fn verify(path: &std::path::Path, requirement: &SecRequirement) -> Result<(), CryptoError> {
    let url = CFURL::from_path(path, false).ok_or_else(unavailable)?;
    SecStaticCode::from_path(&url, Flags::NONE)
        .and_then(|code| {
            code.check_validity(
                Flags::STRICT_VALIDATE | Flags::NO_NETWORK_ACCESS,
                requirement,
            )
        })
        .map_err(|_| unavailable())
}

fn designated_requirement(executable: &std::path::Path) -> Result<String, CryptoError> {
    let signature = Command::new("/usr/bin/codesign")
        .args(["-d", "-r-"])
        .arg(executable)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| unavailable())?;
    let (status, signature) = collect(signature, None, KEYSTORE_LOAD_TIMEOUT, 4097)?;
    if status != 0 || signature.len() > 4096 {
        return Err(unavailable());
    }
    requirement_text(&signature)
}

fn requirement_text(signature: &[u8]) -> Result<String, CryptoError> {
    let text = String::from_utf8_lossy(signature);
    text.lines()
        .find_map(|line| {
            line.strip_prefix("designated => ")
                .or_else(|| line.strip_prefix("# designated => "))
        })
        .map(str::to_owned)
        .ok_or_else(unavailable)
}

pub(super) fn path() -> Result<Option<PathBuf>, CryptoError> {
    let executable = std::env::current_exe().map_err(|_| unavailable())?;
    let bundled = executable
        .parent()
        .ok_or_else(unavailable)?
        .join("copypaste-keychain");
    if !bundled.is_file() {
        // Standalone CLI/daemon releases and isolated Rust tests retain their
        // existing Keychain backend. Production app bundles carry the helper.
        return Ok(None);
    }
    let own = designated_requirement(&executable)?;
    if own.starts_with("cdhash ") {
        // Preserve the documented ad-hoc installation fallback. The installer
        // warns that this fallback cannot retain grants across updates.
        return Ok(None);
    }
    let identifier = "identifier \"com.copypaste.copypaste-daemon\"";
    if !own.contains(identifier) || !own.contains("certificate root = H\"") {
        return Err(unavailable());
    }
    let requirement: SecRequirement = own
        .replace(
            identifier,
            "identifier \"com.copypaste.copypaste-keychain\"",
        )
        .parse()
        .map_err(|_| unavailable())?;
    verify(&bundled, &requirement)?;
    let directory = Command::new(&bundled)
        .arg("support-directory")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| unavailable())?;
    let (status, directory) = collect(directory, None, KEYSTORE_LOAD_TIMEOUT, 4097)?;
    if status != 0 || directory.len() > 4096 {
        return Err(unavailable());
    }
    let directory = std::str::from_utf8(&directory)
        .map_err(|_| unavailable())?
        .trim_end();
    let path = PathBuf::from(directory).join("signing/copypaste-keychain-v1");
    if !path.is_absolute()
        || std::fs::symlink_metadata(&path)
            .map_err(|_| unavailable())?
            .file_type()
            .is_symlink()
    {
        return Err(unavailable());
    }
    verify(&path, &requirement)?;
    Ok(Some(path))
}

fn run(
    path: &std::path::Path,
    input: Option<&[u8]>,
) -> Result<(i32, Zeroizing<Vec<u8>>), CryptoError> {
    let child = Command::new(path)
        .arg(if input.is_some() { "create" } else { "load" })
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_remove("DYLD_INSERT_LIBRARIES")
        .env_remove("DYLD_LIBRARY_PATH")
        .spawn()
        .map_err(|_| unavailable())?;
    collect(child, input, KEYSTORE_LOAD_TIMEOUT, KEY_LEN + 1)
}

fn collect(
    mut child: Child,
    input: Option<&[u8]>,
    timeout: std::time::Duration,
    limit: usize,
) -> Result<(i32, Zeroizing<Vec<u8>>), CryptoError> {
    let deadline = Instant::now() + timeout;
    let result = (|| {
        if let Some(input) = input {
            child
                .stdin
                .take()
                .ok_or_else(unavailable)?
                .write_all(input)
                .map_err(|_| unavailable())?;
        }
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|_| unavailable())? {
                break status;
            }
            if Instant::now() >= deadline {
                return Err(unavailable());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let mut output = Zeroizing::new(Vec::with_capacity(limit));
        child
            .stdout
            .take()
            .ok_or_else(unavailable)?
            .take(limit as u64)
            .read_to_end(&mut output)
            .map_err(|_| unavailable())?;
        Ok((status.code().ok_or_else(unavailable)?, output))
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

pub(super) fn load(path: &std::path::Path) -> Result<Lookup, CryptoError> {
    let (status, bytes) = run(path, None)?;
    match status {
        0 if bytes.len() == KEY_LEN => {
            let mut secret = Zeroizing::new([0; KEY_LEN]);
            secret.copy_from_slice(&bytes);
            Ok(Lookup::Found(secret))
        }
        3 if bytes.is_empty() => Ok(Lookup::Absent),
        4 => Err(CryptoError::KeystoreEntryUnusable(
            "the stored device secret is the wrong length",
        )),
        _ => Err(unavailable()),
    }
}

pub(super) fn create(
    path: &std::path::Path,
    secret: DeviceSecret,
) -> Result<DeviceSecret, CryptoError> {
    let (status, output) = run(path, Some(secret.as_ref()))?;
    if status != 0 || !output.is_empty() {
        return Err(unavailable());
    }
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn executable(directory: &std::path::Path, script: &str) -> PathBuf {
        let path = directory.join("accessor");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn reads_the_native_codesign_requirement_stream() {
        let text = designated_requirement(std::path::Path::new("/usr/bin/codesign")).unwrap();
        let _: SecRequirement = text.parse().unwrap();
        assert!(text.contains("identifier"));
    }

    #[test]
    fn commented_adhoc_requirements_preserve_the_standalone_fallback() {
        let requirement = requirement_text(
            b"# designated => cdhash H\"0123456789012345678901234567890123456789\"\n",
        )
        .unwrap();
        assert!(requirement.starts_with("cdhash "));
        let _: SecRequirement = requirement.parse().unwrap();
    }

    #[test]
    fn missing_status_with_payload_never_authorizes_key_creation() {
        let directory = tempfile::tempdir().unwrap();
        let path = executable(directory.path(), "printf unexpected; exit 3");
        assert!(load(&path).is_err());
    }

    #[test]
    fn deadline_kills_and_reaps_a_blocked_accessor() {
        let child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id().to_string();
        assert!(collect(
            child,
            None,
            std::time::Duration::from_millis(100),
            KEY_LEN + 1
        )
        .is_err());
        let status = Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "the timed-out accessor must be gone");
    }
}
