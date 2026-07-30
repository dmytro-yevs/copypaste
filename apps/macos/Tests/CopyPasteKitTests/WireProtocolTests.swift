@testable import CopyPasteKit
import XCTest

/// The wire contract, asserted the way the Rust side asserts it.
///
/// These mirror the tests in `crates/copypaste-cli/src/client.rs` and
/// `crates/copypaste-ipc/src/lib.rs` deliberately: if the two clients disagree
/// about the envelope, one of these fails and names the field.
final class WireProtocolTests: XCTestCase {
    private func encoded(_ method: Method, id: UInt64 = 7) throws -> [String: Any] {
        let data = try JSONEncoder().encode(Request(id: id, method: method))
        let object = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertFalse(
            String(decoding: data, as: UTF8.self).contains("\n"),
            "a request must be one line"
        )
        return object
    }

    func testRequestsSerialiseToTheDocumentedEnvelope() throws {
        let object = try encoded(.list(limit: 50, offset: 0))
        XCTAssertEqual(object["id"] as? Int, 7)
        XCTAssertEqual(object["protocol_version"] as? Int, 1)
        XCTAssertEqual(object["method"] as? String, "list")
        let params = try XCTUnwrap(object["params"] as? [String: Any])
        XCTAssertEqual(params["limit"] as? Int, 50)
        XCTAssertEqual(params["offset"] as? Int, 0)
    }

    /// Serde emits no `params` key at all for a unit variant. Matching that
    /// exactly is the point — the daemon round-trips its own output.
    func testUnitMethodsSerialiseWithoutParams() throws {
        for method in [Method.status, .deleteAll, .peers] {
            let object = try encoded(method)
            XCTAssertNil(object["params"], "\(method.wireName) must carry no params")
        }
        XCTAssertEqual(try encoded(.status)["method"] as? String, "status")
        XCTAssertEqual(try encoded(.deleteAll)["method"] as? String, "delete_all")
        XCTAssertEqual(try encoded(.peers)["method"] as? String, "peers")
    }

    func testMethodNamesAreSnakeCase() {
        XCTAssertEqual(Method.pairCreate(name: "x").wireName, "pair_create")
        XCTAssertEqual(Method.pairAccept(code: "x", addr: "y").wireName, "pair_accept")
        XCTAssertEqual(Method.syncNow(pairingID: nil).wireName, "sync_now")
        XCTAssertEqual(Method.deleteAll.wireName, "delete_all")
    }

    func testPinCarriesTheDesiredState() throws {
        let params = try XCTUnwrap(try encoded(.pin(id: "abc", pinned: false))["params"] as? [String: Any])
        XCTAssertEqual(params["id"] as? String, "abc")
        XCTAssertEqual(params["pinned"] as? Bool, false)
    }

    func testSyncNowSendsAnExplicitNullForEveryPeer() throws {
        let params = try XCTUnwrap(try encoded(.syncNow(pairingID: nil))["params"] as? [String: Any])
        XCTAssertTrue(params.keys.contains("pairing_id"))
        XCTAssertTrue(params["pairing_id"] is NSNull)

        let named = try XCTUnwrap(try encoded(.syncNow(pairingID: "abc"))["params"] as? [String: Any])
        XCTAssertEqual(named["pairing_id"] as? String, "abc")
    }

    func testUnpairUsesTheSnakeCaseField() throws {
        let params = try XCTUnwrap(try encoded(.unpair(pairingID: "abc"))["params"] as? [String: Any])
        XCTAssertEqual(params["pairing_id"] as? String, "abc")
    }

    // MARK: - Decoding

    private func decode(_ json: String) throws -> Response {
        try JSONDecoder().decode(Response.self, from: Data(json.utf8))
    }

