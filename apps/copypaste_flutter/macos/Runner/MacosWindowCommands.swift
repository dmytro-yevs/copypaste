import Cocoa

enum MacosWindowCommands {
  private static let menuActions: Set<Selector> = [
    #selector(NSWindow.performClose(_:)), NSSelectorFromString("closeAllWindows:"),
    #selector(NSApplication.terminate(_:)), #selector(NSApplication.hide(_:)),
    #selector(NSApplication.hideOtherApplications(_:)), NSSelectorFromString("showPreferences:"),
    #selector(NSWindow.performMiniaturize(_:)), #selector(NSWindow.toggleFullScreen(_:)),
  ]

  static func performKeyEquivalent(with event: NSEvent, in mainMenu: NSMenu) -> Bool {
    guard event.modifierFlags.contains(.command) else { return false }
    for item in mainMenu.items {
      guard let menu = item.submenu,
            menu.items.contains(where: {
              guard let action = $0.action else { return false }
              return menuActions.contains(action)
            }) else { continue }
      if menu.performKeyEquivalent(with: event) { return true }
    }
    return false
  }

  static func closeAll(windows: [NSWindow], sender: Any?) {
    // Use the normal close action so pairing cancellation and hide-to-tray
    // delegates retain ownership. Never dismiss an attached save sheet.
    for window in windows where
      (window.isVisible || window.isMiniaturized) &&
      window.sheetParent == nil && window.attachedSheet == nil &&
      (window.styleMask.contains(.closable) || window is QuickPastePanel)
    {
      window.performClose(sender)
    }
  }
}

final class QuickPastePanel: NSPanel {
  static let presentationStyleMask: NSWindow.StyleMask = [
    .borderless,
    .resizable,
    .fullSizeContentView,
    .nonactivatingPanel,
  ]

  var onCloseRequested: (() -> Void)?

  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { false }

  override func performClose(_ sender: Any?) {
    guard attachedSheet == nil else { return }
    onCloseRequested?()
  }

  override func validateUserInterfaceItem(_ item: NSValidatedUserInterfaceItem) -> Bool {
    if item.action == #selector(performClose(_:)) {
      return onCloseRequested != nil && attachedSheet == nil
    }
    return super.validateUserInterfaceItem(item)
  }

  override func performKeyEquivalent(with event: NSEvent) -> Bool {
    // A nonactivating panel receives keys while another app owns the menu bar.
    // AppKit owns key matching (including keyboard layouts and Caps Lock).
    // Leave the Edit menu and clip keys to Flutter's focused input/actions.
    if let menu = NSApplication.shared.mainMenu,
       MacosWindowCommands.performKeyEquivalent(with: event, in: menu) {
      return true
    }
    return super.performKeyEquivalent(with: event)
  }

  override var contentViewController: NSViewController? {
    get { super.contentViewController }
    set {
      // AppKit sizes the window from its controller; Flutter starts at zero.
      newValue?.view.setFrameSize(contentLayoutRect.size)
      super.contentViewController = newValue
    }
  }
}
