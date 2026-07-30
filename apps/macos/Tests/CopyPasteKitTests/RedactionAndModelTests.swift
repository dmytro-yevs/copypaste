@testable import CopyPasteKit
import XCTest

/// A port of the Rust tests in `copypaste_ipc::redact`. Same inputs, same
/// expectations — if the two implementations drift, one of these fails.
final class PathRedactorTests: XCTestCase {
    func testRedactsUnixPathsAndKeepsTheSentence() {
        XCTAssertEqual(
            PathRedactor.scrub("could not open /Users/alice/Library/x.sock for writing"),
            "could not open <path> for writing"
        )
        XCTAssertEqual(PathRedactor.scrub("failed at /home/bob/.local/share/db"), "failed at <path>")
    }

    func testRedactsRelativeHomeAndURLForms() {
        for input in ["~/Library/Application", "./data/db", "../secrets", "file:///home/carol/x"] {
            XCTAssertTrue(PathRedactor.scrub(input).hasPrefix("<path>"), input)
        }
    }

    func testRedactsPathsWrappedInPunctuation() {
        XCTAssertEqual(PathRedactor.scrub("(/home/dan/x)"), "<path>")
        XCTAssertEqual(PathRedactor.scrub("\"/Users/eve/y\""), "<path>")
    }

    func testLeavesOrdinaryTextAlone() {
        let message = "the clipboard service is not running; start it and try again"
        XCTAssertEqual(PathRedactor.scrub(message), message)
        XCTAssertEqual(PathRedactor.scrub("read/write error"), "read/write error")
        let id = "item not found: 3f2a91c4-0000-4000-8000-000000000000"
        XCTAssertEqual(PathRedactor.scrub(id), id)
    }

    func testPreservesWhitespaceShape() {
        XCTAssertEqual(PathRedactor.scrub("a  /home/x  b"), "a  <path>  b")
        XCTAssertEqual(PathRedactor.scrub("line\n/home/x\n"), "line\n<path>\n")
        XCTAssertEqual(PathRedactor.scrub("a  b\nc"), "a  b\nc")
    }

    func testNoUsernameSurvivesARealisticSocketError() {
        let leaked = "connection refused (os error 61) on "
            + "/Users/dmytro/Library/Application Support/com.copypaste.CopyPaste/daemon.sock"
        XCTAssertFalse(PathRedactor.scrub(leaked).contains("dmytro"))
    }
}

/// The sensitive-content rule, asserted at the only place it can be enforced.
final class ClipItemTests: XCTestCase {
    private func wireItem(
        id: String = "a",
        content: String = "hello",
        sensitive: Bool = false,
        pinned: Bool = false,
        createdAt: Int64 = 1_700_000_000_000
    ) throws -> Item {
        let json = """
        {"id":"\(id)","content":"\(content)","content_type":"text",
        "created_at":\(createdAt),"pinned":\(pinned),"is_sensitive":\(sensitive)}
        """
        return try JSONDecoder().decode(Item.self, from: Data(json.utf8))
    }

    /// Manifest 06 INV-10: the plaintext of a sensitive item must not exist
    /// anywhere a view or the accessibility tree can reach. Here it does not
    /// exist at all.
    func testASensitiveItemKeepsNoPlaintext() throws {
        let item = ClipItem(try wireItem(content: "sk-live-0123456789", sensitive: true))
        XCTAssertNil(item.preview)
        XCTAssertEqual(item.kind, .sensitive)
        XCTAssertEqual(item.singleLineLabel, ClipItem.redactedLabel)
        XCTAssertFalse(item.singleLineLabel.contains("sk-live"))
        // And nothing reflected off the value either.
        XCTAssertFalse(String(describing: item).contains("sk-live"))
    }

    func testANonSensitiveItemKeepsItsPreview() throws {
        let item = ClipItem(try wireItem(content: "hello world"))
        XCTAssertEqual(item.preview, "hello world")
        XCTAssertEqual(item.kind, .text)
    }

    func testLongContentIsTruncatedAndSaysSo() throws {
        let long = String(repeating: "x", count: ClipItem.previewCharacterLimit + 50)
        let item = ClipItem(try wireItem(content: long))
        XCTAssertEqual(item.preview?.count, ClipItem.previewCharacterLimit)
        XCTAssertTrue(item.isTruncated)
    }

    func testSingleLineLabelCollapsesWhitespace() throws {
        let item = ClipItem(try wireItem(content: "one\\ntwo\\tthree"))
        XCTAssertEqual(item.singleLineLabel, "one two three")
    }

    func testKindDetection() throws {
        XCTAssertEqual(ClipItem(try wireItem(content: "https://example.com/x")).kind, .url)
        XCTAssertEqual(ClipItem(try wireItem(content: "not a url")).kind, .text)
        XCTAssertEqual(ClipItem(try wireItem(content: "line\\nline")).kind, .multiline)
        // A scheme we do not render specially must not be mistaken for one.
        XCTAssertEqual(ClipItem(try wireItem(content: "mailto:a@example.com")).kind, .text)
    }

