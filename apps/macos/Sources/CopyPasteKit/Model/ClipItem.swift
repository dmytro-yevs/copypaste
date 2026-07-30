import Foundation

/// A history entry as the interface sees it.
///
/// **This is the boundary that enforces manifest 06 INV-10 / A11Y-3.** A
/// sensitive item's plaintext is discarded *here*, at construction, and never
/// stored on this type. Not blurred, not masked, not held in a private field
/// that a view could reach for: absent. That makes the requirement structural
/// rather than a rule every future row view has to remember —
///
/// * it cannot reach the view hierarchy, because no view can obtain it;
/// * it cannot reach the accessibility tree, for the same reason;
/// * it cannot be matched by the local search filter against text the user was
///   never shown (manifest 06 §3.5.3);
/// * and it cannot appear in a crash log or a memory dump of this process.
///
/// Nothing is lost by dropping it. Copying goes through the service
/// (`Method.copy(id:)`), which reads the item itself — the app never needs the
/// bytes.
///
/// Non-sensitive previews are truncated too. The app is a picker, not a viewer,
/// and holding whole clipboard payloads in a menu-bar process that is resident
/// all day is a cost with no matching benefit.
public struct ClipItem: Identifiable, Sendable, Equatable {
    /// How much of a preview is ever retained.
    public static let previewCharacterLimit = 512

    public let id: String
    public let createdAt: Date
    /// Settable inside this module only, and only by `HistoryStore`, which
    /// flips it optimistically and puts it back if the service refuses
    /// (manifest 06 INV-29). No view can change it.
    public internal(set) var pinned: Bool
    public let isSensitive: Bool
    public let kind: Kind
    /// `nil` for a sensitive item, and only for a sensitive item.
    public let preview: String?
    /// True when the retained preview is shorter than the stored item.
    public let isTruncated: Bool

    public init(_ item: Item) {
        id = item.id
        createdAt = Date(timeIntervalSince1970: TimeInterval(item.createdAt) / 1000)
        pinned = item.pinned
        isSensitive = item.isSensitive

        if item.isSensitive {
            preview = nil
            isTruncated = false
            kind = .sensitive
        } else {
            let trimmed = String(item.content.prefix(Self.previewCharacterLimit))
            preview = trimmed
            isTruncated = trimmed.count < item.content.count
            kind = Kind(content: trimmed)
        }
    }

    /// What the row shows and what VoiceOver reads. A single line: interior
    /// newlines and tabs collapse to spaces, as the v1 tray labels did, so a
    /// multi-line clip cannot push a row's real height past what was reserved
    /// for it.
    public var singleLineLabel: String {
        guard let preview else { return Self.redactedLabel }
        let collapsed = String(preview.map { $0.isNewline || $0 == "\t" ? " " : $0 })
        return collapsed
            .replacingOccurrences(of: " {2,}", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespaces)
    }

    /// The one string a sensitive row is allowed to render or announce.
    public static let redactedLabel = "Sensitive item — hidden"

    /// Content kinds this app distinguishes.
    ///
    /// Deliberately few. v1 had eleven, each with its own hue, and the v2
    /// service only ever writes `text`; inventing the rest would be inventing a
    /// visual language nobody has decided on yet.
    public enum Kind: String, Sendable, Equatable {
        case text
        case multiline
        case url
        case sensitive

        /// Derived from the content, not from `Item.content_type`: the v2
        /// service writes `"text"` for everything it captures, so a switch over
        /// the wire field would have exactly one arm and would go stale
        /// silently the day that changes.
        init(content: String) {
            let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.contains(where: \.isNewline) {
                self = .multiline
            } else if Self.looksLikeWebURL(trimmed) {
                self = .url
            } else {
                self = .text
            }
        }

        static func looksLikeWebURL(_ text: String) -> Bool {
            guard !text.contains(" "), text.count < 2048 else { return false }
            guard let components = URLComponents(string: text), let scheme = components.scheme else {
                return false
            }
            return (scheme == "http" || scheme == "https") && (components.host?.isEmpty == false)
        }
    }
}

extension Array where Element == ClipItem {
    /// The dedup signature from manifest 06 INV-2: a poll that returns
    /// byte-identical data must not produce a new list, because the scroll
    /// anchoring keys on the list actually changing, and because an idle poll
    /// should cost no layout work at all.
    ///
    /// Hashes exactly what v1 hashed — id, pinned, timestamp — so a content
    /// edit is impossible to miss (items are immutable once stored) while a
    /// re-order or a pin change is caught.
    public var historySignature: Int {
        var hasher = Hasher()
        for item in self {
            hasher.combine(item.id)
            hasher.combine(item.pinned)
            hasher.combine(item.createdAt)
        }
        return hasher.finalize()
    }
}
