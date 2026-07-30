import Foundation
import Observation

/// Paired devices, pairing, and sync.
///
/// The cadences come from manifest 06 §5.1 and its token-burn rationale
/// (§5.5): peers refresh every 10 s while the window is open, and back off to
/// 30 s when there is nothing to watch. A closed window polls zero times.
///
/// Pairing state is deliberately not persisted anywhere. A minted code lives in
/// this object while its sheet is open and is dropped when it closes.
@MainActor
@Observable
public final class DevicesStore {
    public private(set) var peers: [PeerSummary] = []
    public private(set) var phase: LoadPhase = .loading
    /// Filled by `status`, for the "This Mac" row.
    public private(set) var service: StatusData?

    /// The result of the last sync run, newest first. Cleared when a new run
    /// starts, so the screen never mixes two runs.
    public private(set) var lastSync: [SyncSummary] = []
    public private(set) var isSyncing = false

    /// Set while a pairing sheet is showing a freshly minted code.
    public private(set) var mintedPairing: PairingSecret?
    public private(set) var isPairing = false

    /// A failure from the last action, mapped to a sentence.
    public private(set) var actionError: DaemonError?
    /// A failure that is not the service's — for example a malformed address
    /// typed into the accept sheet.
    public private(set) var inputError: String?

    private let client: DaemonClient
    private var pollTask: Task<Void, Never>?
    private var refreshSequence: UInt64 = 0

    private static let activeInterval: Duration = .seconds(10)
    private static let idleInterval: Duration = .seconds(30)

    public init(client: DaemonClient) {
        self.client = client
    }

    // MARK: - Visibility gating

    public func start() {
        guard pollTask == nil else { return }
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                await self.refresh()
                // Adaptive idle backoff: with no peers there is nothing whose
                // presence could change (manifest 06 §5.5).
                let interval = self.peers.isEmpty ? Self.idleInterval : Self.activeInterval
                try? await Task.sleep(for: self.phase.isHealthy ? interval : self.phase.pollInterval)
            }
        }
    }

    public func stop() {
        pollTask?.cancel()
        pollTask = nil
        actionError = nil
        inputError = nil
    }

    public func refresh() async {
        refreshSequence &+= 1
        let sequence = refreshSequence
        do {
            let found = try await client.peers().map(PeerSummary.init)
            let status = try? await client.status()
            guard sequence == refreshSequence else { return }
            if found != peers { peers = found }
            if let status { service = status }
            phase = .ready
        } catch let error as DaemonError {
            guard sequence == refreshSequence else { return }
            phase = LoadPhase(error)
        } catch {
            guard sequence == refreshSequence else { return }
            phase = .failed(.serviceFailure)
        }
    }

    // MARK: - Pairing

    /// Mint a pairing on this Mac. The returned code is read out to the other
    /// device; it is never copied to the pasteboard and never logged.
    public func createPairing(name: String) async {
        isPairing = true
        actionError = nil
        defer { isPairing = false }
        do {
            let data = try await client.pairCreate(name: name.isEmpty ? "New device" : name)
            mintedPairing = PairingSecret(data)
            await refresh()
        } catch let error as DaemonError {
            actionError = error
        } catch {
            actionError = .serviceFailure
        }
    }

    /// Forget the minted code. Called when the sheet closes, so the secret does
    /// not outlive the screen that showed it.
    public func discardMintedPairing() {
        mintedPairing = nil
    }

    /// Consume a code minted on another device.
    ///
    /// The service does all the validation; this only rejects an address that
    /// is obviously not `host:port`, so the user gets a pointed message instead
    /// of a round trip. It never says *how* a code was wrong — that is a hint
    /// about what a valid one looks like, and the service is careful about it
    /// for the same reason.
    @discardableResult
    public func acceptPairing(code: String, address: String) async -> Bool {
        inputError = nil
        actionError = nil
        let trimmedCode = code.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedAddress = address.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !trimmedCode.isEmpty else {
            inputError = "Enter the code shown on the other device."
            return false
        }
        guard Self.looksLikeHostPort(trimmedAddress) else {
            inputError = "Enter the other device’s address as host:port."
            return false
        }

        isPairing = true
        defer { isPairing = false }
        do {
            _ = try await client.pairAccept(code: trimmedCode, address: trimmedAddress)
            await refresh()
            return true
        } catch let error as DaemonError {
            actionError = error
            return false
        } catch {
            actionError = .serviceFailure
            return false
        }
    }

    /// `nonisolated`: it is a pure string check with no state, and the tests
    /// call it without a main-actor hop.
    nonisolated static func looksLikeHostPort(_ address: String) -> Bool {
        guard let separator = address.lastIndex(of: ":") else { return false }
        let host = address[address.startIndex..<separator]
        let port = address[address.index(after: separator)...]
        guard !host.isEmpty, let number = UInt16(port), number > 0 else { return false }
        return true
    }

    public func unpair(_ peer: PeerSummary) async {
        actionError = nil
        do {
            try await client.unpair(pairingID: peer.pairingID)
            peers.removeAll { $0.pairingID == peer.pairingID }
            lastSync.removeAll { $0.pairingID == peer.pairingID }
            await refresh()
        } catch let error as DaemonError {
            actionError = error
            await refresh()
        } catch {
            actionError = .serviceFailure
        }
    }

    // MARK: - Sync

    /// Sync with one peer, or with all of them when `peer` is nil.
    public func sync(_ peer: PeerSummary? = nil) async {
        guard !isSyncing else { return }
        isSyncing = true
        actionError = nil
        lastSync = []
        defer { isSyncing = false }
        do {
            let results = try await client.syncNow(pairingID: peer?.pairingID)
            lastSync = results.map(SyncSummary.init)
            await refresh()
        } catch let error as DaemonError {
            actionError = error
        } catch {
            actionError = .serviceFailure
        }
    }

    public func dismissErrors() {
        actionError = nil
        inputError = nil
    }
}
