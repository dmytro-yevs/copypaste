import Foundation
import Observation

/// The history list's state: what is on screen, how it refreshes, and what the
/// per-row actions do.
///
/// The rules it carries from manifest 06, each of which was a shipped bug:
///
/// * **INV-27 — polling is visibility-gated.** `start()` when the popover
///   opens, `stop()` when it closes. A closed popover polls zero times, and
///   opening refreshes immediately rather than showing the last snapshot.
/// * **INV-2 / INV-3 — identical data must not produce a new list.** An idle
///   poll that returns the same ids, pins and timestamps assigns nothing, so
///   the view does not re-render and the scroll position cannot move. Every
///   local mutation clears the signature, or the next poll would compare fresh
///   truth against a stale fingerprint, "match", and discard it.
/// * **INV-32 — selection is tracked by id**, never by index: a poll can
///   reorder the list between a keypress and Return.
/// * **INV-33 — late replies must not clobber newer ones.** Every refresh is
///   sequence-tagged and only the newest is applied.
/// * **INV-29 — optimistic writes revert the specific field** they touched.
@MainActor
@Observable
public final class HistoryStore {
    /// What the list renders, after search.
    public private(set) var visibleItems: [ClipItem] = []
    public private(set) var phase: LoadPhase = .loading
    /// Total the service holds, which is not the same as the page loaded.
    public private(set) var totalCount: UInt64 = 0

    /// Tracked by id (INV-32). The index is re-resolved on every use.
    public var selectedID: String?
    /// Flashes for 700 ms after a successful copy (manifest 06 §5.3).
    public private(set) var flashedID: String?
    /// The one item a user can bring back, while its window is open.
    public private(set) var undoable: ClipItem?
    /// A failure from the last *action* (not the last poll), already mapped to
    /// a sentence. Poll failures live in `phase`.
    public private(set) var actionError: DaemonError?

    /// The search box.
    ///
    /// Written as a computed property over a tracked stored one rather than a
    /// stored property with `didSet`: `@Observable` rewrites stored properties
    /// into accessors, and stacking a property observer on top of that is not a
    /// combination worth betting the search box on. Reading `queryText` in a
    /// view still registers the dependency, and writing it still notifies.
    public var query: String {
        get { queryText }
        set {
            guard newValue != queryText else { return }
            queryText = newValue
            scheduleSearch()
        }
    }

    private var queryText = ""

    private let client: DaemonClient
    private var loaded: [ClipItem] = []
    private var fullTextHits: [ClipItem] = []
    /// `nil` means "no trustworthy fingerprint" — the next poll must apply
    /// whatever it finds (INV-3).
    private var signature: Int?
    private var refreshSequence: UInt64 = 0
    private var statusCountdown = 0
    private var pollTask: Task<Void, Never>?
    private var searchTask: Task<Void, Never>?
    private var flashTask: Task<Void, Never>?
    private var pendingDeletes: PendingDeletes!

    /// Manifest 06 §5.3.
    private static let searchDebounce: Duration = .milliseconds(250)
    private static let copyFlash: Duration = .milliseconds(700)

    public init(client: DaemonClient) {
        self.client = client
        pendingDeletes = PendingDeletes { [weak self] id in
            await self?.commitDelete(id)
        }
    }

    // MARK: - Visibility gating (INV-27)

