import Cocoa
import XCTest
@testable import CopyPaste_Dev

final class MacosWindowCommandsTests: XCTestCase {
  override func setUp() {
    super.setUp()
    _ = NSApplication.shared
  }

  private func event(_ key: String, _ modifiers: NSEvent.ModifierFlags) throws -> NSEvent {
    try XCTUnwrap(NSEvent.keyEvent(
      with: .keyDown, location: .zero, modifierFlags: modifiers,
      timestamp: 0, windowNumber: 0, context: nil, characters: key,
      charactersIgnoringModifiers: key, isARepeat: false, keyCode: 0
    ))
  }

  private func window() -> NSWindow {
    let window = NSWindow(
      contentRect: NSRect(x: -10000, y: -10000, width: 100, height: 100),
      styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false
    )
    window.isReleasedWhenClosed = false
    return window
  }

  private func installMenu(_ menu: NSMenu) {
    let app = NSApplication.shared
    let previousMenu = app.mainMenu
    addTeardownBlock { app.mainMenu = previousMenu }
    let root = NSMenu()
    let item = NSMenuItem()
    item.submenu = menu
    root.addItem(item)
    app.mainMenu = root
  }

  func testCloseAllUsesDelegateGatesAndLeavesHiddenWindowsAlone() {
    let allowed = window()
    let blocked = window()
    let hidden = window()
    let gate = CloseGate()
    blocked.delegate = gate
    defer { [allowed, blocked, hidden].forEach { $0.close() } }
    allowed.orderFront(nil)
    blocked.orderFront(nil)

    MacosWindowCommands.closeAll(windows: [allowed, blocked, hidden], sender: self)

    XCTAssertFalse(allowed.isVisible)
    XCTAssertTrue(blocked.isVisible)
    XCTAssertEqual(gate.requests, 1)
    XCTAssertFalse(hidden.isVisible)
    gate.allow = true
    MacosWindowCommands.closeAll(windows: [blocked], sender: self)
    XCTAssertFalse(blocked.isVisible)
    XCTAssertEqual(gate.requests, 2)
  }

  func testCloseAllDoesNotDismissAnAttachedSheet() {
    let parent = window()
    let sheet = window()
    let gate = CloseGate()
    parent.delegate = gate
    parent.beginSheet(sheet)
    defer { parent.endSheet(sheet); sheet.close(); parent.close() }

    MacosWindowCommands.closeAll(windows: [parent, sheet], sender: self)

    XCTAssertTrue(parent.attachedSheet === sheet)
    XCTAssertEqual(gate.requests, 0)
  }

