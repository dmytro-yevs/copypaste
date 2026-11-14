//! Ranking snippet search hits (spec A-3).
//!
//! Tiers order first. An exact abbreviation match lists duplicates by local
//! recency only; the other tiers give pinned snippets a tie-break boost before
//! recency and the source order (the full-text rank).

use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SearchTier {
    ExactAbbreviation,
    AbbreviationPrefix,
    Title,
    FullText,
}

/// The abbreviation tier of a normalized key for a normalized query.
pub fn abbreviation_tier(query_key: &str, abbreviation_key: &str) -> Option<SearchTier> {
    if abbreviation_key == query_key {
        Some(SearchTier::ExactAbbreviation)
    } else if abbreviation_key.starts_with(query_key) {
        Some(SearchTier::AbbreviationPrefix)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankSignals {
    pub tier: SearchTier,
    pub pinned: bool,
    pub last_used_at: Option<i64>,
    /// Position within the tier's own source order.
    pub position: usize,
}

pub fn compare(a: &RankSignals, b: &RankSignals) -> Ordering {
    a.tier
        .cmp(&b.tier)
        .then_with(|| {
            if a.tier == SearchTier::ExactAbbreviation {
                Ordering::Equal
            } else {
                b.pinned.cmp(&a.pinned)
            }
        })
        .then_with(|| b.last_used_at.cmp(&a.last_used_at))
        .then_with(|| a.position.cmp(&b.position))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(tier: SearchTier, pinned: bool, last_used_at: Option<i64>) -> RankSignals {
        RankSignals {
            tier,
            pinned,
            last_used_at,
            position: 0,
        }
    }

    #[test]
    fn tiers_dominate_and_pins_boost_only_below_the_exact_tier() {
        let exact_recent = signals(SearchTier::ExactAbbreviation, false, Some(9));
        let exact_pinned = signals(SearchTier::ExactAbbreviation, true, Some(1));
        let prefix_pinned = signals(SearchTier::AbbreviationPrefix, true, None);
        let prefix_recent = signals(SearchTier::AbbreviationPrefix, false, Some(9));
        let title = signals(SearchTier::Title, true, Some(10));
        let mut hits = [
            title,
            prefix_recent,
            exact_pinned,
            prefix_pinned,
            exact_recent,
        ];
        hits.sort_by(compare);
        assert_eq!(
            hits,
            [
                exact_recent,
                exact_pinned,
                prefix_pinned,
                prefix_recent,
                title
            ]
        );
        assert_eq!(
            abbreviation_tier("sig", "sig2"),
            Some(SearchTier::AbbreviationPrefix)
        );
        assert_eq!(abbreviation_tier("sig2", "sig"), None);
    }
}
