import Foundation

/// Ranking for the search field (manifest 06 §3.1.7).
///
/// Two sources, deliberately:
///
/// * a **local** subsequence match over the previews already loaded, which is
///   instant and works while the service is busy;
/// * the service's **full-text search**, debounced, which reaches items beyond
///   the loaded page.
///
/// Local hits rank first by score; full-text-only hits follow at score 0, so
/// they are present but never displace a better local match. If full-text
/// search fails, the local filter simply stands — a search box that empties
/// itself because a background call failed is worse than a shorter list.
///
/// Sensitive items cannot appear from either source: the service never indexes
/// them, and locally they have no text to match (`ClipItem.preview` is nil).
/// That is not a filter here that someone could remove — there is nothing to
/// match against.
public enum HistorySearch {
    /// Everything the list should show for `query`.
    public static func results(
        loaded: [ClipItem],
        fullTextHits: [ClipItem],
        query: String
    ) -> [ClipItem] {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !needle.isEmpty else { return loaded }

        let scored = loaded
            .compactMap { item -> (item: ClipItem, score: Int)? in
                guard let score = score(item, needle) else { return nil }
                return (item, score)
            }
            // Stable by construction: `enumerated` order breaks ties, which
            // preserves the service's recency ordering within equal scores.
            .enumerated()
            .sorted { left, right in
                left.element.score == right.element.score
                    ? left.offset < right.offset
                    : left.element.score > right.element.score
            }
            .map(\.element.item)

        var seen = Set(scored.map(\.id))
        var results = scored
        for hit in fullTextHits where !seen.contains(hit.id) {
            seen.insert(hit.id)
            results.append(hit)
        }
        return results
    }

    /// Subsequence match, case- and diacritic-insensitive, scored so that
    /// tighter and earlier matches win. `nil` means "no match".
    static func score(_ item: ClipItem, _ query: String) -> Int? {
        guard let preview = item.preview else { return nil }
        let haystack = preview.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
        let needle = query.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)

        // A contiguous match is worth much more than a scattered one, and an
        // early match more than a late one.
        if let range = haystack.range(of: needle) {
            let offset = haystack.distance(from: haystack.startIndex, to: range.lowerBound)
            return 10_000 - min(offset, 5_000)
        }

        var index = haystack.startIndex
        var matched = 0
        var span = 0
        var firstHit: Int?
        for character in needle {
            guard let found = haystack[index...].firstIndex(of: character) else { return nil }
            if firstHit == nil {
                firstHit = haystack.distance(from: haystack.startIndex, to: found)
            }
            span += haystack.distance(from: index, to: found)
            index = haystack.index(after: found)
            matched += 1
        }
        guard matched == needle.count else { return nil }
        return max(1, 4_000 - span - (firstHit ?? 0))
    }
}
