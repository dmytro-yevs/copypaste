import Foundation

/// A paired device, as the Devices window sees it.
public struct PeerSummary: Identifiable, Sendable, Equatable {
    public var id: String { pairingID }

    /// Non-secret. Safe to show and to log.
    public let pairingID: String
    public let name: String
    /// Last address that worked. Shown because the *other* device may need it,
    /// and because "never reached" is the difference between a peer that can be
    /// synced from here and one that cannot.
    public let lastAddress: String?
    /// `nil` when the service has never heard from this peer.
    public let lastSeen: Date?
    /// Visible on the network right now.
    ///
    /// `false` means "not seen", never "unreachable" — discovery is multicast,
    /// so a peer on another subnet is reachable by address and still reads as
    /// offline. The label must say so.
    public let online: Bool

    public init(_ info: PeerInfo) {
        pairingID = info.pairingID
        name = info.name
        lastAddress = info.lastAddr
        lastSeen = info.lastSeenMs > 0
            ? Date(timeIntervalSince1970: TimeInterval(info.lastSeenMs) / 1000)
            : nil
        online = info.online
    }

    /// A short, non-secret identifier for the row's secondary line.
    public var shortPairingID: String { String(pairingID.prefix(8)) }

    public var hasEverSynced: Bool { lastSeen != nil }
}

/// The outcome of one peer's half of a sync run.
public struct SyncSummary: Identifiable, Sendable, Equatable {
    public var id: String { pairingID }

    public let pairingID: String
    public let name: String
    public let sent: UInt32
    public let received: UInt32
    /// Already scrubbed of anything path-shaped. Shown as secondary text under
    /// a fixed sentence, never as the whole message (manifest 06 INV-12).
    public let failure: String?

    public init(_ result: SyncResult) {
        pairingID = result.pairingID
        name = result.name
        sent = result.sent
        received = result.received
        failure = result.error.map(PathRedactor.scrub)
    }

    public var succeeded: Bool { failure == nil }
}

/// A freshly minted pairing.
///
/// `code` is the Noise pre-shared key in transferable form: whoever holds it
/// can pair with this Mac. The rules this app follows for it, all of them
/// deliberate:
///
/// * never logged — not at any level, not in a `description`, not in an error;
/// * never persisted;
/// * never placed on the pasteboard (this is a clipboard manager: a "Copy code"
///   button would file the pairing secret into the user's own history, and then
///   sync it);
/// * held only while the pairing sheet is open, and dropped when it closes;
/// * revealed only on an explicit action, so it is not on screen behind the
///   user's back or in a screenshare.
public struct PairingSecret: Sendable {
    public let code: String
    public let pairingID: String
    public let listenAddress: String?

    public init(_ data: PairingData) {
        code = data.code
        pairingID = data.pairingID
        listenAddress = data.listenAddr
    }

    /// The code in groups, for reading aloud and for typing without losing
    /// your place. Grouping is display-only; the value sent to the other
    /// device is always `code`.
    public var groupedForDisplay: [String] {
        stride(from: 0, to: code.count, by: 4).map { offset in
            let start = code.index(code.startIndex, offsetBy: offset)
            let end = code.index(start, offsetBy: min(4, code.count - offset))
            return String(code[start..<end])
        }
    }
}

/// `CustomStringConvertible` is overridden so an accidental interpolation of a
/// `PairingSecret` cannot print the code. There is no `Codable` conformance for
/// the same reason.
extension PairingSecret: CustomStringConvertible, CustomDebugStringConvertible {
    public var description: String { "PairingSecret(pairingID: \(pairingID))" }
    public var debugDescription: String { description }
}
