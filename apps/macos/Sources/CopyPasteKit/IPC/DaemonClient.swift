import Foundation

/// The clipboard service client: one Unix socket, one newline-delimited JSON
/// request, one typed response.
///
/// A port of `crates/copypaste-cli/src/client.rs`, including its retry
/// contract, and deliberately no more than that:
///
/// * **One connection per request.** The service may close a connection after
///   an error reply or a read timeout, so a long-lived connection would need
///   reconnection logic that buys nothing at this call volume.
/// * **`not_ready` is retried** with a fixed 500 ms backoff, at most three
///   times, reconnecting each time (port manifest 04 §6.2). Nothing else is
///   retried — retrying arbitrary errors masks bugs (manifest 06 INV-34).
/// * **The reply's `id` must match.** A mismatch is a client-visible failure,
///   never a silently accepted answer to someone else's question.
///
/// Concurrency: the actor serialises callers, and the blocking syscalls run on
/// a private dispatch queue rather than on the actor's own (cooperative) thread
/// — a cooperative thread blocked in `read(2)` is a thread the whole app loses.
/// Nothing here touches the main thread (manifest 06 INV-37).
public actor DaemonClient {
    public static let notReadyMaxRetries = 3
    public static let notReadyBackoff: Duration = .milliseconds(500)

    private let socketPath: String
    private let timeout: TimeInterval
    private let queue = DispatchQueue(label: "com.copypaste.ipc", qos: .userInitiated)
    private var nextID: UInt64 = 1

    public init(
        socketPath: String = SocketPath.daemonSocket,
        timeout: TimeInterval = UnixSocketChannel.defaultTimeout
    ) {
        self.socketPath = socketPath
        self.timeout = timeout
    }

    /// Send one request and return its payload, or throw a `DaemonError`.
    ///
    /// Success with no payload (`data` absent) answers `nil`.
    @discardableResult
    public func send(_ method: Method) async throws -> ResponseData? {
        var attempts = 0
        while true {
            let response = try await roundTrip(method)

            if response.ok {
                return response.data
            }
            if response.errorCode == .notReady, attempts < Self.notReadyMaxRetries {
                attempts += 1
                try? await Task.sleep(for: Self.notReadyBackoff)
                continue
            }
            throw DaemonError.from(code: response.errorCode, message: response.error)
        }
    }

    /// Convenience for the many calls whose reply must carry a payload.
    public func require(_ method: Method) async throws -> ResponseData {
        guard let data = try await send(method) else { throw DaemonError.unexpectedShape }
        return data
    }

    // MARK: - One connection, one exchange

    private func roundTrip(_ method: Method) async throws -> Response {
        let id = nextID
        nextID &+= 1

        let request = Request(id: id, method: method)
        let encoder = JSONEncoder()
        // No `.prettyPrinted`: a request is exactly one line. (Newlines inside
        // string values are escaped by JSON itself, so this holds for content
        // that spans lines.)
        guard var line = try? encoder.encode(request) else {
            // Unreachable for these types, and saying *why* would risk echoing
            // a request that may carry a pairing code.
            throw DaemonError.malformedReply
        }
        line.append(UInt8(ascii: "\n"))

        // Hoisted out of the continuation so the closure captures values, not
        // the actor.
        let path = socketPath
        let timeout = timeout
        let queue = queue
        let reply: Data
        do {
            reply = try await withCheckedThrowingContinuation { continuation in
                queue.async {
                    do {
                        let channel = try UnixSocketChannel(path: path, timeout: timeout)
                        defer { channel.close() }
                        try channel.write(line)
                        continuation.resume(returning: try channel.readFrame())
                    } catch {
                        continuation.resume(throwing: error)
                    }
                }
            }
        } catch let failure as UnixSocketChannel.Failure {
            throw Self.map(failure)
        } catch {
            throw DaemonError.unreachable
        }

        guard let response = try? JSONDecoder().decode(Response.self, from: reply) else {
            throw DaemonError.malformedReply
        }
        guard response.id == id else {
            // The service answered a different request. Never treat it as this
            // request's answer.
            throw DaemonError.malformedReply
        }
        return response
    }

    /// Socket-level failures collapse to "unreachable", the way the CLI's do:
    /// a missing socket, a refused connection and a hang-up before the reply
    /// are the same situation from the user's side, and the user's next step is
    /// the same for all three.
    private static func map(_ failure: UnixSocketChannel.Failure) -> DaemonError {
        switch failure {
        case .frameTooLarge: .malformedReply
        case .pathTooLong, .cannotCreateSocket, .cannotConnect, .timedOut, .closedByPeer, .ioFailed:
            .unreachable
        }
    }
}

// MARK: - Typed calls
//
// One function per operation, so no view has to know a method name or a page
// size, and every reply shape is asserted in exactly one place.

extension DaemonClient {
    /// Page size for the history list. The service clamps with its own maximum.
    public static let pageSize: UInt32 = 200
    /// Manifest 06 §5.3: the full-text search limit.
    public static let searchLimit: UInt32 = 500

    public func status() async throws -> StatusData {
        try await require(.status).status()
    }

    public func list(limit: UInt32 = DaemonClient.pageSize, offset: UInt32 = 0) async throws -> [Item] {
        try await require(.list(limit: limit, offset: offset)).items()
    }

    public func search(_ query: String, limit: UInt32 = DaemonClient.searchLimit) async throws -> [Item] {
        try await require(.search(query: query, limit: limit)).items()
    }

    public func copy(id: String) async throws {
        try await send(.copy(id: id))
    }

    public func add(content: String) async throws {
        try await send(.add(content: content))
    }

    public func delete(id: String) async throws {
        try await send(.delete(id: id))
    }

    /// Answers how many items were removed.
    public func deleteAll() async throws -> UInt64 {
        try await require(.deleteAll).count() ?? 0
    }

    public func pin(id: String, pinned: Bool) async throws {
        try await send(.pin(id: id, pinned: pinned))
    }

    /// Mint a pairing. The returned `code` is a secret — see `PairingData`.
    public func pairCreate(name: String) async throws -> PairingData {
        try await require(.pairCreate(name: name)).pairing()
    }

    /// Consume a code minted on another device. Answers the peer it produced.
    public func pairAccept(code: String, address: String) async throws -> [PeerInfo] {
        try await require(.pairAccept(code: code, addr: address)).peers()
    }

    public func unpair(pairingID: String) async throws {
        try await send(.unpair(pairingID: pairingID))
    }

    public func peers() async throws -> [PeerInfo] {
        try await require(.peers).peers()
    }

    /// Sync with one peer, or with every known peer when `pairingID` is nil.
    /// A peer that failed reports its failure in its own result; the run
    /// continues.
    public func syncNow(pairingID: String? = nil) async throws -> [SyncResult] {
        try await require(.syncNow(pairingID: pairingID)).syncResults()
    }
}
