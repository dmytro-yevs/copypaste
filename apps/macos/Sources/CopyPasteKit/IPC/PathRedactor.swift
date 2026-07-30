import Foundation

/// Keeping filesystem paths out of text shown to a user.
///
/// A Swift port of `copypaste_ipc::redact::scrub_paths`, kept token-for-token
/// so the two behave the same. It is not a duplicated *decision* — the rule and
/// its rationale live in the Rust module — but a client in another language
/// cannot import a Rust function, and every client needs the guarantee.
///
/// The daemon promises not to put paths in `error` strings. This is the second
/// layer, so a daemon-side regression degrades to an unhelpful message rather
/// than printing `~/Library/…/<username>/…` into a screenshot
/// (CLAUDE.md rule 4, manifest 06 INV-12).
///
/// Deliberately eager: a false positive costs one unhelpful word, a false
/// negative leaks the username.
public enum PathRedactor {
    public static let placeholder = "<path>"

    /// Replace anything path-shaped with `<path>`, preserving whitespace so the
    /// rest of the sentence still reads normally.
    public static func scrub(_ message: String) -> String {
        var out = ""
        out.reserveCapacity(message.count)

        var token = ""
        var trailing = ""

        func flush() {
            if !token.isEmpty {
                out += looksLikePath(token) ? placeholder : token
                token = ""
            }
            out += trailing
            trailing = ""
        }

        for character in message {
            if character.isWhitespace {
                trailing.append(character)
            } else {
                if !trailing.isEmpty { flush() }
                token.append(character)
            }
        }
        flush()
        return out
    }

    /// Whether a whitespace-delimited token looks like a filesystem path.
    static func looksLikePath(_ token: String) -> Bool {
        // A message may wrap a path in punctuation — strip it before deciding.
        let core = String(token.drop(while: { "([\"'`".contains($0) }))
        guard !core.isEmpty else { return false }

        if core.hasPrefix("/") || core.hasPrefix("~/") || core.hasPrefix("./")
            || core.hasPrefix("../") || core.hasPrefix("file://")
            || core.contains("/Users/") || core.contains("/home/")
        {
            return true
        }
        // Windows-shaped, for completeness: C:\Users\name
        let bytes = Array(core.utf8)
        return bytes.count > 3 && bytes[1] == UInt8(ascii: ":") && core.contains("\\")
    }
}
