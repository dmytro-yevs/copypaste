import AppKit
import CopyPasteKit
import SwiftUI

/// The menu-bar item and its popover.
///
/// Behaviours carried from manifest 06 that AppKit has to provide:
///
/// * **INV-25 — dismissing hands focus back to the app the user came from**,
///   not to CopyPaste. The frontmost application is recorded before the popover
///   opens and reactivated when it closes; otherwise every copy leaves the user
///   in a menu-bar app with nowhere to paste.
/// * **INV-26 — copy-then-hide.** The popover closes *after* the service
///   confirms the copy. On failure it stays open with the error. That ordering
///   is owned by `HistoryScreen`, which calls back here only on success.
/// * **INV-27 — polling is visibility-gated.** The store starts on show and
///   stops on close.
/// * **INV-35 — content protection.** The popover's window is excluded from
///   screen capture unless the user has opted in.
@MainActor
final class StatusItemController: NSObject, NSPopoverDelegate {
    private let statusItem: NSStatusItem
    private let popover = NSPopover()
    private let history: HistoryStore
    private let preferences: Preferences

    /// The app that was frontmost when the popover opened.
    private var previousApplication: NSRunningApplication?

    init(
        history: HistoryStore,
        service: ServiceController,
        preferences: Preferences,
        openDevices: @escaping () -> Void,
        openSettings: @escaping () -> Void
    ) {
        self.history = history
        self.preferences = preferences
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        super.init()

        if let button = statusItem.button {
            // A template image tracks the menu bar's own appearance in light,
            // dark and reduced-transparency modes without us tinting anything.
            let image = NSImage(
                systemSymbolName: "doc.on.clipboard",
                accessibilityDescription: "CopyPaste clipboard history"
            )
            image?.isTemplate = true
            button.image = image
            if image == nil { button.title = "CP" }
            button.target = self
            button.action = #selector(statusItemClicked)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
            button.setAccessibilityLabel("CopyPaste clipboard history")
        }

        popover.behavior = .transient
        popover.animates = !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        popover.delegate = self
        popover.contentViewController = NSHostingController(
            rootView: HistoryScreen(
                store: history,
                service: service,
                preferences: preferences,
                onCopied: { [weak self] in self?.hidePopover() },
                openDevices: {
                    openDevices()
                },
                openSettings: {
                    openSettings()
                }
            )
        )
    }

    // MARK: - Showing and hiding

    @objc private func statusItemClicked() {
        guard let event = NSApp.currentEvent else {
            toggle()
            return
        }
        if event.type == .rightMouseUp || event.modifierFlags.contains(.control) {
            showMenu()
        } else {
            toggle()
        }
    }

    func toggle() {
        popover.isShown ? hidePopover() : showPopover()
    }

    func showPopover() {
        guard let button = statusItem.button else { return }
        previousApplication = NSWorkspace.shared.frontmostApplication

        popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        // Without this the popover of an accessory app can come up without key
        // focus, and the search field silently swallows nothing.
        NSApp.activate(ignoringOtherApps: true)
        applyContentProtection()
        history.start()
    }

    func hidePopover() {
        popover.performClose(nil)
    }

    /// Excluded from screen recordings and screenshots unless the user says
    /// otherwise (INV-35). A failure here is non-fatal by construction: the
    /// window may not exist yet, and that is not worth an error.
    private func applyContentProtection() {
        popover.contentViewController?.view.window?.sharingType =
            preferences.allowScreenshots ? .readOnly : .none
    }

    // MARK: - NSPopoverDelegate

    func popoverDidClose(_ notification: Notification) {
        history.stop()
        // INV-25: give the keyboard back to whatever the user was using. Only
        // when CopyPaste is still the active app — if the user clicked straight
        // into another window, stealing focus back would be worse.
        if NSApp.isActive, let previousApplication, previousApplication.bundleIdentifier != Bundle.main.bundleIdentifier {
            previousApplication.activate()
        }
        previousApplication = nil
    }

    // MARK: - Right-click menu

    private func showMenu() {
        let menu = NSMenu()
        menu.addItem(withTitle: "Show CopyPaste", action: #selector(menuShow), keyEquivalent: "")
            .target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit CopyPaste", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        // `menu`-then-`nil` rather than assigning `statusItem.menu`: leaving a
        // menu attached would replace the left-click popover permanently.
        statusItem.menu = menu
        statusItem.button?.performClick(nil)
        statusItem.menu = nil
    }

    @objc private func menuShow() {
        showPopover()
    }
}