  func testBorderlessPanelCloseUsesItsOwnerAndCanBeValidated() throws {
    let panel = QuickPastePanel(
      contentRect: .zero, styleMask: QuickPastePanel.presentationStyleMask,
      backing: .buffered, defer: true
    )
    panel.isReleasedWhenClosed = false
    defer { panel.close() }
    var requests = 0
    panel.onCloseRequested = { requests += 1 }
    let closeItem = NSMenuItem(title: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
    closeItem.target = panel
    let menu = NSMenu()
    menu.addItem(closeItem)
    installMenu(menu)
    XCTAssertTrue(panel.validateUserInterfaceItem(closeItem))
    XCTAssertTrue(panel.performKeyEquivalent(with: try event("w", .command)))
    XCTAssertEqual(requests, 1)
    XCTAssertTrue(panel.performKeyEquivalent(with: try event("w", [.command, .capsLock])))
    XCTAssertEqual(requests, 2)
    panel.performClose(self)
    XCTAssertEqual(requests, 3)
    panel.onCloseRequested = nil
    XCTAssertFalse(panel.validateUserInterfaceItem(closeItem))
  }

  func testOptionCommandWClosesPanelAndOtherWindowsThroughTheirOwners() throws {
    let panel = QuickPastePanel(
      contentRect: NSRect(x: -10000, y: -10000, width: 100, height: 100),
      styleMask: QuickPastePanel.presentationStyleMask, backing: .buffered, defer: false
    )
    panel.isReleasedWhenClosed = false
    let other = window()
    defer { panel.close(); other.close() }
    var requests = 0
    panel.onCloseRequested = { requests += 1; panel.orderOut(nil) }
    defer { panel.onCloseRequested = nil }
    let target = CloseAllTarget()
    let menu = NSMenu()
    let item = NSMenuItem(title: "Close All Windows", action: #selector(CloseAllTarget.closeAllWindows(_:)), keyEquivalent: "w")
    item.keyEquivalentModifierMask = [.command, .option]
    item.target = target
    menu.addItem(item)
    installMenu(menu)
    panel.orderFront(nil)
    other.orderFront(nil)

    XCTAssertTrue(panel.performKeyEquivalent(with: try event("w", [.command, .option])))

    XCTAssertEqual(requests, 1)
    XCTAssertFalse(panel.isVisible)
    XCTAssertFalse(other.isVisible)
  }

  func testCloseMenuFindsTheNonactivatingKeyPanelThroughTheResponderChain() throws {
    let panel = QuickPastePanel(
      contentRect: NSRect(x: -10000, y: -10000, width: 100, height: 100),
      styleMask: QuickPastePanel.presentationStyleMask, backing: .buffered, defer: false
    )
    panel.isReleasedWhenClosed = false
    defer { panel.close() }
    var requests = 0
    panel.onCloseRequested = { requests += 1; panel.orderOut(nil) }
    defer { panel.onCloseRequested = nil }
    let menu = NSMenu()
    menu.addItem(NSMenuItem(title: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w"))
    installMenu(menu)
    panel.orderFront(nil)
    panel.makeKey()

    XCTAssertTrue(NSApplication.shared.keyWindow === panel)
    XCTAssertTrue(panel.performKeyEquivalent(with: try event("w", .command)))
    XCTAssertEqual(requests, 1)
    XCTAssertFalse(panel.isVisible)
  }

  func testNonactivatingPanelRoutesAppCommandsToItsOwnMenu() throws {
    let menu = NSMenu()
    let target = CommandTarget()
    let combinations: [(String, NSEvent.ModifierFlags, String)] = [
      ("q", .command, "terminate:"), ("h", .command, "hide:"),
      ("m", .command, "performMiniaturize:"), (",", .command, "showPreferences:"),
      ("h", [.command, .option], "hideOtherApplications:"),
      ("f", [.command, .control], "toggleFullScreen:"),
    ]
    for (key, modifiers, action) in combinations {
      let item = NSMenuItem(title: key, action: NSSelectorFromString(action), keyEquivalent: key)
      item.keyEquivalentModifierMask = modifiers
      item.target = target
      menu.addItem(item)
    }
    installMenu(menu)
    let panel = QuickPastePanel(
      contentRect: .zero, styleMask: QuickPastePanel.presentationStyleMask,
      backing: .buffered, defer: true
    )
    panel.isReleasedWhenClosed = false
    defer { panel.close() }
    for (key, modifiers, _) in combinations {
      XCTAssertTrue(panel.performKeyEquivalent(with: try event(key, modifiers)), key)
    }
    XCTAssertEqual(target.commands, combinations.map { $0.0 })
    for (key, modifiers) in [("c", NSEvent.ModifierFlags.command), ("w", [.command, .shift]), ("\t", .command), ("`", .command)] {
      _ = panel.performKeyEquivalent(with: try event(key, modifiers))
    }
    XCTAssertEqual(target.commands.count, combinations.count)
  }

  func testNativeMenuContainsEveryApplicationShortcut() throws {
    let source = URL(fileURLWithPath: #filePath).resolvingSymlinksInPath()
      .deletingLastPathComponent().deletingLastPathComponent()
      .appendingPathComponent("Runner/Base.lproj/MainMenu.xib")
    let document = try XMLDocument(contentsOf: source)
    let commands: [(String, String, [String])] = [
      ("performClose:", "w", []), ("closeAllWindows:", "w", ["command", "option"]),
      ("terminate:", "q", []), ("performMiniaturize:", "m", []),
      ("hide:", "h", []), ("hideOtherApplications:", "h", ["command", "option"]),
      ("showPreferences:", ",", []), ("toggleFullScreen:", "f", ["command", "control"]),
      ("showFind:", "f", []),
    ]
    for (action, key, modifiers) in commands {
      let nodes = try document.nodes(forXPath: "//menuItem[connections/action/@selector='\(action)']")
      let item = try XCTUnwrap(nodes.first as? XMLElement, action)
      XCTAssertEqual(item.attribute(forName: "keyEquivalent")?.stringValue, key, action)
      let mask = item.elements(forName: "modifierMask").first
      for modifier in ["command", "control", "option", "shift"] {
        XCTAssertEqual(mask?.attribute(forName: modifier)?.stringValue == "YES", modifiers.contains(modifier), action)
      }
      // An absent mask means the AppKit default, Command.
      if modifiers.isEmpty { XCTAssertNil(mask, action) }
    }
  }
}

private final class CloseGate: NSObject, NSWindowDelegate {
  var requests = 0
  var allow = false
  func windowShouldClose(_ sender: NSWindow) -> Bool {
    requests += 1
    return allow
  }
}

private final class CommandTarget: NSObject {
  var commands: [String] = []
  @objc func terminate(_ sender: NSMenuItem) { commands.append(sender.keyEquivalent) }
  @objc func hide(_ sender: NSMenuItem) { commands.append(sender.keyEquivalent) }
  @objc func hideOtherApplications(_ sender: NSMenuItem) { commands.append(sender.keyEquivalent) }
  @objc func showPreferences(_ sender: NSMenuItem) { commands.append(sender.keyEquivalent) }
  @objc func performMiniaturize(_ sender: NSMenuItem) { commands.append(sender.keyEquivalent) }
  @objc func toggleFullScreen(_ sender: NSMenuItem) { commands.append(sender.keyEquivalent) }
}

private final class CloseAllTarget: NSObject {
  @objc func closeAllWindows(_ sender: Any?) {
    MacosWindowCommands.closeAll(windows: NSApplication.shared.windows, sender: sender)
  }
}
