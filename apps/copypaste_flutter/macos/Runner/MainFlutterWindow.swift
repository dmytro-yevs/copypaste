import Cocoa
import ApplicationServices
import Carbon
import FlutterMacOS

private let pairingPresentationHostChannel = "com.copypaste.app/pairing_presentation_host"
private let pairingPresentationContextChannel = "com.copypaste.app/pairing_presentation_context"
private let pairingLinksChannel = "com.copypaste.app/pairing_links"
private let protectedPairingRoutePrefix = "/protected-pairing/"
private let quickPasteHostChannel = "com.copypaste.app/quick_paste_host"
private let quickPasteContextChannel = "com.copypaste.app/quick_paste_context"

class MainFlutterWindow: NSWindow {
  static let unifiedTitlebarHeight: CGFloat = 48

  private var pairingPresentationMethodChannel: FlutterMethodChannel?
  private var pairingLinksMethodChannel: FlutterMethodChannel?
  private var quickPasteHostMethodChannel: FlutterMethodChannel?
  private var macosSetupChannel: MacosSetupChannel?
  private var protectedPresentation: ProtectedPairingPresentationWindow?
  private var quickPastePresentation: QuickPastePresentationWindow?
  private var pendingPairingURI: String?
  private var trafficLightLayoutObservers: [NSObjectProtocol] = []
  private var trafficLightLayoutScheduled = false

  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = frame
    contentViewController = flutterViewController
    setFrame(windowFrame, display: true)
    RegisterGeneratedPlugins(registry: flutterViewController)
    configurePairingPresentationChannel(flutterViewController)
    configurePairingLinksChannel(flutterViewController)
    configureQuickPasteHostChannel(flutterViewController)
    macosSetupChannel = MacosSetupChannel(
      binaryMessenger: flutterViewController.engine.binaryMessenger
    )
    super.awakeFromNib()
    installTrafficLightLayoutObservers()
    scheduleTrafficLightLayout()
  }

  deinit {
    trafficLightLayoutObservers.forEach {
      NotificationCenter.default.removeObserver($0)
    }
  }

  override func order(_ place: NSWindow.OrderingMode, relativeTo otherWin: Int) {
    super.order(place, relativeTo: otherWin)
    if place != .out {
      scheduleTrafficLightLayout()
    }
  }

  static func usesUnifiedTrafficLightLayout(styleMask: NSWindow.StyleMask) -> Bool {
    styleMask.contains(.fullSizeContentView) && !styleMask.contains(.fullScreen)
  }

  static func trafficLightButtonOriginY(
    headerHeight: CGFloat,
    buttonHeight: CGFloat
  ) -> CGFloat {
    max(0, (headerHeight - buttonHeight) / 2)
  }

  static func unifiedTitlebarContainerFrame(
    _ frame: NSRect,
    headerHeight: CGFloat
  ) -> NSRect {
    let height = max(0, headerHeight)
    return NSRect(
      x: frame.minX,
      y: frame.maxY - height,
      width: frame.width,
      height: height
    )
  }

  static func unifiedTitlebarViewFrame(
    containerBounds: NSRect,
    headerHeight: CGFloat
  ) -> NSRect {
    NSRect(
      x: containerBounds.minX,
      y: containerBounds.minY,
      width: containerBounds.width,
      height: headerHeight
    )
  }

  private func installTrafficLightLayoutObservers() {
    let notifications: [Notification.Name] = [
      NSWindow.didBecomeKeyNotification,
      NSWindow.didResizeNotification,
      NSWindow.didChangeBackingPropertiesNotification,
      NSWindow.didExitFullScreenNotification,
    ]
    trafficLightLayoutObservers = notifications.map { notification in
      NotificationCenter.default.addObserver(
        forName: notification,
        object: self,
        queue: .main
      ) { [weak self] _ in
        self?.scheduleTrafficLightLayout()
      }
    }
  }

  private func scheduleTrafficLightLayout() {
    guard Self.usesUnifiedTrafficLightLayout(styleMask: styleMask),
          !trafficLightLayoutScheduled else {
      return
    }
    trafficLightLayoutScheduled = true
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.trafficLightLayoutScheduled = false
      self.layoutTrafficLights()
    }
  }

  private func layoutTrafficLights() {
    guard Self.usesUnifiedTrafficLightLayout(styleMask: styleMask) else {
      return
    }
    let buttons = [
      standardWindowButton(.closeButton),
      standardWindowButton(.miniaturizeButton),
      standardWindowButton(.zoomButton),
    ].compactMap { $0 }
    guard let titlebarView = buttons.first?.superview,
          let titlebarContainer = titlebarView.superview,
          buttons.allSatisfy({ $0.superview === titlebarView }) else {
      return
    }

    let containerFrame = Self.unifiedTitlebarContainerFrame(
      titlebarContainer.frame,
      headerHeight: Self.unifiedTitlebarHeight
    )
    if titlebarContainer.frame != containerFrame {
      titlebarContainer.frame = containerFrame
    }
    let titlebarViewFrame = Self.unifiedTitlebarViewFrame(
      containerBounds: titlebarContainer.bounds,
      headerHeight: Self.unifiedTitlebarHeight
    )
    if titlebarView.frame != titlebarViewFrame {
      titlebarView.frame = titlebarViewFrame
    }

    for button in buttons {
      button.setFrameOrigin(NSPoint(
        x: button.frame.minX,
        y: Self.trafficLightButtonOriginY(
          headerHeight: Self.unifiedTitlebarHeight,
          buttonHeight: button.frame.height
        )
      ))
    }
  }

  private func configureQuickPasteHostChannel(_ controller: FlutterViewController) {
    let channel = FlutterMethodChannel(
      name: quickPasteHostChannel,
      binaryMessenger: controller.engine.binaryMessenger
    )
    quickPasteHostMethodChannel = channel
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else {
        result(FlutterError(code: "window_unavailable", message: nil, details: nil))
        return
      }
      switch call.method {
      case "isSupported":
        result(true)
      case "prepare":
        result(self.prepareQuickPaste())
      case "open":
        guard self.prepareQuickPaste(), let presentation = self.quickPastePresentation else {
          result(FlutterError(code: "window_unavailable", message: nil, details: nil))
          return
        }
        presentation.show()
        result(true)
      case "accessibilityGranted":
        result(MacosAccessibility.isTrusted(prompt: false))
      case "requestAccessibility":
        result(MacosAccessibility.isTrusted(prompt: true))
      case "dispose":
        self.quickPastePresentation?.shutdown()
        result(true)
      default:
        result(FlutterMethodNotImplemented)
      }
    }
  }

  private func prepareQuickPaste() -> Bool {
    if quickPastePresentation != nil { return true }
    guard let presentation = QuickPastePresentationWindow(
      mainWindow: self,
      onOpenSettings: { [weak self] in
        self?.quickPasteHostMethodChannel?.invokeMethod("openSettings", arguments: nil)
      },
      onClose: { [weak self] in self?.quickPastePresentation = nil }
    ) else {
      return false
    }
    quickPastePresentation = presentation
    return true
  }

  func receivePairingURL(_ url: URL) {
    guard url.scheme == "copypaste", url.host == "pair", url.path == "/v1" else {
      return
    }
    sharingType = .none
    pendingPairingURI = url.absoluteString
    makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    pairingLinksMethodChannel?.invokeMethod(
      "openPairingUri",
      arguments: url.absoluteString
    ) { [weak self] result in
      if result as? Bool == true && self?.pendingPairingURI == url.absoluteString {
        self?.pendingPairingURI = nil
      }
    }
  }

  private func configurePairingLinksChannel(_ controller: FlutterViewController) {
    let channel = FlutterMethodChannel(
      name: pairingLinksChannel,
      binaryMessenger: controller.engine.binaryMessenger
    )
    pairingLinksMethodChannel = channel
    channel.setMethodCallHandler { [weak self] call, result in
      guard call.method == "takePendingUri" else {
        result(FlutterMethodNotImplemented)
        return
      }
      let uri = self?.pendingPairingURI
      self?.pendingPairingURI = nil
      result(uri)
    }
  }

  private func configurePairingPresentationChannel(_ controller: FlutterViewController) {
    let channel = FlutterMethodChannel(name: pairingPresentationHostChannel, binaryMessenger: controller.engine.binaryMessenger)
    pairingPresentationMethodChannel = channel
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else { result(FlutterError(code: "window_unavailable", message: nil, details: nil)); return }
      switch call.method {
      case "isSupported": result(true)
      case "setCaptureProtection":
        guard let arguments = call.arguments as? [String: Any], let enabled = arguments["enabled"] as? Bool else {
          result(FlutterError(code: "invalid_arguments", message: nil, details: nil)); return
        }
        self.sharingType = enabled ? .none : .readOnly
        result(true)
      case "open":
        guard let arguments = call.arguments as? [String: Any], let ceremonyId = arguments["ceremonyId"] as? String, !ceremonyId.isEmpty else {
          result(FlutterError(code: "invalid_arguments", message: nil, details: nil)); return
        }
        guard self.protectedPresentation == nil else { result(FlutterError(code: "context_active", message: nil, details: nil)); return }
        let contextId = UUID().uuidString
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
          let generation = CPPairingBegin(ceremonyId, contextId)
          DispatchQueue.main.async { [weak self] in
            guard let self, generation != 0, self.protectedPresentation == nil else {
              result(FlutterError(code: "protected_context_unavailable", message: nil, details: nil)); return
            }
            let presentation = ProtectedPairingPresentationWindow(contextId: contextId, ceremonyId: ceremonyId, generation: generation) { [weak self] closedContextId in
              guard self?.protectedPresentation?.contextId == closedContextId else { return }
              self?.protectedPresentation = nil
            }
            self.protectedPresentation = presentation
            presentation.show()
            result(["contextId": contextId])
          }
        }
      case "close":
        guard let arguments = call.arguments as? [String: Any], let contextId = arguments["contextId"] as? String,
              let presentation = self.protectedPresentation, presentation.contextId == contextId else { result(false); return }
        presentation.requestClose(result: result)
      default: result(FlutterMethodNotImplemented)
      }
    }
  }
}

