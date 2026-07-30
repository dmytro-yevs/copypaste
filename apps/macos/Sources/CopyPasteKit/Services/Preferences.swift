import Foundation
import Observation

/// The handful of things this app remembers.
///
/// Read **per field, with a per-field default** (manifest 06 INV-21). An
/// out-of-range `previewLines` must not also discard `allowScreenshots`, and a
/// value nobody recognises is replaced rather than propagated. Nothing here
/// decodes a single blob, which is what makes that true rather than aspirational
/// — one corrupt field cannot take the others with it.
///
/// Each preference is a *stored* property (so `@Observable` actually tracks it,
/// and SwiftUI re-renders) that is written through to `UserDefaults` on change.
/// A purely computed wrapper over `UserDefaults` would look tidier and would
/// never notify anybody.
@MainActor
@Observable
public final class Preferences {
    public static let previewLineRange: ClosedRange<Int> = 1...6

    @ObservationIgnored private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        // Per-field, with a per-field fallback. Order is irrelevant: no read
        // here can affect another.
        storedPreviewLines = Self.readPreviewLines(defaults)
        storedStartServiceOnLaunch = defaults.object(forKey: Key.startServiceOnLaunch) as? Bool ?? true
        storedAllowScreenshots = defaults.object(forKey: Key.allowScreenshots) as? Bool ?? false
    }

    private enum Key {
        static let previewLines = "previewLines"
        static let startServiceOnLaunch = "startServiceOnLaunch"
        static let allowScreenshots = "allowScreenshots"
    }

    // MARK: - How many lines of a clip a row shows
    //
    // The row reserves the full cap whatever the content is (INV-5).

    private var storedPreviewLines: Int

    public var previewLines: Int {
        get { storedPreviewLines }
        set {
            let clamped = min(
                max(newValue, Self.previewLineRange.lowerBound),
                Self.previewLineRange.upperBound
            )
            guard clamped != storedPreviewLines else { return }
            storedPreviewLines = clamped
            defaults.set(clamped, forKey: Key.previewLines)
        }
    }

    private static func readPreviewLines(_ defaults: UserDefaults) -> Int {
        guard defaults.object(forKey: Key.previewLines) != nil else { return 2 }
        let stored = defaults.integer(forKey: Key.previewLines)
        return previewLineRange.contains(stored) ? stored : 2
    }

    // MARK: - Start the clipboard service when CopyPaste launches

    private var storedStartServiceOnLaunch: Bool

    public var startServiceOnLaunch: Bool {
        get { storedStartServiceOnLaunch }
        set {
            guard newValue != storedStartServiceOnLaunch else { return }
            storedStartServiceOnLaunch = newValue
            defaults.set(newValue, forKey: Key.startServiceOnLaunch)
        }
    }

    // MARK: - Screen-capture exclusion
    //
    // Off by default: CopyPaste's windows are excluded from screen recordings
    // and screenshots (manifest 06 INV-35). A clipboard history is exactly the
    // window nobody wants in a screenshare.

    private var storedAllowScreenshots: Bool

    public var allowScreenshots: Bool {
        get { storedAllowScreenshots }
        set {
            guard newValue != storedAllowScreenshots else { return }
            storedAllowScreenshots = newValue
            defaults.set(newValue, forKey: Key.allowScreenshots)
        }
    }
}
