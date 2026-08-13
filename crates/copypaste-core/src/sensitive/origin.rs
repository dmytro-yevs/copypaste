//! Which origins are credential stores, whatever their content looks like.

/// Windows process image names that no substring below would recognise.
///
/// Each verified from the vendor's own deployment documentation:
///
/// * `nordpass.exe` — support.nordpass.com/hc/en-us/articles/360004799257
/// * `enpass.exe` — help.enpass.io/business/latest/microsoft-365/packaging-enpass-for-windows-via-microsoft-intune
/// * `keeperpasswordmanager.exe` — docs.keeper.io/enterprise-guide/deploying-keeper-to-end-users/desktop-application
/// * `robotaskbaricon.exe` / `robotaskbaricon-x64.exe` — RoboForm's tray
///   process, which owns the clipboard on a copy.
const WINDOWS_CREDENTIAL_STORES: [&str; 5] = [
    "nordpass.exe",
    "enpass.exe",
    "keeperpasswordmanager.exe",
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

/// Credential managers are an independent sensitivity floor: their copied
/// values must never enter full-text search even when their contents do not
/// match a detector rule.
pub fn is_password_manager_app(bundle_id: &str) -> bool {
    let bundle_id = bundle_id.trim().to_ascii_lowercase();
    if matches!(
        bundle_id.as_str(),
        "com.1password.1password"
            | "com.agilebits.onepassword7"
            | "com.bitwarden.desktop"
            | "org.keepassxc.keepassxc"
            | "com.dashlane.dashlane"
            | "com.lastpass.lastpass"
            | "com.apple.passwords"
    ) || WINDOWS_CREDENTIAL_STORES.contains(&bundle_id.as_str())
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
