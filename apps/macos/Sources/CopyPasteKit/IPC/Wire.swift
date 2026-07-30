import Foundation

/// Bumped on any breaking change to the request or response shape.
/// Must equal `copypaste_ipc::PROTOCOL_VERSION`.
public let protocolVersion: UInt32 = 1

/// Frames larger than this are rejected before allocation.
/// Must equal `copypaste_ipc::MAX_FRAME_BYTES`.
public let maxFrameBytes = 8 * 1024 * 1024

/// One request. `id` is echoed back so a client can match replies.
public struct Request: Encodable, Sendable {
    public let id: UInt64
    public let method: Method

    public init(id: UInt64, method: Method) {
        self.id = id
        self.method = method
    }

    public enum CodingKeys: String, CodingKey {
        case id
        case protocolVersion = "protocol_version"
        case method
        case params
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(id, forKey: .id)
        try container.encode(protocolVersion, forKey: .protocolVersion)
        try container.encode(method.wireName, forKey: .method)
        try method.encodeParams(into: &container)
    }
}

/// One reply. `ok` distinguishes success from failure without inspecting the
/// payload.
public struct Response: Decodable, Sendable {
    public let id: UInt64
    public let ok: Bool
    public let data: ResponseData?
    /// The daemon's own sentence. **Never rendered as-is** — see `DaemonError`,
    /// which scrubs it and only ever shows it as secondary detail.
    public let error: String?
    public let errorCode: ErrorCode?

    enum CodingKeys: String, CodingKey {
        case id, ok, data, error
        case errorCode = "error_code"
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UInt64.self, forKey: .id)
        ok = try container.decode(Bool.self, forKey: .ok)
        data = try container.decodeIfPresent(ResponseData.self, forKey: .data)
        error = try container.decodeIfPresent(String.self, forKey: .error)
        // An unrecognised code must not fail the whole decode: the protocol
        // permits an untagged failure, so a *new* code degrades to the same
        // thing rather than to "malformed reply".
        let rawCode = try container.decodeIfPresent(String.self, forKey: .errorCode)
        errorCode = rawCode.flatMap(ErrorCode.init(rawValue:))
    }
}

/// `copypaste_ipc::ErrorCode`, `rename_all = "snake_case"`.
public enum ErrorCode: String, Sendable, Equatable {
    case notFound = "not_found"
    case invalidRequest = "invalid_request"
    case protocolMismatch = "protocol_mismatch"
    case notReady = "not_ready"
    case internalFailure = "internal"
}

/// The payload union. `#[serde(untagged)]` on the Rust side, so the *order*
/// below is part of the contract: an untagged union is decoded by trying the
/// variants in declaration order and keeping the first that fits.
///
/// One consequence is inherited verbatim from the CLI: an **empty JSON array**
/// matches `items` before it can reach `peers` or `sync`. That is a property of
/// the encoding, not a daemon bug, so `peers()` and `syncResults()` below
/// accept both spellings of "none".
public enum ResponseData: Decodable, Sendable {
    case status(StatusData)
    case items([Item])
    case item(Item)
    case count(UInt64)
    case pairing(PairingData)
    case peers([PeerInfo])
    case sync([SyncResult])
    case empty

    public init(from decoder: any Decoder) throws {
        if let value = try? StatusData(from: decoder) {
            self = .status(value)
        } else if let value = try? [Item](from: decoder) {
            self = .items(value)
        } else if let value = try? Item(from: decoder) {
            self = .item(value)
        } else if let value = try? UInt64(from: decoder) {
            self = .count(value)
        } else if let value = try? PairingData(from: decoder) {
            self = .pairing(value)
        } else if let value = try? [PeerInfo](from: decoder) {
            self = .peers(value)
        } else if let value = try? [SyncResult](from: decoder) {
            self = .sync(value)
        } else if (try? EmptyObject(from: decoder)) != nil {
            self = .empty
        } else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath, debugDescription: "unrecognised response payload")
            )
        }
    }
}

/// Matches serde's `Empty {}` — an object, with anything or nothing in it.
private struct EmptyObject: Decodable {
    init(from decoder: any Decoder) throws {
        _ = try decoder.container(keyedBy: NoKeys.self)
    }

