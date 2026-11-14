//! Conflict copies for concurrent snippet body edits.
//!
//! Each body version names the revisions it descends from. Two versions
//! conflict only when neither descends from the other and their content
//! differs; a fast-forward over up to [`MAX_ANCESTORS`] offline edits does
//! not. The losing body becomes a new snippet whose id derives from the losing
//! revision, so every device that observes the conflict creates the same copy.

use sha2::{Digest, Sha256};

pub const MAX_ANCESTORS: usize = 16;

/// The revision of a purged body stub. Identical on every device, so stubs
/// never conflict and a late edit can be recognised against one.
pub const PURGED_REVISION: &str = "00000000000000000000000000000000";

const COPY_PREFIX: &str = "conflict-";

#[derive(Debug, Clone, Copy)]
pub struct BodyVersion<'a> {
    pub revision: &'a str,
    pub ancestors: &'a [String],
    pub text: &'a str,
    pub rich_html: Option<&'a str>,
}

pub fn concurrent(a: &BodyVersion<'_>, b: &BodyVersion<'_>) -> bool {
    a.revision != b.revision
        && a.revision != PURGED_REVISION
        && b.revision != PURGED_REVISION
        && !a.ancestors.iter().any(|revision| revision == b.revision)
        && !b.ancestors.iter().any(|revision| revision == a.revision)
        && (a.text != b.text || a.rich_html != b.rich_html)
}

/// A fresh 128-bit revision, lowercase hex.
pub fn new_revision() -> String {
    loop {
        let revision = format!("{:032x}", rand::random::<u128>());
        if revision != PURGED_REVISION {
            return revision;
        }
    }
}

/// The ancestry of an edit made from `base` while `current` is stored. An edit
/// from an older base keeps the base's own ancestry where `current` records it,
/// so the replaced `current` is recognised as concurrent.
pub fn ancestors_from(base: &str, current: Option<(&str, &[String])>) -> Vec<String> {
    let inherited: &[String] = match current {
        Some((revision, ancestors)) if revision == base => ancestors,
        Some((_, ancestors)) => ancestors
            .iter()
            .position(|ancestor| ancestor == base)
            .map_or(&[], |at| &ancestors[at + 1..]),
        None => &[],
    };
    std::iter::once(base.to_owned())
        .chain(inherited.iter().cloned())
        .take(MAX_ANCESTORS)
        .collect()
}

pub fn copy_id(losing_revision: &str) -> String {
    let digest = hex::encode(Sha256::digest(losing_revision.as_bytes()));
    format!("{COPY_PREFIX}{}", &digest[..32])
}

/// The id range every conflict copy falls in (`.` follows `-`).
pub(crate) const COPY_RANGE: (&str, &str) = (COPY_PREFIX, "conflict.");

/// `"<title> (conflict, <device>, <date>)"`, trimming the title so the whole
/// fits in `max_chars`.
pub fn copy_title(title: &str, device: &str, stamp_ms: i64, max_chars: usize) -> String {
    let (year, month, day) = utc_date(stamp_ms);
    let suffix = format!("(conflict, {device}, {year:04}-{month:02}-{day:02})");
    let room = max_chars.saturating_sub(suffix.chars().count() + 1);
    let title: String = title.trim().chars().take(room).collect();
    let title = title.trim_end();
    let full = if title.is_empty() {
        suffix
    } else {
        format!("{title} {suffix}")
    };
    full.chars().take(max_chars).collect()
}

/// Civil date of a Unix millisecond stamp (days-to-civil, proleptic Gregorian).
fn utc_date(stamp_ms: i64) -> (i64, u32, u32) {
    let days = stamp_ms.div_euclid(86_400_000) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version<'a>(revision: &'a str, ancestors: &'a [String], text: &'a str) -> BodyVersion<'a> {
        BodyVersion {
            revision,
            ancestors,
            text,
            rich_html: None,
        }
    }

    #[test]
    fn only_divergent_edits_with_different_content_conflict() {
        let base = vec!["a1".to_owned()];
        let fast_forward = vec!["a2".to_owned(), "a1".to_owned()];
        assert!(!concurrent(
            &version("a1", &[], "one"),
            &version("a3", &fast_forward, "three")
        ));
        assert!(concurrent(
            &version("b1", &base, "left"),
            &version("c1", &base, "right")
        ));
        assert!(!concurrent(
            &version("b1", &base, "same"),
            &version("c1", &base, "same")
        ));
        assert!(!concurrent(
            &version("b1", &base, "left"),
            &version(PURGED_REVISION, &[], "")
        ));
    }

    #[test]
    fn an_edit_from_a_stale_base_keeps_the_base_ancestry() {
        let current = vec!["b".to_owned(), "a".to_owned()];
        assert_eq!(ancestors_from("c", Some(("c", &current))), ["c", "b", "a"]);
        assert_eq!(ancestors_from("b", Some(("c", &current))), ["b", "a"]);
        assert_eq!(ancestors_from("x", Some(("c", &current))), ["x"]);
        let long: Vec<String> = (0..20).map(|n| n.to_string()).collect();
        assert_eq!(ancestors_from("top", Some(("top", &long))).len(), 16);
    }

    #[test]
    fn copies_are_deterministic_and_titled_within_the_limit() {
        assert_eq!(copy_id("r1"), copy_id("r1"));
        assert_ne!(copy_id("r1"), copy_id("r2"));
        assert!(copy_id("r1").as_str() > COPY_RANGE.0 && copy_id("r1").as_str() < COPY_RANGE.1);
        assert_eq!(
            copy_title("Greeting", "Laptop", 1_767_225_600_000, 200),
            "Greeting (conflict, Laptop, 2026-01-01)"
        );
        assert_eq!(utc_date(951_782_400_000), (2000, 2, 29));
        let long = copy_title(&"x".repeat(300), "Phone", 0, 200);
        assert_eq!(long.chars().count(), 200);
        assert!(long.ends_with("(conflict, Phone, 1970-01-01)"));
    }
}
