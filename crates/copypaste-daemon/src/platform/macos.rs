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

/// TTL for the cached Wi-Fi association status (CopyPaste-crh3.71).
///
/// `is_on_wifi_uncached` forks two `networksetup` subprocesses per call and is
/// invoked from five independent sync sites (relay push/receive, p2p fanout,
/// cloud push/poll). Under `sync_on_wifi_only=true` with fast copying this
/// reached ~240 subprocess pairs/minute and drained the battery. Wi-Fi
/// association changes far more slowly than the sync-event rate, so a short
/// process-global cache collapses the storm to at most one probe per TTL.
/// Slightly longer than the 2 s lsappinfo cache because association is even
/// more stable than the frontmost app.
#[cfg(target_os = "macos")]
const WIFI_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);

/// Process-global cache of `(probed_at, on_wifi)`. `const`-initialised so it
/// needs no lazy initialisation. The probe runs while the lock is held so that
/// concurrent cold callers collapse to a SINGLE `networksetup` fork (the rest
/// block briefly, then read the fresh value) rather than a thundering herd.
#[cfg(target_os = "macos")]
static WIFI_CACHE: std::sync::Mutex<Option<(std::time::Instant, bool)>> =
    std::sync::Mutex::new(None);

/// Return `true` when the machine is currently connected via Wi-Fi, answering
/// from a [`WIFI_CACHE_TTL`] cache so `networksetup` is forked at most once per
/// TTL regardless of how many sync events fire (CopyPaste-crh3.71). Mirrors the
/// `FrontmostAppCache` TTL pattern but as a process-global because the callers
/// are independent async tasks rather than the single capture loop.
///
/// Falls back to `true` (allow sync) on any detection failure so a broken
/// `networksetup` never silently blocks sync. Only compiled on macOS; callers
/// on other platforms always get `true`.
#[cfg(target_os = "macos")]
pub fn is_on_wifi() -> bool {
    is_on_wifi_cached(
        &WIFI_CACHE,
        std::time::Instant::now(),
        WIFI_CACHE_TTL,
        is_on_wifi_uncached,
    )
}

/// Pure TTL-cache core for [`is_on_wifi`], parameterised over the cache cell,
/// the current instant, the TTL, and the probe — so the cache behaviour is
/// unit-testable without forking `networksetup`. Returns the cached value when
/// `now` is within `ttl` of the last probe; otherwise runs `probe`, stores
/// `(now, value)`, and returns it. `probe` runs while the lock is held so
/// concurrent cold callers collapse to a single probe.
#[cfg(target_os = "macos")]
fn is_on_wifi_cached(
    cache: &std::sync::Mutex<Option<(std::time::Instant, bool)>>,
    now: std::time::Instant,
    ttl: std::time::Duration,
    probe: impl FnOnce() -> bool,
) -> bool {
    let mut guard = cache.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((stamp, value)) = *guard {
        if now.duration_since(stamp) < ttl {
            return value;
        }
    }
    let value = probe();
    *guard = Some((now, value));
    value
}

/// Uncached Wi-Fi probe — forks two `networksetup` subprocesses. Always call
/// through [`is_on_wifi`] in production; this exists separately so the cache and
/// the probe can be reasoned about (and tested) independently.
///
/// Uses `networksetup -getairportnetwork <interface>` on the first active
/// Wi-Fi interface reported by `networksetup -listallhardwareports`. Falls back
/// to `true` (allow sync) on any detection failure.
#[cfg(target_os = "macos")]
fn is_on_wifi_uncached() -> bool {
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// CopyPaste-crh3.71: within one TTL window the probe runs exactly once no
    /// matter how many times `is_on_wifi` is called.
    #[test]
    fn wifi_cache_probes_once_within_ttl() {
        let cache: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
        let ttl = Duration::from_secs(5);
        let calls = AtomicUsize::new(0);
        let base = Instant::now();

        // 100 calls, each strictly within the TTL of the first probe.
        for i in 0..100 {
            let now = base + Duration::from_millis(i * 40); // 0..3960ms < 5s
            let probe = || {
                calls.fetch_add(1, Ordering::SeqCst);
                true
            };
            assert!(is_on_wifi_cached(&cache, now, ttl, probe));
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "networksetup must be probed exactly once within a TTL window"
        );
    }

    /// After the TTL elapses the next call re-probes, and a changed result is
    /// reflected.
    #[test]
    fn wifi_cache_refreshes_after_ttl_and_reflects_change() {
        let cache: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
        let ttl = Duration::from_secs(5);
        let calls = AtomicUsize::new(0);
        let base = Instant::now();

        // First probe at t0 → on Wi-Fi.
        assert!(is_on_wifi_cached(&cache, base, ttl, || {
            calls.fetch_add(1, Ordering::SeqCst);
            true
        }));
        // Still within TTL → cached, no new probe.
        assert!(is_on_wifi_cached(
            &cache,
            base + Duration::from_secs(4),
            ttl,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                true
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "cache hit within TTL");

        // Past the TTL → re-probe, and now NOT on Wi-Fi.
        assert!(!is_on_wifi_cached(
            &cache,
            base + ttl + Duration::from_millis(1),
            ttl,
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                false
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 2, "re-probe after TTL");
    }
}
