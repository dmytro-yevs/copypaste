//! Supabase account-identity helpers for cross-device mismatch detection.
//!
//! When two devices pair for cloud sync they must be signed into **the same
//! Supabase project and the same GoTrue account**.  If they are not, Supabase
//! Row-Level Security silently hides each device's rows from the other —
//! items never sync and neither side sees an error.
//!
//! This module provides:
//! - [`supabase_project_ref`] — extract the stable project identifier from a
//!   Supabase project URL.
//! - [`supabase_account_id`] — combine the project ref and GoTrue user UUID
//!   into a single opaque token that can be compared across devices.
//! - [`detect_account_mismatch`] — returns `true` when two account IDs differ,
//!   meaning the two devices will silently fail to sync.
//!
//! # Logging contract
//!
//! Neither function logs anything; callers are responsible for emitting
//! appropriate log lines (typically a `tracing::warn!` in the cloud sync
//! orchestrator when a mismatch is detected).  This keeps the module pure and
//! testable without a tracing subscriber.
//!
//! # Security / PII note
//!
//! The GoTrue user UUID (`user_id`) is not itself PII, but the Supabase project
//! URL (while not secret) should not be logged at debug verbosity in production.
//! [`supabase_project_ref`] returns only the opaque subdomain slug (e.g. `abc`
//! from `https://abc.supabase.co`), which is safe to log.

/// Extract the Supabase project reference slug from a project URL.
///
/// Supabase self-hosted projects use URLs of the form `https://<ref>.supabase.co`
/// or `https://<ref>.supabase.co/`.  This function returns the `<ref>` slug,
/// which is the stable per-project identifier that two devices must share in
/// order for their data to be visible to each other under Row-Level Security.
///
/// Returns `None` when the URL does not look like a standard Supabase cloud URL
/// (e.g. a self-hosted instance).  In that case the full URL is used as the
/// project identity component inside [`supabase_account_id`].
///
/// # Examples
/// ```
/// use copypaste_supabase::account::supabase_project_ref;
///
/// assert_eq!(
///     supabase_project_ref("https://abcdefgh.supabase.co"),
///     Some("abcdefgh".to_string()),
/// );
/// assert_eq!(
///     supabase_project_ref("https://self-hosted.example.com"),
///     None,
/// );
/// ```
pub fn supabase_project_ref(url: &str) -> Option<String> {
    // Strip scheme.
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    // Host is everything up to the first `/`, `?`, or end-of-string.
    let host = rest.split(['/', '?', '#']).next()?;
    // Remove trailing port if present.
    let host_no_port = host.split(':').next()?;
    // Standard Supabase cloud domains end in `.supabase.co`.
    let slug = host_no_port.strip_suffix(".supabase.co")?;
    // The slug itself must be non-empty and must not contain a `.`
    // (which would indicate an unexpected subdomain structure).
    if slug.is_empty() || slug.contains('.') {
        return None;
    }
    Some(slug.to_string())
}

/// Build a canonical account-identity token for a signed-in device.
///
/// The token combines the Supabase project reference and the GoTrue user UUID
/// into a single string that can be compared between devices.  Two devices with
/// the same `supabase_account_id` are guaranteed to share both the same
/// Supabase project and the same GoTrue account — their Row-Level Security
/// policies will allow each to see the other's rows and sync will work.
///
/// When `url` is not a standard Supabase cloud URL (e.g. self-hosted), the full
/// URL (lowercased, trailing slash stripped) is used as the project component.
///
/// The `user_id` is the UUID returned by GoTrue in the session's `user.id`
/// field.  It is NOT the email address (which is PII); callers must not pass the
/// email here.
///
/// # Format (stable)
///
/// `"<project_ref_or_url>|<user_id>"` — the `|` separator was chosen because
/// it cannot appear in a UUID or a valid URL-path segment, making the token
/// unambiguous.  The format is an internal implementation detail; callers should
/// treat it as opaque and compare tokens only via [`detect_account_mismatch`].
///
/// # Example
/// ```
/// use copypaste_supabase::account::supabase_account_id;
///
/// let id = supabase_account_id(
///     "https://abc.supabase.co",
///     "00000000-0000-0000-0000-000000000001",
/// );
/// assert!(id.starts_with("abc|"));
/// ```
pub fn supabase_account_id(url: &str, user_id: &str) -> String {
    let project = supabase_project_ref(url)
        .unwrap_or_else(|| url.trim_end_matches('/').to_ascii_lowercase());
    format!("{project}|{user_id}")
}

