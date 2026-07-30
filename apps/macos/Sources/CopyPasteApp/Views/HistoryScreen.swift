import CopyPasteKit
import SwiftUI

/// The popover: search, list, and the per-row actions.
///
/// Behaviours worth pointing at, all of them from manifest 06:
///
/// * **Scroll stability (INV-1, INV-6).** The list binds `scrollPosition(id:)`
///   to the top-most row, so when a poll prepends a new clip the row the user
///   is reading stays where it is instead of sliding down. Combined with the
///   store's refusal to publish an unchanged list (INV-2), an idle popover does
///   not move at all.
/// * **Selection by id (INV-32)**, resolved against the current list on every
///   use, because a poll can reorder between a keypress and Return.
/// * **Copy-then-hide (INV-26).** `onCopied` fires only after the service has
///   confirmed. A failed copy leaves the popover open with the reason.
/// * **No raw errors (INV-12).** Every message on screen comes from
///   `DaemonError.message`, which is chosen by error *code*.
///
/// The app never pastes for the user: it puts the item on the pasteboard and
/// says so. Synthesising ⌘V would require the Accessibility permission, and
/// this app requires no permissions at all (see `GlobalHotkey`).
struct HistoryScreen: View {
    @Bindable var store: HistoryStore
    let service: ServiceController
    let preferences: Preferences
    let onCopied: () -> Void
    let openDevices: () -> Void
    let openSettings: () -> Void

    @State private var topItemID: String?
    @State private var showDeleteAllConfirmation = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            Divider()
            footer
        }
        .frame(width: 380, height: 520)
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 8) {
            SearchField(
                text: $store.query,
                placeholder: "Search clipboard",
                onMove: { store.moveSelection(by: $0) },
                onSubmit: copySelected,
                onCancel: onCopied
            )

            Menu {
                Button("Devices…", action: openDevices)
                Button("Settings…", action: openSettings)
                Divider()
                Button("Delete All History…") { showDeleteAllConfirmation = true }
                    .disabled(!store.phase.isHealthy)
                Divider()
                Button("Quit CopyPaste") { NSApplication.shared.terminate(nil) }
            } label: {
                Image(systemName: "ellipsis.circle")
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .help("More actions")
            .accessibilityLabel("More actions")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .confirmationDialog(
            "Delete all clipboard history?",
            isPresented: $showDeleteAllConfirmation
        ) {
            Button("Delete All", role: .destructive) {
                Task { await store.deleteAll() }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This permanently deletes every clipboard item on this Mac, including pinned ones. It can’t be undone.")
        }
    }

    // MARK: - Content

    @ViewBuilder
    private var content: some View {
        if store.visibleItems.isEmpty {
            emptyContent
        } else {
            list
        }
    }

    @ViewBuilder
    private var emptyContent: some View {
        switch store.phase {
        case .loading:
            // Never "nothing copied yet" while the first load is in flight —
            // that message flashed on every open in v1 (manifest 06 §3.5.3).
            StateView(title: "Loading…", message: "Fetching your clipboard history.", isBusy: true)
        case .offline:
            StateView(
                title: "The clipboard service isn’t running",
                message: service.isInstalled
                    ? "CopyPaste can’t show your history until the service starts."
                    : "CopyPaste can’t find the clipboard service on this Mac.",
                actionTitle: service.isInstalled ? "Start the service" : nil,
                isBusy: service.isStarting,
                action: startService
            )
        case .starting:
            StateView(
                title: "Starting up…",
                message: "The clipboard service is initializing. Your history will appear in a moment.",
                isBusy: true
            )
        case let .failed(error):
            StateView(
                title: "Couldn’t load your history",
                message: error.message,
                actionTitle: error.recovery == .restartService ? "Start the service" : nil,
                isBusy: service.isStarting,
                action: startService
            )
        case .ready:
            if store.query.isEmpty {
                StateView(
                    title: "Nothing copied yet",
                    message: "Copy something and it will appear here."
                )
            } else {
                StateView(
                    title: "No results",
                    message: "Nothing in your history matches “\(store.query)”."
                )
            }
        }
    }

    private var list: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 2) {
                    ForEach(store.visibleItems) { item in
                        HistoryRow(
                            item: item,
                            previewLines: preferences.previewLines,
                            isSelected: store.selectedID == item.id,
                            isFlashing: store.flashedID == item.id,
                            onCopy: { copy(item) },
                            onTogglePin: { Task { await store.togglePin(item) } },
                            onDelete: { store.delete(item) }
                        )
                        .id(item.id)
                    }
                }
                .scrollTargetLayout()
                .padding(.horizontal, 6)
                .padding(.vertical, 6)
            }
            // Anchored to content, not to pixels (INV-1): the bound row keeps
            // its place when items are inserted above it.
            .scrollPosition(id: $topItemID, anchor: .top)
            .onChange(of: store.selectedID) { _, newValue in
                guard let newValue, let item = store.selectedItem else { return }
                withAnimation(reduceMotion ? nil : .easeOut(duration: 0.12)) {
                    proxy.scrollTo(newValue)
                }
                // INV-9: the rows are not options in a listbox, so nothing is
                // announced automatically. Mirror the active row for VoiceOver.
                AccessibilityNotification.Announcement(item.singleLineLabel).post()
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Clipboard history")
    }

    // MARK: - Footer

    @ViewBuilder
    private var footer: some View {
        if let error = store.actionError {
            banner(error.message, detail: error.detail)
        }
        if let undoable = store.undoable {
            undoBar(for: undoable)
        } else {
            statusLine
        }
    }

    private var statusLine: some View {
        HStack(spacing: 8) {
            Text(countLabel)
            Spacer()
            // Not "⏎ paste": this app copies, and the user pastes. It has no
            // permission to press ⌘V for them, by design.
            Text("↩ copy · then ⌘V to paste")
                .foregroundStyle(.tertiary)
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .accessibilityElement(children: .combine)
    }

    private var countLabel: String {
        if !store.query.isEmpty {
            return "\(store.visibleItems.count) matching"
        }
        if store.totalCount > UInt64(store.visibleItems.count) {
            return "\(store.visibleItems.count) of \(store.totalCount)"
        }
        return store.visibleItems.count == 1 ? "1 item" : "\(store.visibleItems.count) items"
    }

    private func undoBar(for item: ClipItem) -> some View {
        HStack(spacing: 8) {
            Text("Deleted “\(String(item.singleLineLabel.prefix(40)))”")
                .lineLimit(1)
            Spacer()
            Button("Undo") { store.undoDelete() }
                .buttonStyle(.link)
        }
        .font(.caption)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Item deleted. Undo is available.")
    }

    private func banner(_ message: String, detail: String?) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(message)
            if let detail {
                Text(detail).foregroundStyle(.secondary)
            }
        }
        .font(.caption)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.quaternary)
        .accessibilityElement(children: .combine)
    }

    // MARK: - Actions

    private func copy(_ item: ClipItem) {
        Task {
            // INV-26: hide only once the service confirms.
            if await store.copy(item) { onCopied() }
        }
    }

    private func copySelected() {
        guard let item = store.selectedItem ?? store.visibleItems.first else { return }
        copy(item)
    }

    private func startService() {
        Task {
            if await service.start() {
                await store.refresh()
            }
        }
    }
}