enum MacosAccessibility {
  static func isTrusted(prompt: Bool) -> Bool {
    let options = [
      kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: prompt
    ] as CFDictionary
    return AXIsProcessTrustedWithOptions(options)
  }

  static func paste() {
    let source = CGEventSource(stateID: .combinedSessionState)
    source?.setLocalEventsFilterDuringSuppressionState(
      [.permitLocalMouseEvents, .permitSystemDefinedEvents],
      state: .eventSuppressionStateSuppressionInterval
    )
    let keyDown = CGEvent(
      keyboardEventSource: source,
      virtualKey: CGKeyCode(kVK_ANSI_V),
      keyDown: true
    )
    let keyUp = CGEvent(
      keyboardEventSource: source,
      virtualKey: CGKeyCode(kVK_ANSI_V),
      keyDown: false
    )
    keyDown?.flags = .maskCommand
    keyUp?.flags = .maskCommand
    keyDown?.post(tap: .cgSessionEventTap)
    keyUp?.post(tap: .cgSessionEventTap)
  }
}

private final class QuickPastePanel: NSPanel {
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { false }
}

private final class QuickPastePresentationWindow: NSObject, NSWindowDelegate {
  private let mainWindow: NSWindow
  private let onOpenSettings: () -> Void
  private let onClose: () -> Void
  private let window: QuickPastePanel
  private let engine: FlutterEngine
  private let controller: FlutterViewController
  private var contextChannel: FlutterMethodChannel?
  private var previousApplication: NSRunningApplication?
  private var performingAction = false
  private var closed = false