    /// Called when the popover becomes visible.
    public func start() {
        guard pollTask == nil else { return }
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                await self.refresh()
                let interval = self.phase.pollInterval
                try? await Task.sleep(for: interval)
            }
        }
    }

    /// Called when the popover is dismissed. Stops every timer and commits any
    /// deferred deletion, so nothing survives by accident (§3.1.8).
    public func stop() {
        pollTask?.cancel()
        pollTask = nil
        searchTask?.cancel()
        searchTask = nil
        pendingDeletes.commitAll()
        undoable = nil
        actionError = nil
    }

    // MARK: - Loading

    public func refresh() async {
        refreshSequence &+= 1
        let sequence = refreshSequence
        do {
            let items = try await client.list().map(ClipItem.init)
            // The service total is a second round trip, so it rides along only
            // every tenth poll rather than doubling the idle IPC rate
            // (manifest 06 §5.5: decoupling cadences).
            var status: StatusData?
            if statusCountdown == 0 {
                status = try? await client.status()
                statusCountdown = 10
            }
            statusCountdown -= 1
            // INV-33: a slower earlier request must not overwrite this one.
            guard sequence == refreshSequence else { return }
            apply(items)
            if let status { totalCount = status.itemCount }
            phase = .ready
            actionError = nil
        } catch let error as DaemonError {
            guard sequence == refreshSequence else { return }
            phase = LoadPhase(error)
        } catch {
            guard sequence == refreshSequence else { return }
            phase = .failed(.serviceFailure)
        }
    }

    /// Assign only on real change (INV-2).
    private func apply(_ items: [ClipItem]) {
        let fresh = items.historySignature
        if let signature, signature == fresh, !loaded.isEmpty {
            return
        }
        signature = fresh
        loaded = items
        recomputeVisible()
        pruneSelection()
    }

    private func recomputeVisible() {
        // Rows whose deferred deletion has not fired yet are already gone from
        // the user's point of view.
        let source = pendingDeletes.isEmpty
            ? loaded
            : loaded.filter { !pendingDeletes.isPending($0.id) }
        let next = HistorySearch.results(loaded: source, fullTextHits: fullTextHits, query: query)
        // Same reasoning as INV-2, one level down: an unchanged filtered list
        // must not be re-assigned, or the scroll position moves for nothing.
        if next != visibleItems {
            visibleItems = next
        }
    }

    /// A selection that no longer exists must not linger as a phantom
    /// (INV-7's equivalent here): clear it rather than point at nothing.
    private func pruneSelection() {
        guard let selectedID else { return }
        if !visibleItems.contains(where: { $0.id == selectedID }) {
            self.selectedID = visibleItems.first?.id
        }
    }

    // MARK: - Search

    private func scheduleSearch() {
        recomputeVisible()
        searchTask?.cancel()
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !needle.isEmpty else {
            fullTextHits = []
            recomputeVisible()
            return
        }
        searchTask = Task { [weak self] in
            try? await Task.sleep(for: Self.searchDebounce)
            guard !Task.isCancelled, let self else { return }
            // A failure degrades to the local filter, silently and on purpose.
            let hits = (try? await self.client.search(needle).map(ClipItem.init)) ?? []
            guard !Task.isCancelled, self.query.trimmingCharacters(in: .whitespacesAndNewlines) == needle
            else { return }
            self.fullTextHits = hits
            self.recomputeVisible()
        }
    }

    // MARK: - Actions

    /// Put an item back on the pasteboard.
    ///
    /// Answers whether it worked, because the popover must not hide until it
    /// has (INV-26: copy-then-hide, never hide-then-copy).
    @discardableResult
    public func copy(_ item: ClipItem) async -> Bool {
        do {
            try await client.copy(id: item.id)
            actionError = nil
            flash(item.id)
            // No optimistic re-order: v2's `copy` does not restamp the item.
            // The capture loop sees the pasteboard write and decides — moving
            // the row here would be this app inventing an order the service
            // does not have (INV-31's underlying rule: the service is the
            // authority on position).
            signature = nil
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

    public func togglePin(_ item: ClipItem) async {
        let desired = !item.pinned
        // Optimistic, field-scoped (INV-29).
        setPinnedLocally(id: item.id, pinned: desired)
        do {
            try await client.pin(id: item.id, pinned: desired)
            actionError = nil
            signature = nil
            await refresh()
        } catch let error as DaemonError {
            setPinnedLocally(id: item.id, pinned: item.pinned)
            actionError = error
        } catch {
            setPinnedLocally(id: item.id, pinned: item.pinned)
            actionError = .serviceFailure
        }
    }

    private func setPinnedLocally(id: String, pinned: Bool) {
        guard let index = loaded.firstIndex(where: { $0.id == id }) else { return }
        guard loaded[index].pinned != pinned else { return }
        loaded[index].pinned = pinned
        signature = nil
        recomputeVisible()
    }

    /// Remove now, tell the service in 5 s, offer an undo in between.
    public func delete(_ item: ClipItem) {
        undoable = item
        signature = nil
        pendingDeletes.schedule(item.id)
        recomputeVisible()
        pruneSelection()
    }

    public func undoDelete() {
        guard let undoable else { return }
        pendingDeletes.cancel(undoable.id)
        self.undoable = nil
        signature = nil
        recomputeVisible()
        Task { await refresh() }
    }

    private func commitDelete(_ id: String) async {
        if undoable?.id == id { undoable = nil }
        do {
            try await client.delete(id: id)
        } catch let error as DaemonError {
            // The row is already gone from the list; put it back by refetching
            // rather than leaving the user with a lie.
            actionError = error
        } catch {
            actionError = .serviceFailure
        }
        signature = nil
        await refresh()
    }

    /// Erase everything. The confirmation belongs to the view; by the time this
    /// is called the user has said yes.
    public func deleteAll() async {
        pendingDeletes.commitAll()
        undoable = nil
        do {
            _ = try await client.deleteAll()
            actionError = nil
        } catch let error as DaemonError {
            actionError = error
        } catch {
            actionError = .serviceFailure
        }
        signature = nil
        await refresh()
    }

    public func dismissActionError() { actionError = nil }

    // MARK: - Selection

    /// Move the selection by `offset`, clamped at both ends — no wrapping
    /// (manifest 06 §3.1.4).
    public func moveSelection(by offset: Int) {
        guard !visibleItems.isEmpty else { return }
        let current = selectedID.flatMap { id in visibleItems.firstIndex(where: { $0.id == id }) }
        let next = min(max((current ?? -1) + offset, 0), visibleItems.count - 1)
        selectedID = visibleItems[next].id
    }

    public var selectedItem: ClipItem? {
        selectedID.flatMap { id in visibleItems.first(where: { $0.id == id }) }
    }

    private func flash(_ id: String) {
        flashTask?.cancel()
        flashedID = id
        flashTask = Task { [weak self] in
            try? await Task.sleep(for: Self.copyFlash)
            guard !Task.isCancelled else { return }
            self?.flashedID = nil
        }
    }
}
