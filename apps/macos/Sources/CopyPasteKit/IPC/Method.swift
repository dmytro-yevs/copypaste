import Foundation

/// Every operation the daemon supports.
///
/// A one-for-one mirror of `copypaste_ipc::Method`, which is
///
/// ```rust
/// #[serde(tag = "method", content = "params", rename_all = "snake_case")]
/// ```
///
/// flattened into `Request`. That combination — adjacent tagging, flattened —
/// produces exactly this on the wire:
///
/// ```json
/// {"id":7,"protocol_version":1,"method":"list","params":{"limit":50,"offset":0}}
/// {"id":8,"protocol_version":1,"method":"status"}
/// ```
///
/// Note the second line: serde emits **no `params` key at all** for a unit
/// variant, so neither does this. That is the shape the daemon round-trips its
/// own output through, which makes it the shape least likely to surprise it.
public enum Method: Sendable, Equatable {
    case status
    case list(limit: UInt32, offset: UInt32)
    case search(query: String, limit: UInt32)
    case copy(id: String)
    case add(content: String)
    case delete(id: String)
    case deleteAll
    case pin(id: String, pinned: Bool)

    // ---- peer-to-peer sync -------------------------------------------------
    case pairCreate(name: String)
    case pairAccept(code: String, addr: String)
    case unpair(pairingID: String)
    case peers
    case syncNow(pairingID: String?)

    /// The `method` string. `rename_all = "snake_case"` on the Rust enum.
    var wireName: String {
        switch self {
        case .status: "status"
        case .list: "list"
        case .search: "search"
        case .copy: "copy"
        case .add: "add"
        case .delete: "delete"
        case .deleteAll: "delete_all"
        case .pin: "pin"
        case .pairCreate: "pair_create"
        case .pairAccept: "pair_accept"
        case .unpair: "unpair"
        case .peers: "peers"
        case .syncNow: "sync_now"
        }
    }

    /// Whether this method's *arguments* are a secret.
    ///
    /// `pair_accept` carries the pairing code, which is the Noise pre-shared
    /// key in transferable form. Nothing may log one — not the request, not a
    /// failure, not a `description`. This flag is what the client consults
    /// before it describes a request anywhere.
    var carriesSecret: Bool {
        if case .pairAccept = self { return true }
        return false
    }
}

// MARK: - Encoding

extension Method {
    /// Write `params` into the request's own keyed container (the Rust side
    /// flattens the enum into `Request`, so there is no nesting beyond this).
    ///
    /// Optionals are encoded as an explicit `null` rather than omitted, again
    /// to match what serde itself writes. Serde accepts both — a missing
    /// `Option` field deserialises as `None` — but matching its output keeps
    /// one fewer assumption in play.
    func encodeParams(into container: inout KeyedEncodingContainer<Request.CodingKeys>) throws {
        switch self {
        case .status, .deleteAll, .peers:
            // Unit variants carry no `params` key at all.
            break
        case let .list(limit, offset):
            try container.encode(ListParams(limit: limit, offset: offset), forKey: .params)
        case let .search(query, limit):
            try container.encode(SearchParams(query: query, limit: limit), forKey: .params)
        case let .copy(id):
            try container.encode(IDParams(id: id), forKey: .params)
        case let .add(content):
            try container.encode(ContentParams(content: content), forKey: .params)
        case let .delete(id):
            try container.encode(IDParams(id: id), forKey: .params)
        case let .pin(id, pinned):
            try container.encode(PinParams(id: id, pinned: pinned), forKey: .params)
        case let .pairCreate(name):
            try container.encode(NameParams(name: name), forKey: .params)
        case let .pairAccept(code, addr):
            try container.encode(PairAcceptParams(code: code, addr: addr), forKey: .params)
        case let .unpair(pairingID):
            try container.encode(PairingIDParams(pairingID: pairingID), forKey: .params)
        case let .syncNow(pairingID):
            try container.encode(OptionalPairingIDParams(pairingID: pairingID), forKey: .params)
        }
    }
}

// Small structs rather than a hand-written `encode(to:)` per case: the field
// names are the contract, and a struct puts them next to their types.

private struct ListParams: Encodable {
    let limit: UInt32
    let offset: UInt32
}

private struct SearchParams: Encodable {
    let query: String
    let limit: UInt32
}

private struct IDParams: Encodable {
    let id: String
}

private struct ContentParams: Encodable {
    let content: String
}

private struct PinParams: Encodable {
    let id: String
    let pinned: Bool
}

private struct NameParams: Encodable {
    let name: String
}

private struct PairAcceptParams: Encodable {
    let code: String
    let addr: String
}

private struct PairingIDParams: Encodable {
    let pairingID: String

    enum CodingKeys: String, CodingKey {
        case pairingID = "pairing_id"
    }
}

/// `SyncNow { pairing_id: Option<String> }`. Encoded explicitly so `nil`
/// becomes `null` instead of being dropped by the synthesised
/// `encodeIfPresent`.
private struct OptionalPairingIDParams: Encodable {
    let pairingID: String?

    enum CodingKeys: String, CodingKey {
        case pairingID = "pairing_id"
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        if let pairingID {
            try container.encode(pairingID, forKey: .pairingID)
        } else {
            try container.encodeNil(forKey: .pairingID)
        }
    }
}
