import Foundation
import Observation

/// The clipboard service as a thing the user can see and start.
///
/// One instance, shared by every surface that offers "Start the service", so
/// two windows cannot each spawn one. Manifest 06 §3.1.11 and §3.2.5 both hang
/// their offline state off this.
@MainActor
@Observable
public final class ServiceController {
    public private(set) var status: StatusData?
    public private(set) var isStarting = false
    /// Already a finished sentence, and free of paths.
    public private(set) var failure: String?

    private let client: DaemonClient

    public init(client: DaemonClient) {
        self.client = client
    }

    /// Whether a service binary exists to start. Drives the difference between
    /// "Start the service" and telling the user it is not installed.
    public var isInstalled: Bool { ServiceLauncher.isInstalled }

    @discardableResult
    public func refresh() async -> Bool {
        do {
            status = try await client.status()
            return true
        } catch {
            status = nil
            return false
        }
    }

    /// Start it, and wait until it actually answers.
    @discardableResult
    public func start() async -> Bool {
        guard !isStarting else { return false }
        isStarting = true
        failure = nil
        defer { isStarting = false }

        do {
            try await ServiceLauncher.startAndWait(client: client)
            await refresh()
            return true
        } catch let error as ServiceLauncher.Failure {
            failure = error.message
            return false
        } catch {
            failure = ServiceLauncher.Failure.couldNotStart.message
            return false
        }
    }

    public func dismissFailure() { failure = nil }

    /// A warning worth showing in Settings: the service is running against the
    /// stub pasteboard, so nothing is really being captured. v1 surfaced this
    /// so a demo could not be mistaken for the real thing.
    public var isUsingFakeClipboard: Bool {
        guard let backend = status?.clipboardBackend else { return false }
        return backend != "macos"
    }

    /// True when the service speaks a protocol this app does not.
    public var hasProtocolMismatch: Bool {
        guard let status else { return false }
        return status.protocolVersion != protocolVersion
    }
}