  init?(
    mainWindow: NSWindow,
    onOpenSettings: @escaping () -> Void,
    onClose: @escaping () -> Void
  ) {
    self.mainWindow = mainWindow
    self.onOpenSettings = onOpenSettings
    self.onClose = onClose
    let project = FlutterDartProject()
    let engine = FlutterEngine(
      name: "quick-paste",
      project: project,
      allowHeadlessExecution: true
    )
    guard engine.run(withEntrypoint: "quickPasteMain") else { return nil }
    self.engine = engine
    controller = FlutterViewController(engine: engine, nibName: nil, bundle: nil)
    window = QuickPastePanel(
      contentRect: NSRect(x: 0, y: 0, width: 520, height: 720),
      styleMask: [.borderless, .resizable, .fullSizeContentView],
      backing: .buffered,
      defer: false
    )
    super.init()
    window.title = "CopyPaste Quick Paste"
    window.level = .floating
    window.hasShadow = true
    window.isOpaque = true
    window.isReleasedWhenClosed = false
    window.hidesOnDeactivate = true
    window.collectionBehavior = [.transient, .moveToActiveSpace, .fullScreenAuxiliary]
    window.delegate = self
    window.contentViewController = controller
    window.contentView?.wantsLayer = true
    window.contentView?.layer?.cornerRadius = 12
    window.contentView?.layer?.masksToBounds = true
    RegisterGeneratedPlugins(registry: controller)
    configureContextChannel()
  }

