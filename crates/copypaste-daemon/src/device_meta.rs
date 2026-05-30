//! Device identity metadata collector.
//!
//! Collects human-readable identity fields for THIS device so the UI can show
//! meaningful labels instead of raw fingerprints.  All collection is
//! best-effort: every field is `Option<String>` and failures are logged at
//! `debug` level rather than propagated — a missing field just omits the row
//! in the UI, it never breaks pairing or sync.
//!
//! The collected values are intentionally non-secret (hostname, model, OS
//! version, LAN IP) and mirror what mDNS service records already broadcast on
//! the local network.
//!
//! **Blocking note:** `DeviceMeta::collect` spawns short-lived child processes
//! (`scutil`, `sysctl`, `sw_vers`) that may take up to 2 s to complete.
//! Call it via `tokio::task::spawn_blocking` in async contexts.

use std::net::IpAddr;
use tracing::debug;

/// Rich identity metadata for this device.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceMeta {
    /// Human-readable hostname (`scutil --get ComputerName` on macOS, else
    /// `hostname`).  E.g. `"Dmytro's MacBook Air"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,

    /// Friendly hardware model string.  E.g. `"MacBook Air"`, `"Mac mini"`.
    /// Derived from `sysctl hw.model` + a lookup table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,

    /// OS name + version.  E.g. `"macOS 15.5"`, `"Linux"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,

    /// App / daemon version string.
    pub app_version: String,

    /// Best LAN-routable IPv4 address for display (the same address the
    /// mDNS advertisement already publishes).  Absent when the device has no
    /// real LAN interface (e.g. a CI sandbox).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,
}

impl DeviceMeta {
    /// Collect metadata for the current device.  Never panics; all sub-calls
    /// are fallible and their errors are logged then discarded.
    ///
    /// **This is a blocking function** (spawns child processes).  In an async
    /// context wrap it with `tokio::task::spawn_blocking(|| DeviceMeta::collect(ver))`.
    pub fn collect(app_version: &str) -> Self {
        Self {
            device_name: collect_device_name(),
            device_model: collect_device_model(),
            os_version: collect_os_version(),
            app_version: app_version.to_owned(),
            local_ip: collect_local_ip(),
        }
    }
}

// ---------------------------------------------------------------------------
// Hostname / computer name
// ---------------------------------------------------------------------------

fn collect_device_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        // Prefer the user-visible "Computer Name" (e.g. "Dmytro's MacBook Air")
        // over the bare hostname.  `scutil --get ComputerName` is the canonical
        // way; fall through to HOSTNAME env var or `hostname` on failure.
        if let Some(name) = run_command("scutil", &["--get", "ComputerName"]) {
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    // Generic fallback: HOSTNAME env var → COMPUTERNAME → `hostname` binary.
    if let Ok(h) = std::env::var("HOSTNAME") {
        let h = h.trim().to_owned();
        if !h.is_empty() {
            return Some(h);
        }
    }
    if let Ok(h) = std::env::var("COMPUTERNAME") {
        let h = h.trim().to_owned();
        if !h.is_empty() {
            return Some(h);
        }
    }
    run_command("hostname", &[])
}

// ---------------------------------------------------------------------------
// Hardware model
// ---------------------------------------------------------------------------

