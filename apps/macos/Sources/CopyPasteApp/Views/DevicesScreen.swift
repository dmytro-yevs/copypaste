import CopyPasteKit
import SwiftUI

/// Paired devices: what is paired, whether it is reachable, and the four things
/// a user can do about it — pair, sync, unpair, and read the result.
///
/// Rules carried from manifest 06 §3.2:
///
/// * "Online" means *seen on the network*, never *reachable*. Discovery is
///   multicast, so a peer on another subnet is perfectly syncable and still
///   reads as offline. The label says which one it means (INV-15's underlying
///   rule: never present unverified network hearsay as fact).
/// * Loading states are visible things, not empty elements.
/// * The vocabulary is "clipboard service", never "daemon".
/// * No raw error text (INV-12).
struct DevicesScreen: View {
    @Bindable var store: DevicesStore
    let service: ServiceController

    @State private var sheet: PairingSheet?
    @State private var pendingUnpair: PeerSummary?

    enum PairingSheet: Identifiable {
        case show
        case enter

        var id: Int {
            switch self {
            case .show: 0
            case .enter: 1
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            toolbar
            Divider()
            phaseContent(for: store.phase)
        }
        .frame(minWidth: 420, minHeight: 360)
        .sheet(item: $sheet) { which in
            switch which {
            case .show: ShowPairingCodeSheet(store: store)
            case .enter: EnterPairingCodeSheet(store: store)
            }
        }
        .confirmationDialog(
            "Unpair “\(pendingUnpair?.name ?? "this device")”?",
            isPresented: .init(get: { pendingUnpair != nil }, set: { if !$0 { pendingUnpair = nil } })
        ) {
            Button("Unpair", role: .destructive) {
                if let peer = pendingUnpair {
                    Task { await store.unpair(peer) }
                }
                pendingUnpair = nil
            }
            Button("Cancel", role: .cancel) { pendingUnpair = nil }
        } message: {
            Text("This Mac will stop syncing with that device. The other device keeps its half of the pairing until it unpairs too.")
        }
    }

    // MARK: - Toolbar

    private var toolbar: some View {
        HStack(spacing: 8) {
            Button("Pair a Device…") { sheet = .show }
            Button("Enter a Code…") { sheet = .enter }
            Spacer()
            Button {
                Task { await store.sync() }
            } label: {
                if store.isSyncing {
                    ProgressView().controlSize(.small)
                } else {
                    Text("Sync Now")
                }
            }
            .disabled(store.peers.isEmpty || store.isSyncing || !store.phase.isHealthy)
        }
        .padding(12)
        .disabled(!store.phase.isHealthy && store.phase != .loading)
    }

    // MARK: - Body

    /// Not named `body(for:)`: a method sharing its base name with the `View`
    /// requirement is legal and confusing in equal measure.
    @ViewBuilder
    private func phaseContent(for phase: LoadPhase) -> some View {
        switch phase {
        case .loading:
            StateView(title: "Loading…", message: "Fetching your paired devices.", isBusy: true)
        case .offline:
            StateView(
                title: "The clipboard service isn’t running",
                message: "Devices and sync need the clipboard service.",
                actionTitle: service.isInstalled ? "Start the service" : nil,
                isBusy: service.isStarting,
                action: { Task { if await service.start() { await store.refresh() } } }
            )
        case .starting:
            StateView(
                title: "Starting…",
                message: "CopyPaste is starting up. Your devices will appear in a moment.",
                isBusy: true
            )
        case let .failed(error):
            StateView(
                title: "Couldn’t load your devices",
                message: error.message,
                actionTitle: "Try again",
                action: { Task { await store.refresh() } }
            )
        case .ready:
            list
        }
    }

    private var list: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                if let error = store.actionError {
                    MessageBox(title: error.message, detail: error.detail, isError: true)
                }
                thisMac
                if store.peers.isEmpty {
                    MessageBox(
                        title: "No other devices paired",
                        detail: "Pair your phone or another Mac to sync your clipboard — end-to-end encrypted.",
                        isError: false
                    )
                } else {
                    ForEach(store.peers) { peer in
                        PeerRow(
                            peer: peer,
                            result: store.lastSync.first { $0.pairingID == peer.pairingID },
                            isSyncing: store.isSyncing,
                            onSync: { Task { await store.sync(peer) } },
                            onUnpair: { pendingUnpair = peer }
                        )
                    }
                }
            }
            .padding(12)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Paired devices")
    }

    private var thisMac: some View {
        GroupBox {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("This Mac").font(.headline)
                    if let service = store.service {
                        Text("Clipboard service \(service.version) · \(service.itemCount) items")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityElement(children: .combine)
    }
}

/// One paired device.
private struct PeerRow: View {
    let peer: PeerSummary
    let result: SyncSummary?
    let isSyncing: Bool
    let onSync: () -> Void
    let onUnpair: () -> Void

    var body: some View {
        GroupBox {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: "laptopcomputer.and.iphone")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: 3) {
                    Text(peer.name).font(.headline)
                    Text(presence)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    if let result {
                        Text(resultText(result))
                            .font(.caption)
                            .foregroundStyle(result.succeeded ? AnyShapeStyle(.secondary) : AnyShapeStyle(Color.red))
                    }
                }

                Spacer(minLength: 8)

                VStack(alignment: .trailing, spacing: 4) {
                    Button("Sync", action: onSync).disabled(isSyncing)
                    Button("Unpair", action: onUnpair)
                }
                .controlSize(.small)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(peer.name). \(presence)")
    }

    /// Never claims reachability it does not have.
    private var presence: String {
        var parts: [String] = []
        parts.append(peer.online ? "Visible on this network" : "Not seen on this network")
        if let lastSeen = peer.lastSeen {
            parts.append("last synced \(lastSeen.formatted(.relative(presentation: .named)))")
        } else {
            parts.append("never synced")
        }
        return parts.joined(separator: " · ")
    }

    private func resultText(_ result: SyncSummary) -> String {
        if let failure = result.failure {
            return "Last sync didn’t finish — \(failure)"
        }
        return "Sent \(result.sent), received \(result.received)"
    }
}

/// A framed sentence, used for empty states and for failures inside a list.
struct MessageBox: View {
    let title: String
    var detail: String?
    var isError = false

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.callout.weight(.medium))
                    .foregroundStyle(isError ? AnyShapeStyle(Color.red) : AnyShapeStyle(.primary))
                if let detail {
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityElement(children: .combine)
    }
}
