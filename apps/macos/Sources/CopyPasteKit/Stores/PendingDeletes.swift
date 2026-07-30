import Foundation

/// Deferred deletion with an undo window (manifest 06 §3.1.8, §5.3).
///
/// The row disappears immediately and the service call fires 5 s later, so
/// "delete" is instant and reversible. Three rules, each of which was a defect
/// in v1 before it was a rule:
///
/// * a second delete inside the window **commits the first immediately**, so
///   the undo affordance always refers to exactly one item;
/// * undo cancels the timer and nothing is ever sent;
/// * when the surface goes away, everything pending **commits at once** — an
///   item the user deleted must not survive because the popover closed.
///
/// CLAUDE.md rule 1 applies to the timer itself: this is `Task.sleep`, not a
/// hand-rolled scheduler. What is written here is the policy, which no library
/// knows.
@MainActor
public final class PendingDeletes {
    public static let window: Duration = .seconds(5)

    private var timers: [String: Task<Void, Never>] = [:]
    private let commit: (String) async -> Void

    /// - Parameter commit: performs the real deletion. Called at most once per
    ///   scheduled id.
    public init(commit: @escaping (String) async -> Void) {
        self.commit = commit
    }

    public var isEmpty: Bool { timers.isEmpty }

    public func isPending(_ id: String) -> Bool { timers[id] != nil }

    /// Remove after the undo window. Any earlier deletion commits now.
    public func schedule(_ id: String, after window: Duration = PendingDeletes.window) {
        commitAll()
        timers[id] = Task { [weak self] in
            try? await Task.sleep(for: window)
            guard !Task.isCancelled else { return }
            await self?.fire(id)
        }
    }

    /// Cancel a pending deletion. Answers whether there was one.
    @discardableResult
    public func cancel(_ id: String) -> Bool {
        guard let timer = timers.removeValue(forKey: id) else { return false }
        timer.cancel()
        return true
    }

    /// Commit everything outstanding immediately.
    public func commitAll() {
        let outstanding = timers
        timers.removeAll()
        for (id, timer) in outstanding {
            timer.cancel()
            Task { await commit(id) }
        }
    }

    private func fire(_ id: String) async {
        guard timers.removeValue(forKey: id) != nil else { return }
        await commit(id)
    }
}
