import Cocoa
import FlutterMacOS

private let windowMaterialChannel = "com.copypaste.app/window_material"
private let systemAccentChannel = "com.copypaste.app/system_accent"

/// Answers the system accent color and reports when the user changes it.
final class SystemAccentChannel {
  private let channel: FlutterMethodChannel
  private var observer: NSObjectProtocol?

  init(messenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(name: systemAccentChannel, binaryMessenger: messenger)
    channel.setMethodCallHandler { call, result in
      result(call.method == "read" ? Self.accent() : FlutterMethodNotImplemented)
    }
    observer = NotificationCenter.default.addObserver(
      forName: NSColor.systemColorsDidChangeNotification, object: nil, queue: .main
    ) { [weak self] _ in
      self?.channel.invokeMethod("changed", arguments: Self.accent())
    }
  }

  func detach() {
    channel.setMethodCallHandler(nil)
    if let observer { NotificationCenter.default.removeObserver(observer) }
    observer = nil
  }

  /// The accent as an opaque ARGB integer.
  private static func accent() -> Int? {
    guard let color = NSColor.controlAccentColor.usingColorSpace(.sRGB) else { return nil }
    func channel(_ value: CGFloat) -> Int { Int((min(max(value, 0), 1) * 255).rounded()) }
    return 0xFF00_0000 | channel(color.redComponent) << 16
      | channel(color.greenComponent) << 8 | channel(color.blueComponent)
  }
}

/// Paints the system material and never takes pointer events, wherever it
/// sits in the view hierarchy.
final class WindowMaterialBackdropView: NSView {
  override func hitTest(_ point: NSPoint) -> NSView? { nil }

  /// Puts the backdrop under every other subview of [host], filling it.
  func install(in host: NSView) {
    frame = host.bounds
    autoresizingMask = [.width, .height]
    host.addSubview(self, positioned: .below, relativeTo: nil)
  }
}

/// Shows the system window material behind a Flutter view.
///
/// The FlutterViewController stays the window's content controller; the
/// material goes to the bottom of its view, the wrapper Flutter provides for
/// AppKit subviews. It exists only while the glass preference is on and the
/// system does not reduce transparency, so an opaque window keeps no extra
/// layer or backdrop sampling.
final class WindowMaterialHost {
  enum Role {
    /// A standard window that is opaque without the material.
    case main
    /// An always-transparent panel with rounded corners.
    case panel
  }

  private weak var controller: FlutterViewController?
  private let role: Role
  private let opaqueContentBackground: NSColor?
  private var backdrop: WindowMaterialBackdropView?
  private var backdropMaterial = "none"
  private var enabled = false
  private var channel: FlutterMethodChannel?
  private var accent: SystemAccentChannel?
  private var accessibilityObserver: NSObjectProtocol?

  init(controller: FlutterViewController, role: Role) {
    self.controller = controller
    self.role = role
    opaqueContentBackground = controller.backgroundColor
  }

  /// Must run in the same main-thread turn that starts the engine's window,
  /// before Dart can send its first request.
  func attachChannel() {
    guard let controller else { return }
    let channel = FlutterMethodChannel(
      name: windowMaterialChannel, binaryMessenger: controller.engine.binaryMessenger)
    self.channel = channel
    accent = SystemAccentChannel(messenger: controller.engine.binaryMessenger)
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else {
        result(FlutterMethodNotImplemented)
        return
      }
      switch call.method {
      case "setEnabled":
        guard let enabled = (call.arguments as? [String: Any])?["enabled"] as? Bool else {
          result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
          return
        }
        self.enabled = enabled
        result(self.apply())
      case "setAppearance":
        guard let mode = (call.arguments as? [String: Any])?["mode"] as? String else {
          result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
          return
        }
        Self.applyAppearance(mode)
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    }
    accessibilityObserver = NSWorkspace.shared.notificationCenter.addObserver(
      forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
      object: nil,
      queue: .main
    ) { [weak self] _ in
      guard let self, let channel = self.channel else { return }
      channel.invokeMethod("stateChanged", arguments: self.apply())
    }
  }

  func detachChannel() {
    channel?.setMethodCallHandler(nil)
    channel = nil
    accent?.detach()
    accent = nil
    if let accessibilityObserver {
      NSWorkspace.shared.notificationCenter.removeObserver(accessibilityObserver)
    }
    accessibilityObserver = nil
    removeBackdrop()
  }

  private func apply() -> [String: Any] {
    let reduced = NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
    if enabled && !reduced {
      installBackdrop()
    } else {
      removeBackdrop()
    }
    return ["material": backdropMaterial, "supported": true, "reducedTransparency": reduced]
  }

  private func installBackdrop() {
    guard backdrop == nil, let controller else { return }
    let (material, name) = Self.makeMaterial(role: role)
    let backdrop = WindowMaterialBackdropView()
    material.frame = backdrop.bounds
    material.autoresizingMask = [.width, .height]
    backdrop.addSubview(material)
    backdrop.install(in: controller.view)
    self.backdrop = backdrop
    backdropMaterial = name
    if role == .main, let window = controller.view.window {
      window.isOpaque = false
      window.backgroundColor = .clear
      controller.backgroundColor = .clear
    }
  }

  private func removeBackdrop() {
    guard let backdrop else { return }
    backdrop.removeFromSuperview()
    self.backdrop = nil
    backdropMaterial = "none"
    if role == .main, let controller, let window = controller.view.window {
      window.isOpaque = true
      window.backgroundColor = .windowBackgroundColor
      controller.backgroundColor = opaqueContentBackground
    }
  }

  /// Every window of the application, panels included, shares one appearance.
  private static func applyAppearance(_ mode: String) {
    switch mode {
    case "light": NSApp.appearance = NSAppearance(named: .aqua)
    case "dark": NSApp.appearance = NSAppearance(named: .darkAqua)
    default: NSApp.appearance = nil
    }
  }

  private static func makeMaterial(role: Role) -> (NSView, String) {
    #if compiler(>=6.2)
    if #available(macOS 26.0, *) {
      let glass = NSGlassEffectView()
      glass.style = .regular
      glass.cornerRadius = role == .panel ? QuickPastePanel.cornerRadius : 0
      return (glass, "liquidGlass")
    }
    #endif
    let vibrancy = NSVisualEffectView()
    vibrancy.blendingMode = .behindWindow
    switch role {
    case .main:
      vibrancy.material = .underWindowBackground
      vibrancy.state = .followsWindowActiveState
    case .panel:
      vibrancy.material = .popover
      vibrancy.state = .active
    }
    return (vibrancy, "vibrancy")
  }
}