    func testItemListsDecodeIntoTypedItems() throws {
        let response = try decode(#"""
        {"id":1,"ok":true,"data":[{"id":"a","content":"hi","content_type":"text/plain",
        "created_at":5,"pinned":true,"is_sensitive":false}]}
        """#)
        let items = try XCTUnwrap(response.data).items()
        XCTAssertEqual(items.count, 1)
        XCTAssertEqual(items[0].content, "hi")
        XCTAssertTrue(items[0].pinned)
    }

    func testAnEmptyItemListIsNotAShapeError() throws {
        let response = try decode(#"{"id":1,"ok":true,"data":[]}"#)
        XCTAssertTrue(try XCTUnwrap(response.data).items().isEmpty)
    }

    func testStatusDecodesIntoTypedStatus() throws {
        let response = try decode(#"""
        {"id":1,"ok":true,"data":{"version":"2.0.0","protocol_version":1,
        "item_count":3,"capture_running":true,"clipboard_backend":"macos"}}
        """#)
        let status = try XCTUnwrap(response.data).status()
        XCTAssertEqual(status.itemCount, 3)
        XCTAssertEqual(status.clipboardBackend, "macos")
    }

    func testACountPayloadReadsBackAsACount() throws {
        let response = try decode(#"{"id":1,"ok":true,"data":9}"#)
        XCTAssertEqual(try XCTUnwrap(response.data).count(), 9)
    }

    func testAnEmptyPayloadCarriesNeitherItemNorCount() throws {
        let response = try decode(#"{"id":1,"ok":true,"data":{}}"#)
        let data = try XCTUnwrap(response.data)
        XCTAssertNil(data.item())
        XCTAssertNil(data.count())
    }

    func testAWrongShapeIsReportedRatherThanSilentlyDefaulted() throws {
        let response = try decode(#"{"id":1,"ok":true,"data":{}}"#)
        XCTAssertThrowsError(try XCTUnwrap(response.data).items())
    }

    func testAPairingPayloadReadsBackAsAPairing() throws {
        let response = try decode(#"""
        {"id":1,"ok":true,"data":{"code":"ABCD-EFGH","pairing_id":"0123456789abcdef",
        "listen_addr":"192.168.1.24:47654"}}
        """#)
        let pairing = try XCTUnwrap(response.data).pairing()
        XCTAssertEqual(pairing.code, "ABCD-EFGH")
        XCTAssertEqual(pairing.listenAddr, "192.168.1.24:47654")
    }

    /// The untagged union tries `items` before `peers`, so a peer array must
    /// still land as peers — and an *empty* array cannot be told apart from an
    /// empty item list, which is why both spellings of "none" are accepted.
    func testAPeerListReadsBackAsPeersAndNotAsItems() throws {
        let response = try decode(#"""
        {"id":1,"ok":true,"data":[{"pairing_id":"abc","name":"phone","last_addr":null,
        "last_seen_ms":0,"online":true}]}
        """#)
        let peers = try XCTUnwrap(response.data).peers()
        XCTAssertEqual(peers.count, 1)
        XCTAssertTrue(peers[0].online)
        XCTAssertNil(peers[0].lastAddr)
    }

    func testSyncResultsReadBackAsSyncResults() throws {
        let response = try decode(#"""
        {"id":1,"ok":true,"data":[{"pairing_id":"abc","name":"phone","sent":2,
        "received":3,"error":null}]}
        """#)
        let results = try XCTUnwrap(response.data).syncResults()
        XCTAssertEqual(results[0].sent, 2)
        XCTAssertEqual(results[0].received, 3)
        XCTAssertNil(results[0].error)
    }

    func testAnEmptyListIsNoPeersAndNoSyncResults() throws {
        let response = try decode(#"{"id":1,"ok":true,"data":[]}"#)
        let data = try XCTUnwrap(response.data)
        XCTAssertTrue(try data.peers().isEmpty)
        XCTAssertTrue(try data.syncResults().isEmpty)
    }

    func testAWronglyShapedPeerReplyIsStillReported() throws {
        let response = try decode(#"{"id":1,"ok":true,"data":9}"#)
        XCTAssertThrowsError(try XCTUnwrap(response.data).peers())
    }

    // MARK: - Failures

    func testAFailureResponseBecomesATypedError() throws {
        let response = try decode(#"{"id":1,"ok":false,"error":"no such item","error_code":"not_found"}"#)
        XCTAssertFalse(response.ok)
        XCTAssertEqual(response.errorCode, .notFound)
        XCTAssertEqual(
            DaemonError.from(code: response.errorCode, message: response.error),
            .notFound(detail: "no such item")
        )
    }

    func testAnUntaggedFailureStillBecomesAnError() throws {
        let response = try decode(#"{"id":1,"ok":false,"error":"unknown method"}"#)
        XCTAssertNil(response.errorCode)
        XCTAssertEqual(DaemonError.from(code: nil, message: "unknown method"), .serviceFailure)
    }

    /// A code this client has never heard of must degrade to an untagged
    /// failure, not to "malformed reply".
    func testAnUnknownErrorCodeDoesNotFailTheDecode() throws {
        let response = try decode(#"{"id":1,"ok":false,"error":"nope","error_code":"teapot"}"#)
        XCTAssertNil(response.errorCode)
        XCTAssertEqual(DaemonError.from(code: response.errorCode, message: response.error), .serviceFailure)
    }

    // MARK: - No paths, ever

    /// Every message this client can put on screen, checked for a path.
    func testNoUserFacingMessageContainsAPath() {
        let errors: [DaemonError] = [
            .unreachable, .notReady, .protocolMismatch, .serviceFailure,
            .malformedReply, .unexpectedShape,
            .notFound(detail: nil), .rejected(detail: nil),
        ]
        for error in errors {
            XCTAssertFalse(error.message.contains("/"), "\(error): \(error.message)")
            XCTAssertFalse(error.message.contains("~"), "\(error): \(error.message)")
        }
    }

    /// A daemon-side regression that leaks a path must be scrubbed before it
    /// can be shown as detail.
    func testDaemonSuppliedPathsAreScrubbedFromDetail() {
        let error = DaemonError.from(
            code: .notFound,
            message: "could not open /Users/dmitriy/Library/Application Support/CopyPaste/x.db"
        )
        let detail = error.detail ?? ""
        XCTAssertFalse(detail.contains("dmitriy"), detail)
        XCTAssertTrue(detail.contains("<path>"), detail)
    }

    func testAnInternalFailureCarriesNoDetailAtAll() {
        XCTAssertNil(DaemonError.from(code: .internalFailure, message: "boom at /tmp/x").detail)
    }
}
