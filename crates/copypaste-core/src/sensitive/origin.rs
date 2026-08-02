/// Credential managers are an independent sensitivity floor: their copied
/// values must never enter full-text search even when their contents do not
/// match a detector rule.
pub fn is_password_manager_app(bundle_id: &str) -> bool {
    let bundle_id = bundle_id.to_ascii_lowercase();
    matches!(
        bundle_id.as_str(),
        "com.1password.1password"
            | "com.agilebits.onepassword7"
            | "com.bitwarden.desktop"
            | "org.keepassxc.keepassxc"
            | "com.dashlane.dashlane"
            | "com.lastpass.lastpass"
            | "com.apple.passwords"
    ) || bundle_id.contains("1password")
        || bundle_id.contains("bitwarden")
        || bundle_id.contains("keepass")
        || bundle_id.contains("dashlane")
        || bundle_id.contains("lastpass")
        || bundle_id.contains("protonpass")
        || bundle_id.contains("proton.pass")
        || bundle_id.contains("strongbox")
        || bundle_id.contains("secretive")
        || bundle_id.contains("keepassium")
}
