import Darwin
@testable import CopyPasteKit
import XCTest

/// End-to-end over a real `AF_UNIX` socket, against a stub that answers a fixed
/// script — the same shape as the Rust CLI's `StubDaemon`. This exercises the
/// framing on both sides, so the codec is covered without a running service.
///
/// If any test in this file hangs, the socket layer is the first suspect: the
/// channel sets `SO_RCVTIMEO`, so a hang means the timeout is not being applied.
final class DaemonClientTests: XCTestCase {
    private var stub: StubDaemon!

    override func tearDown() {
        stub?.stop()
        stub = nil
        super.tearDown()
    }

    func testAFullRoundTripYieldsTypedItems() async throws {
        stub = try StubDaemon(name: "roundtrip", replies: [
            #"""
            {"id":{id},"ok":true,"data":[{"id":"a","content":"hi","content_type":"text",
            "created_at":5,"pinned":false,"is_sensitive":false}]}
            """#
        ])
        let client = DaemonClient(socketPath: stub.path, timeout: 2)
        let items = try await client.list(limit: 1, offset: 0)
        XCTAssertEqual(items.first?.content, "hi")
    }

    /// The request must arrive as exactly one line of JSON with the documented
    /// envelope — asserted against what the stub actually received.
    func testTheRequestOnTheWireIsOneLineWithTheDocumentedEnvelope() async throws {
        stub = try StubDaemon(name: "wire", replies: [#"{"id":{id},"ok":true,"data":{}}"#])
        let client = DaemonClient(socketPath: stub.path, timeout: 2)
        _ = try await client.send(.pin(id: "abc", pinned: true))

        let received = try XCTUnwrap(stub.requests.first)
        XCTAssertFalse(received.contains("\n"))
        let object = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: Data(received.utf8)) as? [String: Any]
        )
        XCTAssertEqual(object["method"] as? String, "pin")
        XCTAssertEqual(object["protocol_version"] as? Int, 1)
        let params = try XCTUnwrap(object["params"] as? [String: Any])
        XCTAssertEqual(params["pinned"] as? Bool, true)
    }

    func testAMissingSocketIsReportedAsAnUnreachableService() async {
        let missing = NSTemporaryDirectory() + "copypaste-absent-\(getpid()).sock"
        try? FileManager.default.removeItem(atPath: missing)
        let client = DaemonClient(socketPath: missing, timeout: 2)
        do {
            _ = try await client.status()
            XCTFail("expected a failure")
        } catch let error as DaemonError {
            XCTAssertEqual(error, .unreachable)
            XCTAssertFalse(error.message.contains("/"))
        } catch {
            XCTFail("unexpected error type")
        }
    }

    func testHangingUpWithoutAnsweringIsTreatedAsUnreachable() async throws {
        // `nil` reply: accept, read the request, then close — which is what the
        // service does on a read timeout.
        stub = try StubDaemon(name: "hangup", replies: [nil])
        let client = DaemonClient(socketPath: stub.path, timeout: 2)
        do {
            _ = try await client.status()
            XCTFail("expected a failure")
        } catch let error as DaemonError {
            XCTAssertEqual(error, .unreachable)
        }
    }

    func testAReplyForADifferentRequestIsRejected() async throws {
        stub = try StubDaemon(name: "idmismatch", replies: [#"{"id":999999,"ok":true,"data":{}}"#])
        let client = DaemonClient(socketPath: stub.path, timeout: 2)
        do {
            _ = try await client.status()
            XCTFail("expected a failure")
        } catch let error as DaemonError {
            XCTAssertEqual(error, .malformedReply)
        }
    }

    /// Port manifest 04 §6.2: `not_ready` is retried on a fresh connection.
    func testNotReadyIsRetriedOnAFreshConnection() async throws {
        stub = try StubDaemon(name: "notready", replies: [
            #"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#,
            #"""
            {"id":{id},"ok":true,"data":{"version":"2.0.0","protocol_version":1,
            "item_count":0,"capture_running":true,"clipboard_backend":"fake"}}
            """#,
        ])
        let client = DaemonClient(socketPath: stub.path, timeout: 2)
        let status = try await client.status()
        XCTAssertEqual(status.itemCount, 0)
        XCTAssertEqual(stub.requests.count, 2)
    }

    func testNotReadyGivesUpRatherThanRetryingForever() async throws {
        let reply = #"{"id":{id},"ok":false,"error":"still starting","error_code":"not_ready"}"#
        stub = try StubDaemon(
            name: "notready-exhausted",
            replies: Array(repeating: reply, count: DaemonClient.notReadyMaxRetries + 1)
        )
        let client = DaemonClient(socketPath: stub.path, timeout: 2)
        do {
            _ = try await client.status()
            XCTFail("expected a failure")
        } catch let error as DaemonError {
            XCTAssertEqual(error, .notReady)
        }
    }
}

// MARK: - Stub service

/// A listener that answers a fixed script of replies, one per connection.
/// `nil` means "accept, read the request, then hang up".
///
/// `{id}` in a reply is replaced with the request's own id, as the protocol
/// requires.
private final class StubDaemon: @unchecked Sendable {
    let path: String
    private let listener: Int32
    private let lock = NSLock()
    private var received: [String] = []
    private var stopped = false

    var requests: [String] {
        lock.lock()
        defer { lock.unlock() }
        return received
    }

    init(name: String, replies: [String?]) throws {
        // Short, because `sun_path` is 104 bytes.
        path = "\(NSTemporaryDirectory())cp-\(name)-\(getpid()).sock"
        try? FileManager.default.removeItem(atPath: path)

        listener = socket(AF_UNIX, SOCK_STREAM, 0)
        guard listener >= 0 else { throw StubError.cannotListen }

        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard bytes.count < capacity else { throw StubError.pathTooLong }
        withUnsafeMutablePointer(to: &address.sun_path) { tuple in
            tuple.withMemoryRebound(to: CChar.self, capacity: capacity) { destination in
                for (offset, byte) in bytes.enumerated() {
                    destination[offset] = CChar(bitPattern: byte)
                }
                destination[bytes.count] = 0
            }
        }
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)

        let bound = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, listen(listener, 8) == 0 else { throw StubError.cannotListen }

        Thread.detachNewThread { [self] in
            for reply in replies {
                let connection = accept(listener, nil, nil)
                guard connection >= 0 else { return }
                if let line = Self.readLine(connection) {
                    lock.lock()
                    received.append(line)
                    lock.unlock()

                    if let reply {
                        let id = Self.requestID(in: line) ?? 0
                        var answer = reply.replacingOccurrences(of: "{id}", with: String(id))
                        // Replies are one line, like the real service's.
                        answer = answer.replacingOccurrences(of: "\n", with: "")
                        answer += "\n"
                        _ = answer.withCString { pointer in
                            Darwin.write(connection, pointer, strlen(pointer))
                        }
                    }
                }
                Darwin.close(connection)
            }
        }
    }

    func stop() {
        lock.lock()
        defer { lock.unlock() }
        guard !stopped else { return }
        stopped = true
        Darwin.close(listener)
        try? FileManager.default.removeItem(atPath: path)
    }

    private static func readLine(_ descriptor: Int32) -> String? {
        var accumulated = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            let n = buffer.withUnsafeMutableBytes { Darwin.read(descriptor, $0.baseAddress, $0.count) }
            if n <= 0 { return accumulated.isEmpty ? nil : String(decoding: accumulated, as: UTF8.self) }
            accumulated.append(contentsOf: buffer[0..<n])
            if let newline = accumulated.firstIndex(of: UInt8(ascii: "\n")) {
                return String(decoding: accumulated[accumulated.startIndex..<newline], as: UTF8.self)
            }
        }
    }

    private static func requestID(in line: String) -> UInt64? {
        guard let object = try? JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any],
              let id = object["id"] as? NSNumber
        else { return nil }
        return id.uint64Value
    }

    enum StubError: Error {
        case cannotListen
        case pathTooLong
    }
}
