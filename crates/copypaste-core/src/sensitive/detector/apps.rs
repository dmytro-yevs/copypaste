/// Bundle IDs / process names for apps whose clipboard content should always
/// be treated as sensitive regardless of content patterns (e.g. password managers).
pub(super) static SENSITIVE_APP_BUNDLE_IDS: &[&str] = &[
    // Password managers
    "com.1password.1password",
    "com.1password.7.1password",
    "com.agilebits.onepassword",
    "com.agilebits.onepassword4",
    "com.agilebits.onepassword-osx-helper",
    "com.bitwarden.desktop",
    "com.bitwarden.desktop.safari",
    "com.keepassxc.keepassxc",
    "org.keepassxc.keepassxc-browser",
    "com.lastpass.lastpass",
    "de.peterb.Dashlane",
    "com.dashlane.dashlane",
    "com.enpass.Enpass",
    "net.sourceforge.keepass",
    "com.stegosafe.StegSafe",
    "com.webpas.webpas",
    "com.roboform.roboform",
    "com.nordpass.macos",
    "com.logmeininc.lastpass",
    // Process name fragments (matched as substring)
    "1password",
    "bitwarden",
    "keepass",
    "dashlane",
    "lastpass",
    "enpass",
    "nordpass",
    "roboform",
];

/// Returns true if the given app bundle ID or process name is a known sensitive app
/// (e.g. a password manager). Match is case-insensitive substring on the lowercased input.
pub fn is_sensitive_app(app_bundle_id: &str) -> bool {
    let lower = app_bundle_id.to_lowercase();
    SENSITIVE_APP_BUNDLE_IDS
        .iter()
        .any(|&known| lower.contains(known))
}