fn collect_device_model() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        // `sysctl -n hw.model` returns identifiers like "Mac14,2" (MacBook Air
        // M2) or "Macmini9,1" (Mac mini M1).  Map to friendly names.
        if let Some(raw) = run_command("sysctl", &["-n", "hw.model"]) {
            let friendly = model_id_to_friendly(&raw);
            return Some(friendly);
        }
    }
    #[cfg(target_os = "linux")]
    {
        // /sys/devices/virtual/dmi/id/product_name is best-effort on Linux.
        if let Ok(s) = std::fs::read_to_string("/sys/devices/virtual/dmi/id/product_name") {
            let s = s.trim().to_owned();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

/// Map a raw `hw.model` identifier to a human-friendly string.
fn model_id_to_friendly(raw: &str) -> String {
    let raw = raw.trim();

    let friendly: &str = if raw.starts_with("MacBookAir") {
        "MacBook Air"
    } else if raw.starts_with("MacBookPro") {
        "MacBook Pro"
    } else if raw.starts_with("MacBook") {
        "MacBook"
    } else if raw.starts_with("Macmini") || raw.starts_with("MacMini") {
        "Mac mini"
    } else if raw.starts_with("MacPro") {
        "Mac Pro"
    } else if raw.starts_with("iMacPro") {
        "iMac Pro"
    } else if raw.starts_with("iMac") {
        "iMac"
    } else if raw.starts_with("Mac") {
        // Covers "Mac14,2", "Mac15,3", etc. — Apple Silicon unified Mac IDs.
        "Mac"
    } else {
        return raw.to_owned();
    };

    friendly.to_owned()
}

// ---------------------------------------------------------------------------
// OS version
// ---------------------------------------------------------------------------

fn collect_os_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(ver) = run_command("sw_vers", &["-productVersion"]) {
            if !ver.is_empty() {
                return Some(format!("macOS {ver}"));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                    let val = val.trim_matches('"').trim().to_owned();
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
        return Some("Linux".to_owned());
    }
    #[cfg(target_os = "windows")]
    {
        return Some("Windows".to_owned());
    }
    #[allow(unreachable_code)]
    None
}

// ---------------------------------------------------------------------------
// Local IP
// ---------------------------------------------------------------------------

fn collect_local_ip() -> Option<String> {
    let usable = copypaste_p2p::interfaces::usable_advertise_addrs();
    let ip = copypaste_p2p::interfaces::pick_advertise_host(
        &usable,
        IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
    );
    if ip.is_unspecified() || ip.is_loopback() {
        debug!("device_meta: no real LAN interface found; omitting local_ip");
        None
    } else {
        Some(ip.to_string())
    }
}

// ---------------------------------------------------------------------------
// Command runner
// ---------------------------------------------------------------------------

/// Run a short-lived command and return its trimmed stdout, or `None` on
/// any failure.  Caps wall-time at 2 seconds to avoid blocking the IPC loop.
fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    use std::time::Duration;

    let mut child = match std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            debug!("device_meta: {cmd} spawn failed: {e}");
            return None;
        }
    };

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    debug!("device_meta: {cmd} exited with {status}");
                    return None;
                }
                break;
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    debug!("device_meta: {cmd} timed out");
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                debug!("device_meta: {cmd} wait error: {e}");
                return None;
            }
        }
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            debug!("device_meta: {cmd} output read failed: {e}");
            return None;
        }
    };
    let s = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_id_known_prefixes() {
        assert_eq!(model_id_to_friendly("MacBookAir10,1"), "MacBook Air");
        assert_eq!(model_id_to_friendly("MacBookPro18,3"), "MacBook Pro");
        assert_eq!(model_id_to_friendly("MacBook8,1"), "MacBook");
        assert_eq!(model_id_to_friendly("Macmini9,1"), "Mac mini");
        assert_eq!(model_id_to_friendly("MacPro7,1"), "Mac Pro");
        assert_eq!(model_id_to_friendly("iMacPro1,1"), "iMac Pro");
        assert_eq!(model_id_to_friendly("iMac21,1"), "iMac");
        assert_eq!(model_id_to_friendly("Mac14,2"), "Mac");
        assert_eq!(model_id_to_friendly("Mac15,3"), "Mac");
    }

    #[test]
    fn model_id_unknown_returns_raw() {
        assert_eq!(model_id_to_friendly("SomeUnknown1,2"), "SomeUnknown1,2");
    }

    #[test]
    fn collect_does_not_panic() {
        let meta = DeviceMeta::collect("0.5.2");
        assert_eq!(meta.app_version, "0.5.2");
        for v in [
            &meta.device_name,
            &meta.device_model,
            &meta.os_version,
            &meta.local_ip,
        ]
        .into_iter()
        .flatten()
        {
            assert!(!v.is_empty(), "optional field must not be Some(\"\")");
        }
    }
}
