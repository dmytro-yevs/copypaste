//! Provider-facing identifiers for organization records.
//!
//! Local organization ids are `org:v1:<64 hex>`, but cloud providers accept
//! only `[A-Za-z0-9_-]` identifiers. The colon form becomes `org-v1-<64 hex>`
//! on the way out and is restored on the way in. Only an exact match carrying
//! the organization content type is rewritten; every other id passes through.

use std::borrow::Cow;

/// The content type every organization record is stored and synced under.
pub const ORGANIZATION_CONTENT_TYPE: &str = "application/vnd.copypaste.organization+json";

const LOCAL_PREFIX: &str = "org:v1:";
const PROVIDER_PREFIX: &str = "org-v1-";
const DIGEST_HEX_LEN: usize = 64;

/// The identifier a cloud provider stores for a local `item_id`.
#[must_use]
pub fn to_provider<'a>(item_id: &'a str, content_type: &str) -> Cow<'a, str> {
    rewrite(item_id, content_type, LOCAL_PREFIX, PROVIDER_PREFIX)
}

/// The local `item_id` for an identifier a cloud provider returned.
#[must_use]
pub fn to_local<'a>(item_id: &'a str, content_type: &str) -> Cow<'a, str> {
    rewrite(item_id, content_type, PROVIDER_PREFIX, LOCAL_PREFIX)
}

fn rewrite<'a>(item_id: &'a str, content_type: &str, from: &str, to: &str) -> Cow<'a, str> {
    match item_id.strip_prefix(from) {
        Some(digest) if content_type == ORGANIZATION_CONTENT_TYPE && is_digest(digest) => {
            Cow::Owned(format!("{to}{digest}"))
        }
        _ => Cow::Borrowed(item_id),
    }
}

fn is_digest(value: &str) -> bool {
    value.len() == DIGEST_HEX_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "50f2356f8816787f9831ec4687cf642e5aa6fbf84e92deb123a2eb1f6d747f85";

    #[test]
    fn organization_ids_round_trip_in_the_provider_charset() {
        let local = format!("org:v1:{DIGEST}");
        let provider = to_provider(&local, ORGANIZATION_CONTENT_TYPE);
        assert_eq!(provider, format!("org-v1-{DIGEST}"));
        assert!(provider
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_eq!(to_local(&provider, ORGANIZATION_CONTENT_TYPE), local);
    }

    #[test]
    fn only_exact_organization_ids_are_rewritten() {
        let uuid = "0b9d2f7e-4c1a-4f0e-9a51-6f2c1d3e4b5a";
        assert!(matches!(
            to_provider(uuid, ORGANIZATION_CONTENT_TYPE),
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            to_local(uuid, ORGANIZATION_CONTENT_TYPE),
            Cow::Borrowed(_)
        ));
        let lookalike = format!("org-v1-{DIGEST}");
        assert_eq!(to_local(&lookalike, "text"), lookalike);
        let local = format!("org:v1:{DIGEST}");
        assert_eq!(to_provider(&local, "text"), local);
        for malformed in [
            format!("org-v1-{}", DIGEST.to_uppercase()),
            format!("org-v1-{}", &DIGEST[1..]),
            format!("org-v1-{DIGEST}0"),
            format!("org-v2-{DIGEST}"),
        ] {
            assert_eq!(to_local(&malformed, ORGANIZATION_CONTENT_TYPE), malformed);
        }
    }
}