/// Returns `true` when the two account IDs represent different accounts.
///
/// A `true` result means the two devices are signed into different Supabase
/// projects or different GoTrue accounts within the same project.  In either
/// case they will silently fail to sync because RLS prevents each device from
/// seeing the other's rows.
///
/// A `false` result means the tokens are identical — the accounts match and
/// cloud sync can proceed normally.
///
/// Callers should log a `warn!` and set a status flag when this returns `true`
/// so the UI can surface an actionable error rather than leaving the user to
/// wonder why items are not appearing.
///
/// # Example
/// ```
/// use copypaste_supabase::account::{supabase_account_id, detect_account_mismatch};
///
/// let device_a = supabase_account_id("https://proj.supabase.co", "user-1");
/// let device_b = supabase_account_id("https://proj.supabase.co", "user-2");
/// assert!(detect_account_mismatch(&device_a, &device_b));
///
/// let device_c = supabase_account_id("https://proj.supabase.co", "user-1");
/// assert!(!detect_account_mismatch(&device_a, &device_c));
/// ```
pub fn detect_account_mismatch(this_account: &str, peer_account: &str) -> bool {
    this_account != peer_account
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── supabase_project_ref ─────────────────────────────────────────────────

    #[test]
    fn project_ref_standard_url() {
        assert_eq!(
            supabase_project_ref("https://abcdefgh.supabase.co"),
            Some("abcdefgh".to_string()),
        );
        // Trailing slash
        assert_eq!(
            supabase_project_ref("https://abcdefgh.supabase.co/"),
            Some("abcdefgh".to_string()),
        );
        // With path
        assert_eq!(
            supabase_project_ref("https://myproject.supabase.co/rest/v1"),
            Some("myproject".to_string()),
        );
    }

    #[test]
    fn project_ref_self_hosted_returns_none() {
        assert_eq!(supabase_project_ref("https://db.example.com"), None);
        assert_eq!(supabase_project_ref("https://supabase.example.com"), None);
    }

    #[test]
    fn project_ref_invalid_input_returns_none() {
        assert_eq!(supabase_project_ref(""), None);
        assert_eq!(supabase_project_ref("not-a-url"), None);
        // The `.supabase.co` root domain itself has no slug.
        assert_eq!(supabase_project_ref("https://supabase.co"), None);
    }

    #[test]
    fn project_ref_case_insensitive() {
        // URL scheme and host are normalised to lowercase.
        assert_eq!(
            supabase_project_ref("HTTPS://MyProject.SUPABASE.CO"),
            Some("myproject".to_string()),
        );
    }

    #[test]
    fn project_ref_with_port() {
        // Port is stripped before matching the domain suffix.
        assert_eq!(
            supabase_project_ref("https://myproject.supabase.co:5432/"),
            Some("myproject".to_string()),
        );
    }

    // ── supabase_account_id ──────────────────────────────────────────────────

    #[test]
    fn account_id_uses_project_slug_for_standard_url() {
        let id = supabase_account_id(
            "https://abc.supabase.co",
            "00000000-0000-0000-0000-000000000001",
        );
        assert_eq!(id, "abc|00000000-0000-0000-0000-000000000001");
    }

    #[test]
    fn account_id_uses_full_url_for_self_hosted() {
        let id = supabase_account_id(
            "https://db.company.internal",
            "00000000-0000-0000-0000-000000000001",
        );
        // Full URL (no recognised project slug) used as the project component.
        assert!(
            id.starts_with("https://db.company.internal|"),
            "expected full URL prefix, got: {id}"
        );
    }

    #[test]
    fn account_id_same_inputs_produce_same_token() {
        let a = supabase_account_id("https://abc.supabase.co", "uid-1");
        let b = supabase_account_id("https://abc.supabase.co", "uid-1");
        assert_eq!(a, b, "identical inputs must yield identical tokens");
    }

    // ── detect_account_mismatch ──────────────────────────────────────────────

    #[test]
    fn mismatch_detected_for_different_user_ids() {
        let device_a = supabase_account_id("https://proj.supabase.co", "user-id-1");
        let device_b = supabase_account_id("https://proj.supabase.co", "user-id-2");
        assert!(
            detect_account_mismatch(&device_a, &device_b),
            "different user IDs on same project must be detected as a mismatch"
        );
    }

    #[test]
    fn mismatch_detected_for_different_projects() {
        let device_a = supabase_account_id("https://project-x.supabase.co", "user-id-1");
        let device_b = supabase_account_id("https://project-y.supabase.co", "user-id-1");
        assert!(
            detect_account_mismatch(&device_a, &device_b),
            "same user on different projects must be detected as a mismatch"
        );
    }

    #[test]
    fn no_mismatch_for_matching_accounts() {
        let device_a =
            supabase_account_id("https://proj.supabase.co", "00000000-0000-0000-0000-000000000001");
        let device_b =
            supabase_account_id("https://proj.supabase.co", "00000000-0000-0000-0000-000000000001");
        assert!(
            !detect_account_mismatch(&device_a, &device_b),
            "identical account/project must not report a mismatch"
        );
    }

    #[test]
    fn mismatch_detected_for_different_projects_and_users() {
        // Both project AND user differ — still a mismatch.
        let device_a = supabase_account_id("https://proj-a.supabase.co", "uid-1");
        let device_b = supabase_account_id("https://proj-b.supabase.co", "uid-2");
        assert!(
            detect_account_mismatch(&device_a, &device_b),
            "completely different accounts must be detected as a mismatch"
        );
    }
}
