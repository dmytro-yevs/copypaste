import AppKit
import CopyPasteKit
import SwiftUI

/// A reusable, lazily-created window hosting a SwiftUI view.
///
/// Two rules live here rather than in every window:
///
/// * **Closing hides, it does not quit** (manifest 06 INV-36). A menu-bar app
///   that terminates when its last window closes leaves the user with nothing
///   in the menu bar; only Quit ends the process.
/// * **Content protection** (INV-35): the window is excluded from screen
///   capture unless the user has explicitly allowed screenshots. Devices and
///   Settings both show things worth keeping out of a screenshare — a pairing
///   code most of all.
@MainActor
final class UtilityWindow: NSObject, NSWindowDelegate {
    private let title: String
    private let size: NSSize
    private let preferences: Preferences
    private let content: () -> AnyView
    private var window: NSWindow?

    init(
        title: String,
        size: NSSize,
        preferences: Preferences,
        content: @escaping () -> AnyView
    ) {
        self.title = title
        self.size = size
        self.preferences = preferences
        self.content = content
    }

    func show() {
        let window = existingOrNew()
        window.sharingType = preferences.allowScreenshots ? .readOnly : .none
        // An accessory app has to ask for activation; without it the window
        // opens behind whatever the user was doing.
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    func close() {
        window?.orderOut(nil)
    }

    private func existingOrNew() -> NSWindow {
        if let window { return window }
        let created = NSWindow(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        created.title = title
        created.titlebarAppearsTransparent = false
        created.isReleasedWhenClosed = false
        created.delegate = self
        created.contentViewController = NSHostingController(rootView: content())
        created.setContentSize(size)
        created.center()
        // Everything must stay reachable when the window is made small — the
        // manifest's responsive minimum (A11Y-15), translated to this app's
        // much smaller windows.
        created.minSize = NSSize(width: 420, height: 360)
        window = created
        return created
    }

    // MARK: - NSWindowDelegate

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sender.orderOut(nil)
        return false
    }
}