  func show() {
    let workspace = NSWorkspace.shared
    let frontmost = workspace.frontmostApplication
    if frontmost?.bundleIdentifier != Bundle.main.bundleIdentifier {
      previousApplication = frontmost
    }
    let cursor = NSEvent.mouseLocation
    let displays = NSScreen.screens.map {
      QuickPasteDisplay(frame: $0.frame, visibleFrame: $0.visibleFrame)
    }
    let frame = quickPasteFrame(cursor: cursor, size: window.frame.size, displays: displays)
    window.setFrame(frame, display: true)
    window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    contextChannel?.invokeMethod("opened", arguments: nil)
  }

  func shutdown() {
    window.close()
  }

  func windowDidResignKey(_ notification: Notification) {
    if !performingAction { window.orderOut(nil) }
  }

  func windowWillClose(_ notification: Notification) {
    close()
  }

  private func close() {
    guard !closed else { return }
    closed = true
    contextChannel?.setMethodCallHandler(nil)
    contextChannel = nil
    engine.shutDownEngine()
    onClose()
  }

  private func hide() {
    performingAction = true
    window.orderOut(nil)
    performingAction = false
  }

  private func showMainWindow(openSettings: Bool) {
    hide()
    mainWindow.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    if openSettings { onOpenSettings() }
  }

  private func configureContextChannel() {
    let channel = FlutterMethodChannel(
      name: quickPasteContextChannel,
      binaryMessenger: controller.engine.binaryMessenger
    )
    contextChannel = channel
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self, !self.closed else {
        result(FlutterError(code: "context_closed", message: nil, details: nil))
        return
      }
      switch call.method {
      case "accessibilityGranted":
        result(MacosAccessibility.isTrusted(prompt: false))
      case "requestAccessibility":
        result(MacosAccessibility.isTrusted(prompt: true))
      case "paste":
        guard MacosAccessibility.isTrusted(prompt: false) else {
          result(false)
          return
        }
        self.hide()
        self.previousApplication?.activate(options: [])
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
          MacosAccessibility.paste()
          result(true)
        }
      case "close":
        self.hide()
        result(true)
      case "openMain":
        self.showMainWindow(openSettings: false)
        result(true)
      case "openSettings":
        self.showMainWindow(openSettings: true)
        result(true)
      case "quit":
        result(true)
        NSApplication.shared.terminate(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    }
  }
}

private final class ProtectedPairingPresentationWindow: NSObject, NSWindowDelegate {
  let contextId: String
  private let ceremonyId: String
  private let generation: UInt64
  private let onClose: (String) -> Void
  private let window: NSWindow
  private let controller: FlutterViewController
  private var contextChannel: FlutterMethodChannel?
  private var closed = false
  private var closeApproved = false

