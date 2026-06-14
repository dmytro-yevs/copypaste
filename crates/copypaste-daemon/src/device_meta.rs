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
use std::sync::OnceLock;
use tracing::debug;

// ---------------------------------------------------------------------------
// Startup cache
// ---------------------------------------------------------------------------

/// Process-wide cache for the static parts of this device's metadata.
///
/// `DeviceMeta::collect` spawns several child processes (`scutil`, `sysctl`,
/// `sw_vers`) that together take up to ~6 s on cold macOS systems.  Because
/// `device_name`, `device_model`, and `os_version` never change while the
/// daemon is running, it is safe — and much cheaper — to collect them ONCE at
/// daemon startup and reuse the result for every subsequent call.
///
/// `local_ip` is also captured here.  It can change when the user moves
/// between networks, but the existing code already only collects it once per
/// pairing/QR action, so caching it here is no regression — the same trade-off
/// that the mDNS advertisement already makes (it re-registers on interface
/// change events separately).
///
/// # How to populate
/// Call [`warm_cache`] once from a `tokio::task::spawn_blocking` context
/// (or from a plain blocking thread) before any other code path reaches
/// [`get_cached`].  Subsequent calls to [`warm_cache`] are no-ops.
///
/// # Thread-safety
/// `OnceLock` guarantees that exactly one thread runs `DeviceMeta::collect`
/// even when two threads race on the first call.  All readers after the first
/// write are wait-free.
static CACHED_META: OnceLock<DeviceMeta> = OnceLock::new();

/// Populate the process-wide metadata cache.
///
/// This is a **blocking** function (it may call `scutil`, `sysctl`, etc.).
/// Call it from `tokio::task::spawn_blocking` in async contexts.
///
/// Idempotent: if the cache is already populated, this is a no-op (the
/// already-stored value is never replaced).
pub(crate) fn warm_cache(app_version: &str) {
    CACHED_META.get_or_init(|| DeviceMeta::collect(app_version));
}

/// Return the cached metadata, collecting it now (blocking) if the cache has
/// not been warmed yet.
///
/// Prefer calling [`warm_cache`] explicitly at startup so the latency is paid
/// once at a predictable time rather than on the first pairing/QR request.
/// This fallback guarantees correctness even if `warm_cache` was never called.
pub(crate) fn get_cached(app_version: &str) -> &'static DeviceMeta {
    CACHED_META.get_or_init(|| DeviceMeta::collect(app_version))
}

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

    /// App / daemon version string (from `BUILD_VERSION`).
    pub app_version: String,

    /// Best LAN-routable IPv4 address for display (the same address the
    /// mDNS advertisement already publishes).  Absent when the device has no
    /// real LAN interface (e.g. a CI sandbox).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,

    /// Best-effort public / WAN IPv4 address, resolved once on startup via a
    /// STUN binding request and refreshed every ~15 minutes.  `None` when the
    /// network query fails, times out, or the user has opted out via the
    /// `collect_public_ip = false` config flag.  Never blocks startup.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub public_ip: Option<String>,
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
            // public_ip is NOT populated here: it requires an async network
            // call (STUN) and is injected by the IPC layer from the cached
            // value in ServerState.  Keeping collect() sync + offline ensures
            // it stays usable from spawn_blocking.
            public_ip: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Hostname / computer name
// ---------------------------------------------------------------------------

pub(crate) fn collect_device_name() -> Option<String> {
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
///
/// The identifier encodes family + generation; we extract the family name.
/// Unknown identifiers are returned verbatim so the UI always shows something.
fn model_id_to_friendly(raw: &str) -> String {
    let raw = raw.trim();

    // Ordered from most to least specific.
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
        // Return raw so the UI is never blank.
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
        // `sw_vers -productVersion` → "15.5"
        if let Some(ver) = run_command("sw_vers", &["-productVersion"]) {
            if !ver.is_empty() {
                return Some(format!("macOS {ver}"));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        // /etc/os-release is standard on modern Linux.
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
    // Reuse the exact same selection policy as the mDNS advertisement so the
    // IP shown in the UI matches what peers dial.  This never makes a network
    // request — it only reads the OS interface table.
    let usable = copypaste_p2p::interfaces::usable_advertise_addrs();
    let ip = copypaste_p2p::interfaces::pick_advertise_host(
        &usable,
        // Fallback to UNSPECIFIED so we can detect "no real LAN interface"
        // and return None instead of a meaningless "0.0.0.0".
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

    // Re-wait to collect stdout after the process exits.
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
        // Unknown identifiers are returned verbatim.
        assert_eq!(model_id_to_friendly("SomeUnknown1,2"), "SomeUnknown1,2");
    }

    #[test]
    fn collect_does_not_panic() {
        // Smoke test: collect must never panic regardless of the environment.
        let meta = DeviceMeta::collect("0.5.2");
        // app_version is always present.
        assert_eq!(meta.app_version, "0.5.2");
        // All optional fields must be either None or a non-empty Some.
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
        // public_ip is NOT collected by DeviceMeta::collect (requires async +
        // network); it is injected at the IPC layer. The field must exist and
        // default to None.
        assert!(
            meta.public_ip.is_none(),
            "DeviceMeta::collect must not populate public_ip"
        );
    }

    /// `DeviceMeta` serialises `public_ip` only when it is `Some` (the field is
    /// tagged `skip_serializing_if = "Option::is_none"`).
    #[test]
    fn public_ip_skipped_in_serialisation_when_none() {
        let meta = DeviceMeta {
            device_name: None,
            device_model: None,
            os_version: None,
            app_version: "test".to_owned(),
            local_ip: None,
            public_ip: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(
            !json.contains("public_ip"),
            "public_ip must be absent from JSON when None: {json}"
        );
    }

    /// When `public_ip` is `Some`, it IS included in the serialised form.
    #[test]
    fn public_ip_present_in_serialisation_when_some() {
        let meta = DeviceMeta {
            device_name: None,
            device_model: None,
            os_version: None,
            app_version: "test".to_owned(),
            local_ip: None,
            public_ip: Some("203.0.113.42".to_owned()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(
            json.contains("\"public_ip\":\"203.0.113.42\""),
            "public_ip must appear in JSON when Some: {json}"
        );
    }

    /// `get_cached` must always return a non-panicking result, and repeated
    /// calls must return the same `app_version` (cache hit, not a re-collect).
    #[test]
    fn get_cached_is_idempotent() {
        let first = get_cached("0.6.0");
        let second = get_cached("0.6.0");
        // Both calls must return the same static reference (pointer equality).
        assert!(
            std::ptr::eq(first, second),
            "get_cached must return the same cached reference on repeated calls"
        );
        assert_eq!(first.app_version, "0.6.0");
        assert!(
            first.public_ip.is_none(),
            "cached meta must not populate public_ip"
        );
    }

    /// `warm_cache` followed by `get_cached` must return the pre-warmed value.
    ///
    /// Note: because `CACHED_META` is a process-wide `OnceLock`, other tests in
    /// this module may have already warmed it with a different `app_version`.
    /// This test only checks that the cache is populated (non-panic) and that
    /// `get_cached` returns without blocking.
    #[test]
    fn warm_cache_and_get_cached_do_not_panic() {
        warm_cache("0.6.0");
        let meta = get_cached("0.6.0");
        // The cache must have been populated.
        assert!(!meta.app_version.is_empty());
    }
}
