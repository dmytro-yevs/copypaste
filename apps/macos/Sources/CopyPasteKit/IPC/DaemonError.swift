import Foundation

/// Everything that can go wrong between this app and the clipboard service,
/// and the exact words the user is allowed to see for each.
///
/// Two rules are enforced here rather than at every call site:
///
/// * **Branch on `error_code`, never on the `error` string** (port manifest 04,
///   I9). Nothing in this file inspects message text to decide anything.
/// * **Raw errors are never rendered** (manifest 06 INV-12). `message` is a
///   fixed sentence chosen by code. The daemon's own sentence is available as
///   `detail`, and even that has been through `PathRedactor` before it gets
///   here, so a daemon-side regression cannot leak the local username into a
///   screenshot.
///
/// `detail` exists because the daemon's pairing failures are genuinely useful,
/// fixed, path-free sentences ("that pairing code is not valid") and a pairing
/// screen that replaced them with "something went wrong" would be worse. It is
/// only ever shown as *secondary* text under the mapped sentence, never as the
/// primary message, and never for an internal failure.
///
/// The canonical vocabulary is "clipboard service" / "background service".
/// Never "daemon" (manifest 06 §3.2.5).
public enum DaemonError: Error, Sendable, Equatable {
    /// The socket is absent, refused the connection, or the service hung up
    /// before answering. The three are indistinguishable and the user's next
    /// step is the same for all of them.
    case unreachable
    /// `not_ready` — still starting, and the retry budget was spent.
    case notReady
    case protocolMismatch
    case notFound(detail: String?)
    /// `invalid_request` — the service understood and refused.
    case rejected(detail: String?)
    /// `internal`, or a failure with no code at all.
    case serviceFailure
    /// A reply this client could not parse, or one for a different request.
    case malformedReply
    /// A well-formed reply carrying the wrong payload shape for the request.
    case unexpectedShape

    /// Build from a failure response. `code` decides; the string is only ever
    /// carried through as scrubbed detail.
    public static func from(code: ErrorCode?, message: String?) -> DaemonError {
        let detail = message.map(PathRedactor.scrub).flatMap { $0.isEmpty ? nil : $0 }
        switch code {
        case .notFound: return .notFound(detail: detail)
        case .invalidRequest: return .rejected(detail: detail)
        case .protocolMismatch: return .protocolMismatch
        case .notReady: return .notReady
        case .internalFailure, nil: return .serviceFailure
        }
    }

    /// The sentence shown to the user. Contains no path, no error code, and no
    /// daemon-supplied text.
    public var message: String {
        switch self {
        case .unreachable:
            "The clipboard service isn’t running."
        case .notReady:
            "The clipboard service is still starting up."
        case .protocolMismatch:
            "CopyPaste and the clipboard service are different versions. Update both, then restart the service."
        case .notFound:
            "That item is no longer there."
        case .rejected:
            "The clipboard service couldn’t do that."
        case .serviceFailure:
            "The clipboard service ran into a problem."
        case .malformedReply, .unexpectedShape:
            "CopyPaste couldn’t understand the clipboard service’s reply. The two may be different versions."
        }
    }

    /// Secondary text, when the service gave a usable reason. Already scrubbed.
    public var detail: String? {
        switch self {
        case let .notFound(detail), let .rejected(detail): detail
        default: nil
        }
    }

    /// What the user can do about it, when there is something.
    public var recovery: Recovery? {
        switch self {
        case .unreachable: .startService
        case .notReady: .wait
        case .protocolMismatch, .serviceFailure, .malformedReply, .unexpectedShape: .restartService
        case .notFound, .rejected: nil
        }
    }

    public enum Recovery: Sendable, Equatable {
        case startService
        case restartService
        /// Nothing to do but let it finish; the UI keeps polling.
        case wait

        public var title: String {
            switch self {
            case .startService: "Start the service"
            case .restartService: "Restart the service"
            case .wait: "Waiting…"
            }
        }
    }

    /// True while the condition is expected to clear on its own, so the UI
    /// should keep polling (at the backoff cadence) rather than give up.
    public var isTransient: Bool {
        switch self {
        case .unreachable, .notReady: true
        default: false
        }
    }
}
