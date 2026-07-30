import KeyboardShortcuts
import SwiftUI

/// The global shortcut that opens the popover.
///
/// **This file is the only place that imports `KeyboardShortcuts`.** Everything
/// else talks to `GlobalHotkey` and `GlobalHotkey.RecorderField`, so the
/// dependency can be replaced without touching a view.
///
/// Why a dependency at all (CLAUDE.md rule 1): a system-wide hotkey needs
/// `RegisterEventHotKey` plus a recorder control, and the recorder is where the
/// subtlety lives — manifest 06 INV-23 requires the binding to be captured from
/// the *physical* key, so a Dvorak or AZERTY layout records the same
/// combination, and A11Y-13 requires the control to announce the currently
/// bound accelerator rather than its glyphs. This library does both. Writing it
/// again is precisely the "it's only a few lines" trap rule 1 names.
///
/// Tradeoff stated rather than hidden: one third-party package, MIT, no
/// transitive dependencies, and it is macOS-only — which is fine because this
/// target is macOS-only. The Android app has its own answer.
///
/// ## This must stay permission-free
///
/// `KeyboardShortcuts` wraps Carbon `RegisterEventHotKey`, which is the one
/// public global-hotkey API that needs **no** Accessibility permission: the app
/// is told about the single combination it registered and never sees any other
/// input. That property is load-bearing, not incidental.
///
/// CopyPaste ships ad-hoc signed. An ad-hoc signature has no Team ID, so TCC
/// keys a grant to the binary's cdhash — which changes on every build. Any
/// permission the user granted would therefore be revoked by every update and
/// have to be granted again by hand. A clipboard manager that does that is
/// broken, so the app requires no permission at all.
///
/// Two things that would forfeit it, neither of which may be added here:
///
/// * `NSEvent.addGlobalMonitorForEvents` or `CGEvent.tapCreate` for hotkeys —
///   both require Accessibility;
/// * `CGEventPost` to synthesise ⌘V — also Accessibility. Selecting an item
///   puts it on the pasteboard and stops there; the user presses ⌘V. No text
///   in this app may imply otherwise.
///
/// One consequence to accept rather than work around: `RegisterEventHotKey`
/// cannot bind a modifier-only shortcut (double-tap Control and friends), so
/// the recorder does not offer one.
enum GlobalHotkey {
    /// Default `⌘⇧V`, matching v1's `CmdOrCtrl+Shift+V` (manifest 06 §7.1).
    /// The user can rebind it in Settings.
    static let toggle = KeyboardShortcuts.Name(
        "togglePopover",
        default: .init(.v, modifiers: [.command, .shift])
    )

    /// Registration failure is non-fatal (INV-24): the library logs and carries
    /// on, and the menu-bar item still opens the popover.
    ///
    /// The hop through `Task { @MainActor in }` is deliberate. The library
    /// delivers its callback on the main thread, but this way the code does not
    /// *depend* on that — an `assumeIsolated` here would be a crash if it ever
    /// stopped being true, and one run-loop turn is nothing against a keypress.
    static func onToggle(_ handler: @escaping @Sendable @MainActor () -> Void) {
        KeyboardShortcuts.onKeyUp(for: toggle) {
            Task { @MainActor in handler() }
        }
    }

    /// The recorder control for Settings.
    struct RecorderField: View {
        var body: some View {
            KeyboardShortcuts.Recorder("Show CopyPaste:", name: GlobalHotkey.toggle)
        }
    }
}