    private struct NoKeys: CodingKey {
        var stringValue: String
        var intValue: Int?
        init?(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { nil }
    }
}

/// An item as seen by clients.
///
/// `content` is **plaintext** — the daemon decrypts on the way out and the
/// socket is `0600`. It exists on this type because it is on the wire, and it
/// is dropped at the very next boundary: `ClipItem.init(_:)` discards it for
/// sensitive items so that no view, and no accessibility label, can ever reach
/// it. Do not hand a `Wire.Item` to a view.
public struct Item: Decodable, Sendable, Equatable {
    public let id: String
    public let content: String
    public let contentType: String
    /// Milliseconds since the Unix epoch.
    public let createdAt: Int64
    public let pinned: Bool
    public let isSensitive: Bool

    enum CodingKeys: String, CodingKey {
        case id, content, pinned
        case contentType = "content_type"
        case createdAt = "created_at"
        case isSensitive = "is_sensitive"
    }
}

public struct StatusData: Decodable, Sendable, Equatable {
    public let version: String
    public let protocolVersion: UInt32
    public let itemCount: UInt64
    public let captureRunning: Bool
    /// Which clipboard backend is live — the real pasteboard or the fake used
    /// on non-macOS hosts and in tests. Surfaced in Settings so a demo cannot
    /// be mistaken for the real thing.
    public let clipboardBackend: String

    enum CodingKeys: String, CodingKey {
        case version
        case protocolVersion = "protocol_version"
        case itemCount = "item_count"
        case captureRunning = "capture_running"
        case clipboardBackend = "clipboard_backend"
    }
}

/// A freshly minted pairing, returned once and never retrievable again.
///
/// `code` is the transferable form of the pre-shared key. Anyone holding it can
/// pair. It is never logged, never written to disk, never put on the pasteboard
/// by this app, and is held only for as long as the pairing sheet is open.
public struct PairingData: Decodable, Sendable, Equatable {
    public let code: String
    /// Non-secret identifier for the pairing. Safe to show and to log.
    public let pairingID: String
    /// Where the other device should connect, when it can be determined.
    public let listenAddr: String?

    enum CodingKeys: String, CodingKey {
        case code
        case pairingID = "pairing_id"
        case listenAddr = "listen_addr"
    }
}

public struct PeerInfo: Decodable, Sendable, Equatable {
    public let pairingID: String
    public let name: String
    public let lastAddr: String?
    public let lastSeenMs: Int64
    /// True when the peer is currently visible on the network. Discovery is a
    /// convenience, so `false` means "not seen", never "unreachable".
    public let online: Bool

    enum CodingKeys: String, CodingKey {
        case name, online
        case pairingID = "pairing_id"
        case lastAddr = "last_addr"
        case lastSeenMs = "last_seen_ms"
    }
}

public struct SyncResult: Decodable, Sendable, Equatable {
    public let pairingID: String
    public let name: String
    public let sent: UInt32
    public let received: UInt32
    /// Present when this peer failed; the rest of the run still reports.
    public let error: String?

    enum CodingKeys: String, CodingKey {
        case name, sent, received, error
        case pairingID = "pairing_id"
    }
}

// MARK: - Typed extraction
//
// The CLI's `expect_*` helpers, with the same rule: branch on shape, never
// silently default. v1's UI walked an untyped tree with 138 `.as_*()` calls
// that defaulted on a missing field; the whole point of the typed contract is
// that a wrong shape is reported.

extension ResponseData {
    public func items() throws -> [Item] {
        guard case let .items(items) = self else { throw DaemonError.unexpectedShape }
        return items
    }

    public func status() throws -> StatusData {
        guard case let .status(status) = self else { throw DaemonError.unexpectedShape }
        return status
    }

    public func item() -> Item? {
        if case let .item(item) = self { return item }
        return nil
    }

    public func count() -> UInt64? {
        if case let .count(count) = self { return count }
        return nil
    }

    public func pairing() throws -> PairingData {
        guard case let .pairing(pairing) = self else { throw DaemonError.unexpectedShape }
        return pairing
    }

    /// Peers, tolerating the empty-array ambiguity documented on `ResponseData`.
    public func peers() throws -> [PeerInfo] {
        switch self {
        case let .peers(peers): return peers
        case let .items(items) where items.isEmpty: return []
        default: throw DaemonError.unexpectedShape
        }
    }

    /// Per-peer sync results, tolerating the same ambiguity.
    public func syncResults() throws -> [SyncResult] {
        switch self {
        case let .sync(results): return results
        case let .items(items) where items.isEmpty: return []
        default: throw DaemonError.unexpectedShape
        }
    }
}
