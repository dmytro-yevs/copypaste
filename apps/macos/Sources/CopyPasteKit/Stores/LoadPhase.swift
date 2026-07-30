import Foundation

/// What a screen currently knows about the clipboard service.
///
/// Five states, because the manifest's empty-state table has five rows and the
/// difference between them is the whole point: "starting up" and "not running"
/// and "nothing copied yet" are three different sentences with three different
/// next steps, and collapsing them into an empty list is the bug
/// (manifest 06 §3.1.11, §3.2.5).
public enum LoadPhase: Sendable, Equatable {
    /// First load, nothing to show yet. Never renders "nothing here".
    case loading
    case ready
    /// The service answered `not_ready` — it is coming up.
    case starting
    /// The socket could not be reached.
    case offline
    case failed(DaemonError)

    public init(_ error: DaemonError) {
        switch error {
        case .unreachable: self = .offline
        case .notReady: self = .starting
        default: self = .failed(error)
        }
    }

    public var isHealthy: Bool { self == .ready }

    /// Manifest 06 §5.1: 3 s while healthy and visible, 5 s while degraded —
    /// never hammer a dead or initialising service. Both are only ever used
    /// while a surface is actually on screen; a hidden surface polls zero times
    /// (INV-27).
    public var pollInterval: Duration {
        switch self {
        case .ready, .loading: .seconds(3)
        case .starting, .offline, .failed: .seconds(5)
        }
    }

    public var error: DaemonError? {
        switch self {
        case .offline: .unreachable
        case .starting: .notReady
        case let .failed(error): error
        case .loading, .ready: nil
        }
    }
}
