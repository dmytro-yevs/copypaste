//! macOS platform backend — wraps existing clipboard.rs + keychain.rs.
//! Full implementation lives in those modules; this re-exports the trait impls.
//!
//! Also provides [`is_on_wifi`] for the `sync_on_wifi_only` guard.

use super::{ClipboardBackend, ClipboardEvent, ClipboardSource, KeystoreBackend};
use crate::clipboard::{ClipboardContent, ClipboardMonitor};
use crate::keychain::{self, KeychainError};

/// macOS clipboard backend wrapping NSPasteboard polling.
pub struct MacosClipboardBackend {
    monitor: ClipboardMonitor,
    poll_interval_ms: u64,
}

impl MacosClipboardBackend {
    pub fn new(max_text_bytes: u64, poll_interval_ms: u64) -> Self {
        Self {
            monitor: ClipboardMonitor::new(max_text_bytes),
            poll_interval_ms,
        }
    }
}

impl ClipboardBackend for MacosClipboardBackend {
    fn next_change(&mut self) -> Option<ClipboardEvent> {
        std::thread::sleep(std::time::Duration::from_millis(self.poll_interval_ms));
        match self.monitor.poll() {
            Ok(Some(ClipboardContent::Text(text))) => Some(ClipboardEvent {
                text: Some(text),
                image_bytes: None,
                source: ClipboardSource::General,
            }),
            Ok(Some(ClipboardContent::Image(bytes))) => Some(ClipboardEvent {
                text: None,
                image_bytes: Some(bytes),
                source: ClipboardSource::General,
            }),
            _ => None,
        }
    }
}

/// macOS keystore backend — macOS Keychain via security-framework.
pub struct MacosKeystoreBackend;

impl KeystoreBackend for MacosKeystoreBackend {
    type Error = KeychainError;

    fn load_or_create(
        &self,
        _service: &str,
        _account: &str,
    ) -> Result<zeroize::Zeroizing<[u8; 32]>, Self::Error> {
        keychain::load_or_create().map(|kp| kp.secret_key_bytes_zeroizing())
    }

    fn store(&self, _service: &str, _account: &str, _secret: &[u8; 32]) -> Result<(), Self::Error> {
        // Keychain stores on load_or_create; explicit store not yet needed
        Ok(())
    }

    fn delete(&self, _service: &str, _account: &str) -> Result<(), Self::Error> {
        #[cfg(target_os = "macos")]
        return keychain::delete_stored();
        #[cfg(not(target_os = "macos"))]
        Ok(())
    }
}

/// Return `true` when the machine is currently connected via Wi-Fi.
///
/// Uses `networksetup -getairportnetwork <interface>` on the first active
/// Wi-Fi interface reported by `networksetup -listallhardwareports`. Falls back
/// to `true` (allow sync) on any detection failure so that a broken
/// `networksetup` never silently blocks sync.
///
/// Only compiled on macOS; callers on other platforms always get `true`.
#[cfg(target_os = "macos")]
pub fn is_on_wifi() -> bool {
    // Step 1: find the Wi-Fi device name (en0, en1, …) by parsing
    // `networksetup -listallhardwareports`. The output looks like:
    //
    //   Hardware Port: Wi-Fi
    //   Device: en0
    //   Ethernet Address: …
    //
    // We look for the block whose Hardware Port contains "Wi-Fi" and grab the
    // following Device: line.
    let list_output = match std::process::Command::new("networksetup")
        .args(["-listallhardwareports"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "is_on_wifi: could not run networksetup -listallhardwareports; \
                 assuming Wi-Fi (allowing sync)"
            );
            return true;
        }
    };

    let list_str = String::from_utf8_lossy(&list_output.stdout);
    let mut wifi_device: Option<String> = None;
    let mut next_is_device = false;
    for line in list_str.lines() {
        let line = line.trim();
        if line.to_ascii_lowercase().contains("wi-fi") && line.starts_with("Hardware Port") {
            next_is_device = true;
        } else if next_is_device {
            if let Some(dev) = line.strip_prefix("Device: ") {
                wifi_device = Some(dev.trim().to_owned());
            }
            next_is_device = false;
        }
    }

    let device = match wifi_device {
        Some(d) if !d.is_empty() => d,
        _ => {
            tracing::debug!(
                "is_on_wifi: no Wi-Fi interface found by networksetup; \
                 assuming Wi-Fi (allowing sync)"
            );
            return true;
        }
    };

    // Step 2: check whether that interface has an active SSID (i.e. is
    // associated). `networksetup -getairportnetwork <dev>` returns either:
    //   "Current Wi-Fi Network: <ssid>"   — associated
    //   "You are not associated with an AirPort network."   — not associated
    let net_output = match std::process::Command::new("networksetup")
        .args(["-getairportnetwork", &device])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(
                error = %e,
                device,
                "is_on_wifi: could not run networksetup -getairportnetwork; \
                 assuming Wi-Fi (allowing sync)"
            );
            return true;
        }
    };

    let net_str = String::from_utf8_lossy(&net_output.stdout);
    let on_wifi = net_str.contains("Current Wi-Fi Network:");
    tracing::debug!(device, on_wifi, "is_on_wifi check");
    on_wifi
}

/// Non-macOS stub — always returns `true` (allow sync) so the caller compiles
/// on all platforms without conditional compilation at every call site.
#[cfg(not(target_os = "macos"))]
pub fn is_on_wifi() -> bool {
    true
}
