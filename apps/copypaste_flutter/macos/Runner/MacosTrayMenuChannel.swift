import Cocoa
import FlutterMacOS

final class MacosTrayMenuChannel {
  typealias Activate = (NSRunningApplication?, NSRect) -> Void
  private let channel: FlutterMethodChannel
  private var binding: TrayQuickPasteBinding?
  private let activate: Activate

  init(binaryMessenger: FlutterBinaryMessenger, activate: @escaping Activate = { _, _ in }) {
    self.activate = activate
    channel = FlutterMethodChannel(
      name: "com.copypaste.app/tray_menu",
      binaryMessenger: binaryMessenger
    )
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else { result(nil); return }
      if call.method == "unbindQuickPaste" {
        self.binding?.detach()
        self.binding = nil
        result(nil)
        return
      }
      let key: String
      switch call.method {
      case "showImages": key = "nativeMenuAddress"
      case "bindQuickPaste": key = "nativeTrayAddress"
      default: result(FlutterMethodNotImplemented); return
      }
      guard
        let arguments = call.arguments as? [String: Any],
        let address = arguments[key] as? Int,
        address > 0,
        let pointer = UnsafeRawPointer(bitPattern: address)
      else {
        result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
        return
      }
      // Dart retains the nativeapi object through this acknowledgement.
      DispatchQueue.main.async {
        if call.method == "showImages" {
          let menu = Unmanaged<NSMenu>.fromOpaque(pointer).takeUnretainedValue()
          Self.showImages(in: menu)
        } else {
          let item = Unmanaged<NSStatusItem>.fromOpaque(pointer).takeUnretainedValue()
          guard let button = item.button else {
            result(FlutterError(code: "tray_unavailable", message: nil, details: nil))
            return
          }
          if Bundle.main.bundleIdentifier?.hasSuffix(".dev") == true {
            button.title = "Dev"
            button.imagePosition = .imageLeft
            button.toolTip = "CopyPaste Dev"
          }
          self.binding?.detach()
          self.binding = TrayQuickPasteBinding(button: button, activate: self.activate)
        }
        result(nil)
      }
    }
  }

  deinit {
    binding?.detach()
    channel.setMethodCallHandler(nil)
  }

  static func isPrimaryActivation(
    type: NSEvent.EventType, modifiers: NSEvent.ModifierFlags, clickCount: Int
  ) -> Bool {
    type == .leftMouseUp && !modifiers.contains(.control) && clickCount == 1
  }

  static func showImages(in menu: NSMenu) {
    // AppKit 27 hides menu images by default. Use its public property through
    // KVC so the application also compiles with SDKs predating macOS 27.
    let setter = NSSelectorFromString("setPreferredImageVisibility:")
    for item in menu.items where item.image != nil && item.responds(to: setter) {
      item.setValue(1, forKey: "preferredImageVisibility")
    }
  }
}

final class TrayQuickPasteBinding: NSObject {
  private weak var button: NSButton?
  private weak var originalTarget: AnyObject?
  private let originalAction: Selector?
  private let activate: MacosTrayMenuChannel.Activate
  private var active = true
  private var queued = false

  init(button: NSButton, activate: @escaping MacosTrayMenuChannel.Activate) {
    self.button = button
    originalTarget = button.target
    originalAction = button.action
    self.activate = activate
    super.init()
    button.target = self
    button.action = #selector(clicked(_:))
  }

  deinit { detach() }

  func detach() {
    active = false
    if let button, button.target === self {
      button.target = originalTarget
      button.action = originalAction
    }
  }

  @objc private func clicked(_ sender: Any?) {
    guard active, let button, let event = NSApp.currentEvent else { return }
    guard MacosTrayMenuChannel.isPrimaryActivation(
      type: event.type, modifiers: event.modifierFlags, clickCount: event.clickCount
    ) else {
      if let originalAction {
        NSApp.sendAction(originalAction, to: originalTarget, from: sender)
      }
      return
    }
    guard !queued, let window = button.window else { return }
    let anchor = window.convertToScreen(button.convert(button.bounds, to: nil))
    let foreground = NSWorkspace.shared.frontmostApplication
    queued = true
    // Leave AppKit's button tracking before starting Flutter. No Dart callback
    // or main-engine wakeup is needed between the click and the shared opener.
    DispatchQueue.main.async { [weak self] in
      guard let self, self.active else { return }
      self.queued = false
      self.activate(foreground, anchor)
    }
  }
}
