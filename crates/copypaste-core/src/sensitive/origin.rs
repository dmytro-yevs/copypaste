//! Which origins are credential stores, whatever their content looks like.

/// Windows process image names that no substring below would recognise.
///
/// From the vendor's own deployment documentation:
///
/// * `nordpass.exe` — support.nordpass.com/hc/en-us/articles/360004799257
/// * `enpass.exe` — help.enpass.io/business/latest/microsoft-365/packaging-enpass-for-windows-via-microsoft-intune
/// * `keeperpasswordmanager.exe` — docs.keeper.io/enterprise-guide/deploying-keeper-to-end-users/desktop-application
const WINDOWS_CREDENTIAL_STORES: [&str; 5] = [
    "nordpass.exe",
    "enpass.exe",
    "keeperpasswordmanager.exe",
    // Not vendor-documented and unverified on an installed system: RoboForm's
    // manual and help centre name the tray program ("AI RoboForm -> Taskbar
    // Icon") but no executable except the installer, so both spellings come
    // from third-party process catalogues (file.net, glarysoft.com) that
    // attribute them to Siber Systems. Kept because the `roboform` fragment
    // below matches neither, and dropping them would index a fill from the
    // tray icon as ordinary text.
    "robotaskbaricon.exe",
    "robotaskbaricon-x64.exe",
];

/// Fragments that identify a credential store inside a bundle identifier or a
/// process image name.
///
/// Matched against the identifier with its spaces removed as well as verbatim,
/// because Windows names a process the way the product is written —
/// `Proton Pass.exe`, `Sticky Password.exe` — and macOS does not.
const CREDENTIAL_STORE_FRAGMENTS: [&str; 11] = [
    "1password",
    "bitwarden",
    "keepass",
    "dashlane",
    "lastpass",
    "nordpass",
    "enpass",
    "roboform",
    "protonpass",
    "stickypassword",
    "strongbox",
];

/// macOS bundle identifiers, as each vendor ships them.
const MACOS_CREDENTIAL_STORES: [&str; 7] = [
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.bitwarden.desktop",
    "org.keepassxc.keepassxc",
    "com.dashlane.dashlane",
    "com.lastpass.lastpass",
    "com.apple.passwords",
];

/// The identifiers a fresh install excludes from capture, per platform.
///
/// The exact entries above, never [`CREDENTIAL_STORE_FRAGMENTS`]: this list is
/// user-editable, and a fragment would keep matching the entry the user just
/// deleted, so removal would not remove.
///
/// Narrower than the sensitivity floor on purpose. The floor still catches
/// everything it catches today whatever the user does here; exclusion only adds
/// the never-captured guarantee for identifiers we can name exactly.
pub fn default_excluded_app_ids(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::MacOs => &MACOS_CREDENTIAL_STORES,
        Platform::Windows => &WINDOWS_CREDENTIAL_STORES,
        // No opt-out marker and no stable per-app identity to key on from the
        // clipboard service; DMY-170 records the evidence.
        Platform::Android => &[],
    }
}

/// The shipped platforms ([ADR-0013](../../../../docs/adr/0013-windows-as-a-third-platform.md)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    Android,
}

/// Credential managers are an independent sensitivity floor: their copied
/// values must never enter full-text search even when their contents do not
/// match a detector rule.
pub fn is_password_manager_app(bundle_id: &str) -> bool {
    let bundle_id = bundle_id.trim().to_ascii_lowercase();
    if MACOS_CREDENTIAL_STORES.contains(&bundle_id.as_str())
        || WINDOWS_CREDENTIAL_STORES.contains(&bundle_id.as_str())
    {
        return true;
    }
    let squashed = bundle_id.replace(' ', "");
    CREDENTIAL_STORE_FRAGMENTS
        .iter()
        .any(|fragment| bundle_id.contains(fragment) || squashed.contains(fragment))
        // `proton.pass` is the reverse-DNS spelling; `secretive` is an agent
        // rather than a store, and has no name a fragment would survive.
        || squashed.contains("proton.pass")
        || bundle_id.contains("secretive")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seed is a strict subset of the floor. If it ever is not, an entry a
    /// user removed from the exclusion list would still be a credential store
    /// the floor forces sensitive — two mechanisms disagreeing about one app.
    #[test]
    fn every_seeded_identifier_is_also_a_credential_store() {
        for platform in [Platform::MacOs, Platform::Windows, Platform::Android] {
            for id in default_excluded_app_ids(platform) {
                assert!(is_password_manager_app(id), "{id}");
            }
        }
    }

    /// Fragments stay out: they are substring rules, and this list is edited by
    /// hand. Removing an entry has to actually remove it.
    #[test]
    fn the_seed_carries_no_substring_rules() {
        for platform in [Platform::MacOs, Platform::Windows] {
            let seeded = default_excluded_app_ids(platform);
            assert!(!seeded.is_empty());
            for fragment in CREDENTIAL_STORE_FRAGMENTS {
                assert!(!seeded.contains(&fragment), "{fragment}");
            }
        }
    }

    /// Android ships empty, and that is a claim about the platform rather than
    /// an oversight — the UI has to say so (DMY-170).
    #[test]
    fn android_seeds_nothing() {
        assert!(default_excluded_app_ids(Platform::Android).is_empty());
    }

    #[test]
    fn macos_bundle_identifiers_are_recognised() {
        for bundle_id in [
            "COM.1PASSWORD.1PASSWORD",
            "com.agilebits.onepassword7",
            "com.bitwarden.desktop",
            "org.keepassxc.keepassxc",
            "com.apple.Passwords",
            "com.proton.pass",
            "com.strongbox.passwordsafe",
            "com.mortenjust.secretive",
            "com.keepassium.keepassium",
        ] {
            assert!(is_password_manager_app(bundle_id), "{bundle_id}");
        }
    }

    /// DMY-158. A Windows source identity is a process image name, and one that
    /// is not recognised is a password indexed as ordinary text.
    #[test]
    fn windows_process_image_names_are_recognised() {
        for image_name in [
            "1Password.exe",
            "Bitwarden.exe",
            "KeePass.exe",
            "KeePassXC.exe",
            "Dashlane.exe",
            "LastPass.exe",
            "NordPass.exe",
            "Enpass.exe",
            "KeeperPasswordManager.exe",
            "RoboForm.exe",
            "RoboTaskBarIcon.exe",
            "robotaskbaricon-x64.exe",
            "Proton Pass.exe",
            "Sticky Password.exe",
            "Strongbox.exe",
        ] {
            assert!(is_password_manager_app(image_name), "{image_name}");
        }
    }

    /// The floor keeps an item out of full-text search, so a false positive is
    /// a note the user can no longer find. These are the neighbours that a
    /// looser fragment would have caught.
    #[test]
    fn ordinary_applications_are_not_credential_stores() {
        for id in [
            "com.apple.TextEdit",
            "notepad.exe",
            "chrome.exe",
            "Code.exe",
            "beekeeper-studio.exe",
            "housekeeper.exe",
            "Passwords.exe",
            "",
        ] {
            assert!(!is_password_manager_app(id), "{id}");
        }
    }
}