    /// Manifest 06 INV-2: identical data must produce an identical signature,
    /// and any change the user can see must change it.
    func testSignatureIsStableAcrossIdenticalDataAndMovesOnChange() throws {
        let first = [ClipItem(try wireItem(id: "a")), ClipItem(try wireItem(id: "b"))]
        let same = [ClipItem(try wireItem(id: "a")), ClipItem(try wireItem(id: "b"))]
        XCTAssertEqual(first.historySignature, same.historySignature)

        let pinned = [ClipItem(try wireItem(id: "a", pinned: true)), ClipItem(try wireItem(id: "b"))]
        XCTAssertNotEqual(first.historySignature, pinned.historySignature)

        let reordered = [ClipItem(try wireItem(id: "b")), ClipItem(try wireItem(id: "a"))]
        XCTAssertNotEqual(first.historySignature, reordered.historySignature)

        let restamped = [ClipItem(try wireItem(id: "a", createdAt: 1)), ClipItem(try wireItem(id: "b"))]
        XCTAssertNotEqual(first.historySignature, restamped.historySignature)
    }
}

final class HistorySearchTests: XCTestCase {
    private func item(_ id: String, _ content: String, sensitive: Bool = false) throws -> ClipItem {
        let json = """
        {"id":"\(id)","content":"\(content)","content_type":"text",
        "created_at":1,"pinned":false,"is_sensitive":\(sensitive)}
        """
        return ClipItem(try JSONDecoder().decode(Item.self, from: Data(json.utf8)))
    }

    func testAnEmptyQueryReturnsEverythingUnchanged() throws {
        let loaded = [try item("a", "one"), try item("b", "two")]
        XCTAssertEqual(HistorySearch.results(loaded: loaded, fullTextHits: [], query: "   "), loaded)
    }

    func testContiguousMatchesOutrankScatteredOnes() throws {
        let exact = try item("a", "clipboard")
        let scattered = try item("b", "c l i p b o a r d")
        let results = HistorySearch.results(loaded: [scattered, exact], fullTextHits: [], query: "clip")
        XCTAssertEqual(results.first?.id, "a")
    }

    /// A sensitive item has no text to match, so it cannot be found by typing
    /// the secret the user was never shown.
    func testSensitiveItemsAreUnmatchable() throws {
        let secret = try item("a", "sk-live-0123456789", sensitive: true)
        XCTAssertTrue(HistorySearch.results(loaded: [secret], fullTextHits: [], query: "sk-live").isEmpty)
    }

    func testFullTextOnlyHitsAppendAfterLocalMatches() throws {
        let local = try item("a", "alpha")
        let remote = try item("z", "something else entirely")
        let results = HistorySearch.results(loaded: [local], fullTextHits: [remote], query: "alpha")
        XCTAssertEqual(results.map(\.id), ["a", "z"])
    }

    func testFullTextHitsAreNotDuplicated() throws {
        let local = try item("a", "alpha")
        let results = HistorySearch.results(loaded: [local], fullTextHits: [local], query: "alpha")
        XCTAssertEqual(results.map(\.id), ["a"])
    }

    func testMatchingIsCaseAndDiacriticInsensitive() throws {
        let accented = try item("a", "Résumé draft")
        XCTAssertNotNil(HistorySearch.score(accented, "resume"))
    }
}

final class AddressValidationTests: XCTestCase {
    func testHostPortShapes() {
        XCTAssertTrue(DevicesStore.looksLikeHostPort("192.168.1.24:47654"))
        XCTAssertTrue(DevicesStore.looksLikeHostPort("my-mac.local:47654"))
        XCTAssertFalse(DevicesStore.looksLikeHostPort("192.168.1.24"))
        XCTAssertFalse(DevicesStore.looksLikeHostPort(":47654"))
        XCTAssertFalse(DevicesStore.looksLikeHostPort("host:0"))
        XCTAssertFalse(DevicesStore.looksLikeHostPort("host:not-a-port"))
        XCTAssertFalse(DevicesStore.looksLikeHostPort(""))
    }
}

/// The socket path has to agree with `copypaste_ipc::socket_path()`, which is
/// `directories`' macOS convention for `com.copypaste.CopyPaste`.
final class SocketPathTests: XCTestCase {
    func testSocketPathMatchesTheServiceConvention() {
        let path = SocketPath.daemonSocket
        XCTAssertTrue(path.hasSuffix("/Library/Application Support/com.copypaste.CopyPaste/daemon.sock"), path)
        XCTAssertTrue(path.hasPrefix(NSHomeDirectory()), path)
    }
}
