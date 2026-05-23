// settings.rs — AppSettings data type for the UI layer
// This is a UI-facing struct, not the same as AppConfig (which is the daemon config).

use serde::{Deserialize, Serialize};

/// History size limit options exposed in the UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryLimit {
    /// Keep 50 most-recent items.
    Fifty,
    /// Keep 100 most-recent items (default).
    #[default]
    Hundred,
    /// Keep 500 most-recent items.
    FiveHundred,
    /// Keep everything.
    Unlimited,
}

impl HistoryLimit {
    /// Returns the numeric value used internally (0 = unlimited).
    pub fn as_count(self) -> usize {
        match self {
            HistoryLimit::Fifty       => 50,
            HistoryLimit::Hundred     => 100,
            HistoryLimit::FiveHundred => 500,
            HistoryLimit::Unlimited   => 0,
        }
    }

    /// Build from a numeric count (0 = unlimited; nearest valid value is chosen).
    pub fn from_count(n: usize) -> Self {
        match n {
            0          => HistoryLimit::Unlimited,
            1..=75     => HistoryLimit::Fifty,
            76..=300   => HistoryLimit::Hundred,
            _          => HistoryLimit::FiveHundred,
        }
    }
}

/// Settings managed through the SettingsWindow UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub launch_at_login: bool,
    pub private_mode:    bool,
    pub history_limit:   HistoryLimit,
    pub supabase_url:    String,
    /// Supabase anon key — stored in memory only, persisted via Keychain by the caller.
    pub supabase_key:    String,
    pub device_name:     String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            private_mode:    false,
            history_limit:   HistoryLimit::default(),
            supabase_url:    String::new(),
            supabase_key:    String::new(),
            device_name:     String::from("My Mac"),
        }
    }
}

impl AppSettings {
    /// Convenience constructor.
    pub fn new() -> Self {
        Self::default()
    }
}

/// A device that has been paired with this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDevice {
    /// Human-readable name (may be empty if unknown).
    pub name:        String,
    /// Full hex fingerprint (as returned by `DeviceKeypair::fingerprint()`).
    pub fingerprint: String,
}

impl PairedDevice {
    pub fn new(name: impl Into<String>, fingerprint: impl Into<String>) -> Self {
        Self {
            name:        name.into(),
            fingerprint: fingerprint.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── HistoryLimit ─────────────────────────────────────────────────────────

    #[test]
    fn history_limit_default_is_hundred() {
        assert_eq!(HistoryLimit::default(), HistoryLimit::Hundred);
        assert_eq!(HistoryLimit::default().as_count(), 100);
    }

    #[test]
    fn history_limit_from_count_zero_is_unlimited() {
        assert_eq!(HistoryLimit::from_count(0), HistoryLimit::Unlimited);
        assert_eq!(HistoryLimit::Unlimited.as_count(), 0);
    }

    #[test]
    fn history_limit_from_count_boundaries() {
        assert_eq!(HistoryLimit::from_count(50),  HistoryLimit::Fifty);
        assert_eq!(HistoryLimit::from_count(75),  HistoryLimit::Fifty);
        assert_eq!(HistoryLimit::from_count(76),  HistoryLimit::Hundred);
        assert_eq!(HistoryLimit::from_count(100), HistoryLimit::Hundred);
        assert_eq!(HistoryLimit::from_count(300), HistoryLimit::Hundred);
        assert_eq!(HistoryLimit::from_count(301), HistoryLimit::FiveHundred);
        assert_eq!(HistoryLimit::from_count(500), HistoryLimit::FiveHundred);
    }

    #[test]
    fn history_limit_as_count_all_variants() {
        assert_eq!(HistoryLimit::Fifty.as_count(),       50);
        assert_eq!(HistoryLimit::Hundred.as_count(),     100);
        assert_eq!(HistoryLimit::FiveHundred.as_count(), 500);
        assert_eq!(HistoryLimit::Unlimited.as_count(),   0);
    }

    // ── AppSettings serialization ─────────────────────────────────────────────

    #[test]
    fn app_settings_default_serializes_and_roundtrips() {
        let s = AppSettings::default();
        let json = serde_json::to_string(&s).expect("serialize");
        let restored: AppSettings = serde_json::from_str(&json).expect("deserialize");
        assert!(!restored.launch_at_login);
        assert!(!restored.private_mode);
        assert_eq!(restored.history_limit,   HistoryLimit::Hundred);
        assert_eq!(restored.device_name,     "My Mac");
    }

    #[test]
    fn app_settings_custom_values_roundtrip() {
        let s = AppSettings {
            launch_at_login: true,
            private_mode:    true,
            history_limit:   HistoryLimit::Unlimited,
            supabase_url:    "https://example.supabase.co".into(),
            supabase_key:    "secret-key".into(),
            device_name:     "Alice's MacBook".into(),
        };
        let json = serde_json::to_string(&s).expect("serialize");
        let r: AppSettings = serde_json::from_str(&json).expect("deserialize");
        assert!(r.launch_at_login);
        assert!(r.private_mode);
        assert_eq!(r.history_limit, HistoryLimit::Unlimited);
        assert_eq!(r.supabase_url, "https://example.supabase.co");
        assert_eq!(r.device_name, "Alice's MacBook");
    }

    // ── PairedDevice ─────────────────────────────────────────────────────────

    #[test]
    fn paired_device_new_stores_fields() {
        let d = PairedDevice::new("Bob's iPhone", "deadbeef01234567");
        assert_eq!(d.name, "Bob's iPhone");
        assert_eq!(d.fingerprint, "deadbeef01234567");
    }

    #[test]
    fn paired_device_serializes_and_roundtrips() {
        let d = PairedDevice::new("Test Device", "aabbccdd");
        let json = serde_json::to_string(&d).expect("serialize");
        let r: PairedDevice = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, d);
    }

    #[test]
    fn paired_device_empty_name_is_valid() {
        let d = PairedDevice::new("", "aabbccdd");
        assert!(d.name.is_empty());
    }

    // ── PairedDevice list model operations ───────────────────────────────────

    #[test]
    fn paired_device_list_add_and_remove() {
        let mut devices: Vec<PairedDevice> = Vec::new();

        let d1 = PairedDevice::new("Device A", "aabbccdd");
        let d2 = PairedDevice::new("Device B", "11223344");

        devices.push(d1.clone());
        devices.push(d2.clone());
        assert_eq!(devices.len(), 2);

        // Remove by fingerprint
        devices.retain(|d| d.fingerprint != "aabbccdd");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].fingerprint, "11223344");
    }

    #[test]
    fn paired_device_list_no_duplicates_by_fingerprint() {
        let mut devices: Vec<PairedDevice> = Vec::new();
        let fp = "aabbccdd";
        let d = PairedDevice::new("Device A", fp);

        // Simulate "add only if not present"
        if !devices.iter().any(|x| x.fingerprint == fp) {
            devices.push(d.clone());
        }
        if !devices.iter().any(|x| x.fingerprint == fp) {
            devices.push(d.clone());
        }
        assert_eq!(devices.len(), 1);
    }
}