  init(contextId: String, ceremonyId: String, generation: UInt64, onClose: @escaping (String) -> Void) {
    self.contextId = contextId
    self.ceremonyId = ceremonyId
    self.generation = generation
    self.onClose = onClose
    window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 520, height: 640), styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.sharingType = .none
    let project = FlutterDartProject()
    project.dartEntrypointArguments = ["--route=\(protectedPairingRoutePrefix)\(contextId)"]
    controller = FlutterViewController(project: project)
    super.init()
    window.title = "CopyPaste"
    window.isReleasedWhenClosed = false
    window.delegate = self
    window.contentViewController = controller
    RegisterGeneratedPlugins(registry: controller)
    configureContextChannel()
  }

  func show() {
    window.center()
    window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
  }

  func requestClose(result: FlutterResult? = nil) {
    guard !closed else { result?(true); return }
    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
      guard let self else { return }
      let active = CPPairingActive(self.contextId)
      let canClose = !active || CPPairingCancel(self.contextId)
      if !active { _ = CPPairingDetach(self.contextId) }
      DispatchQueue.main.async {
        guard !self.closed else { result?(true); return }
        guard canClose else { result?(FlutterError(code: "decision_busy", message: nil, details: nil)); return }
        self.closeApproved = true
        result?(true)
        self.window.close()
      }
    }
  }

  func windowShouldClose(_ sender: NSWindow) -> Bool {
    if closed || closeApproved { return true }
    requestClose()
    return false
  }

  func windowWillClose(_ notification: Notification) {
    guard !closed else { return }
    closed = true
    contextChannel?.setMethodCallHandler(nil)
    contextChannel = nil
    DispatchQueue.global(qos: .utility).async { [contextId] in _ = CPPairingDetach(contextId) }
    onClose(contextId)
  }

  private func configureContextChannel() {
    let channel = FlutterMethodChannel(name: pairingPresentationContextChannel, binaryMessenger: controller.engine.binaryMessenger)
    contextChannel = channel
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self, !self.closed else { result(FlutterError(code: "context_closed", message: nil, details: nil)); return }
      switch call.method {
      case "isContextActive": self.performNative(result) { CPPairingActive(self.contextId) }
      case "ceremonyId": self.performNative(result) { CPPairingActive(self.contextId) ? self.ceremonyId : nil }
      case "status": self.performNative(result) {
        var state: UInt32 = 0; var expiresInMs: UInt64 = 0
        guard CPPairingStatus(self.contextId, self.generation, &state, &expiresInMs) else { return nil }
        return ["state": state, "expiresInMs": expiresInMs]
      }
      case "revealQr": self.performNative(result) {
        guard let data = CPPairingRevealQr(self.ceremonyId, self.contextId, self.generation) else { return nil }
        return FlutterStandardTypedData(bytes: data)
      }
      case "revealSas": self.performNative(result) {
        guard let data = CPPairingRevealSas(self.contextId, self.generation) else { return nil }
        return FlutterStandardTypedData(bytes: data)
      }
      case "joinManual":
        guard let arguments = call.arguments as? [String: Any], let code = arguments["code"] as? String,
              let address = arguments["address"] as? String, !code.isEmpty, !address.isEmpty else { result(FlutterError(code: "invalid_arguments", message: nil, details: nil)); return }
        self.performNative(result) { CPPairingJoin(self.contextId, self.generation, code, address) }
      case "joinQr":
        guard let arguments = call.arguments as? [String: Any], let uri = arguments["uri"] as? String,
              !uri.isEmpty else { result(FlutterError(code: "invalid_arguments", message: nil, details: nil)); return }
        self.performNative(result) { CPPairingJoinURI(self.contextId, self.generation, uri) }
      case "confirm":
        guard let arguments = call.arguments as? [String: Any], let sas = arguments["sas"] as? String,
              let accept = arguments["accept"] as? Bool, !sas.isEmpty else { result(FlutterError(code: "invalid_arguments", message: nil, details: nil)); return }
        self.performNative(result) { CPPairingDecide(self.contextId, self.generation, sas, accept) }
      case "cancel": self.performNative(result) { CPPairingCancel(self.contextId) }
      case "closeContext": self.requestClose(result: result)
      default: result(FlutterMethodNotImplemented)
      }
    }
  }

  private func performNative(_ result: @escaping FlutterResult, operation: @escaping () -> Any?) {
    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
      let output = operation()
      DispatchQueue.main.async {
        guard let self, !self.closed else { result(FlutterError(code: "context_closed", message: nil, details: nil)); return }
        guard let output else { result(FlutterError(code: "operation_rejected", message: nil, details: nil)); return }
        result(output)
      }
    }
  }
}
