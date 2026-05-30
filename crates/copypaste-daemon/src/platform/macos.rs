//! macOS platform backend — wraps existing clipboard.rs + keychain.rs.
//! Full implementation lives in those modules; this re-exports the trait impls.

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

/// Returns `true` when the primary network interface is Wi-Fi.
///
/// Used by the cloud push/poll loops for the `sync_on_wifi_only` gate.
/// Implemented by running `networksetup -getairportnetwork` on a non-empty
/// set of Wi-Fi interfaces and checking for a non-error response.  Falls back
/// to `true` (allow sync) when the check cannot complete so a misconfigured or
/// headless environment does not silently block cloud sync.
///
/// This is a **blocking** function (spawns a child process).  Always call it
/// via `tokio::task::spawn_blocking`.
pub fn is_on_wifi() -> bool {
    // Get the list of Wi-Fi hardware ports from networksetup.
    // Typical output line: "Wi-Fi  en0"
    let output = match std::process::Command::new("networksetup")
        .args(["-listallhardwareports"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return true, // fallback: assume Wi-Fi
    };
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Find lines that contain "Wi-Fi" and extract the device name.
    let mut wifi_devices: Vec<String> = Vec::new();
    let mut lines = stdout.lines().peekable();
    while let Some(line) = lines.next() {
        if line.contains("Wi-Fi") || line.contains("AirPort") {
            // Next non-empty line is "Device: enX"
            if let Some(dev_line) = lines.peek() {
                if let Some(dev) = dev_line.strip_prefix("Device: ") {
                    wifi_devices.push(dev.trim().to_owned());
                }
            }
        }
    }

    if wifi_devices.is_empty() {
        // No Wi-Fi hardware found — allow sync.
        return true;
    }

    // For each Wi-Fi interface, check if it has an associated network.
    for device in &wifi_devices {
        let result = std::process::Command::new("networksetup")
            .args(["-getairportnetwork", device])
            .output();
        if let Ok(out) = result {
            let text = String::from_utf8_lossy(&out.stdout);
            // Connected: "Current Wi-Fi Network: MySSID"
            // Disconnected: "You are not associated with an AirPort network."
            if text.contains("Current Wi-Fi Network:") {
                return true;
            }
        }
    }

    false
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
