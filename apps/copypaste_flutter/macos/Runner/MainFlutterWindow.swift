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
  private var securityChannel: FlutterMethodChannel?
  private var appUpdateChannel: MacosAppUpdateChannel?
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
    MacosScreenshotProtection.shared.register(window: self, view: flutterViewController.view)
    let security = FlutterMethodChannel(
      name: "com.copypaste.app/security", binaryMessenger: flutterViewController.engine.binaryMessenger)
    securityChannel = security
    security.setMethodCallHandler { call, result in
      switch call.method {
      case "getBlockScreenshots": result(MacosScreenshotProtection.shared.blocked)
      case "setBlockScreenshots":
        guard let args = call.arguments as? [String: Any], let enabled = args["enabled"] as? Bool else {
          result(FlutterError(code: "invalid_arguments", message: nil, details: nil)); return
        }
        result(MacosScreenshotProtection.shared.setBlocked(enabled))
      default: result(FlutterMethodNotImplemented)
      }
    }
    appUpdateChannel = MacosAppUpdateChannel(
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
        let frontmost = NSWorkspace.shared.frontmostApplication
        guard self.prepareQuickPaste(), let presentation = self.quickPastePresentation else {
          result(FlutterError(code: "window_unavailable", message: nil, details: nil))
          return
        }
        result(presentation.show(frontmost: frontmost))
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
        guard let arguments = call.arguments as? [String: Any], arguments["enabled"] is Bool else {
          result(FlutterError(code: "invalid_arguments", message: nil, details: nil)); return
        }
        result(MacosScreenshotProtection.shared.applyCurrentPolicy())
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

  // Construct the entire chord before any event is posted.
  static func preparePaste(
    source: () -> CGEventSource? = { CGEventSource(stateID: .combinedSessionState) },
    event: (CGEventSource, Bool) -> CGEvent? = {
      CGEvent(keyboardEventSource: $0, virtualKey: CGKeyCode(kVK_ANSI_V), keyDown: $1)
    },
    post: @escaping (CGEvent) -> Void = { $0.post(tap: .cgSessionEventTap) }
  ) -> (() -> Void)? {
    guard let source = source(), let keyDown = event(source, true),
          let keyUp = event(source, false) else { return nil }
    source.setLocalEventsFilterDuringSuppressionState(
      [.permitLocalMouseEvents, .permitSystemDefinedEvents],
      state: .eventSuppressionStateSuppressionInterval
    )
    keyDown.flags = .maskCommand
    keyUp.flags = .maskCommand
    return { post(keyDown); post(keyUp) }
  }

}

final class QuickPastePanel: NSPanel {
  static let presentationStyleMask: NSWindow.StyleMask = [
    .borderless,
    .resizable,
    .fullSizeContentView,
    .nonactivatingPanel,
  ]

  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { false }
}

protocol QuickPasteApplicationTarget: AnyObject {
  var processIdentifier: pid_t { get }
  var isTerminated: Bool { get }
  func activateForQuickPaste() -> Bool
}

extension NSRunningApplication: QuickPasteApplicationTarget {
  func activateForQuickPaste() -> Bool { activate(options: []) }
}

// One native presentation owns one external application and at most one handoff.
final class QuickPastePasteSession {
  private final class Pending {
    let id: Int64
    let target: QuickPasteApplicationTarget
    let result: (Bool) -> Void
    var cancel: (() -> Void)?

    init(id: Int64, target: QuickPasteApplicationTarget, result: @escaping (Bool) -> Void) {
      self.id = id
      self.target = target
      self.result = result
    }
  }

  private let ownPID: pid_t
  private let foreground: () -> pid_t?
  private let trusted: () -> Bool
  private let prepareInput: () -> (() -> Void)?
  private let schedule: (@escaping () -> Void) -> (() -> Void)
  private var counter: Int64 = 0
  private var pending: Pending?
  private(set) var id: Int64 = 0
  private(set) var target: QuickPasteApplicationTarget?
  var hasPendingPaste: Bool { pending != nil }

  init(
    ownPID: pid_t = ProcessInfo.processInfo.processIdentifier,
    foreground: @escaping () -> pid_t? = { NSWorkspace.shared.frontmostApplication?.processIdentifier },
    trusted: @escaping () -> Bool = { MacosAccessibility.isTrusted(prompt: false) },
    prepareInput: @escaping () -> (() -> Void)? = { MacosAccessibility.preparePaste() },
    schedule: @escaping (@escaping () -> Void) -> (() -> Void) = { continuation in
      let item = DispatchWorkItem(block: continuation)
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.08, execute: item)
      return { item.cancel() }
    }
  ) {
    self.ownPID = ownPID
    self.foreground = foreground
    self.trusted = trusted
    self.prepareInput = prepareInput
    self.schedule = schedule
  }

  deinit { invalidate() }

  func isValid(_ target: QuickPasteApplicationTarget?) -> Bool {
    guard let target else { return false }
    return !target.isTerminated && target.processIdentifier > 0 && target.processIdentifier != ownPID
  }

  func begin(frontmost: QuickPasteApplicationTarget?, popupActive: Bool) -> Int64? {
    let retained = target
    let preserve = popupActive && id > 0 && isValid(retained) &&
      (frontmost?.processIdentifier == ownPID || frontmost?.processIdentifier == retained?.processIdentifier)
    invalidate()
    guard counter < Int64.max else { return nil }
    counter += 1
    id = counter
    target = preserve ? retained : (isValid(frontmost) ? frontmost : nil)
    return id
  }

  func invalidate() {
    id = 0
    target = nil
    let action = pending
    pending = nil
    action?.cancel?()
    action?.result(false)
  }

  func matches(_ requestedID: Int64) -> Bool { requestedID > 0 && requestedID == id }

  func paste(id requestedID: Int64, hide: () -> Void, result: @escaping (Bool) -> Void) {
    guard matches(requestedID), pending == nil, let target, isValid(target), trusted() else {
      result(false)
      return
    }
    let action = Pending(id: requestedID, target: target, result: result)
    pending = action
    hide()
    guard pending === action, matches(requestedID), isValid(target) else { finish(action, false); return }
    if foreground() != target.processIdentifier && !target.activateForQuickPaste() {
      finish(action, false)
      return
    }
    guard pending === action, matches(requestedID), isValid(target) else { finish(action, false); return }
    action.cancel = schedule { [weak self, weak action] in
      guard let self, let action, self.pending === action else { return }
      guard let submit = self.prepareInput(), self.pending === action,
            self.matches(action.id), self.isValid(action.target), self.trusted(),
            self.foreground() == action.target.processIdentifier,
            self.pending === action, self.matches(action.id) else {
        self.finish(action, false)
        return
      }
      // Public APIs cannot atomically compare focus and submit input.
      self.pending = nil
      self.id = 0
      self.target = nil
      action.cancel = nil
      submit()
      action.result(true)
    }
  }

  private func finish(_ action: Pending, _ success: Bool) {
    guard pending === action else { return }
    pending = nil
    id = 0
    target = nil
    action.cancel?()
    action.cancel = nil
    action.result(success)
  }
}

// Keep rollback and each native presentation stage owned by the captured ID.
func presentQuickPaste(
  id: Int64,
  session: QuickPastePasteSession,
  position: () -> Bool,
  show: () -> Bool,
  hide: () -> Void,
  opened: () -> Void
) -> Bool {
  let owns = { session.matches(id) }
  func fail() -> Bool {
    if owns() {
      session.invalidate()
      if session.id == 0 { hide() }
    }
    return false
  }
  guard owns() else { return false }
  guard position(), owns() else { return fail() }
  guard show(), owns() else { return fail() }
  opened()
  return true
}

private final class QuickPastePresentationWindow: NSObject, NSWindowDelegate {
  private let mainWindow: NSWindow
  private let onOpenSettings: () -> Void
  private let onClose: () -> Void
  private let window: QuickPastePanel
  private let engine: FlutterEngine
  private let controller: FlutterViewController
  private var contextChannel: FlutterMethodChannel?
  private let pasteSession = QuickPastePasteSession()
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
      styleMask: QuickPastePanel.presentationStyleMask,
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
    MacosScreenshotProtection.shared.register(window: window, view: controller.view)
    window.contentView?.wantsLayer = true
    window.contentView?.layer?.cornerRadius = 12
    window.contentView?.layer?.masksToBounds = true
    RegisterGeneratedPlugins(registry: controller)
    configureContextChannel()
  }

  deinit { pasteSession.invalidate() }

  func show(frontmost: NSRunningApplication?) -> Bool {
    guard !closed else { return false }
    guard let id = pasteSession.begin(
      frontmost: frontmost,
      popupActive: window.isVisible && window.isKeyWindow
    ) else {
      if pasteSession.id == 0 { hide() }
      return false
    }
    return presentQuickPaste(
      id: id,
      session: pasteSession,
      position: {
        let cursor = NSEvent.mouseLocation
        let displays = NSScreen.screens.map {
          QuickPasteDisplay(frame: $0.frame, visibleFrame: $0.visibleFrame)
        }
        let frame = quickPasteFrame(cursor: cursor, size: self.window.frame.size, displays: displays)
        self.window.setFrame(frame, display: true)
        return true
      },
      show: { self.window.makeKeyAndOrderFront(nil); return true },
      hide: { self.hide() },
      opened: { self.contextChannel?.invokeMethod("opened", arguments: ["presentationId": id]) }
    )
  }

  func shutdown() {
    pasteSession.invalidate()
    window.close()
  }

  func windowDidResignKey(_ notification: Notification) {
    guard !performingAction else { return }
    if pasteSession.hasPendingPaste && !window.isVisible { return }
    pasteSession.invalidate()
    window.orderOut(nil)
  }

  func windowWillClose(_ notification: Notification) {
    close()
  }

  private func close() {
    guard !closed else { return }
    closed = true
    pasteSession.invalidate()
    contextChannel?.setMethodCallHandler(nil)
    contextChannel = nil
    engine.shutDownEngine()
    onClose()
  }

  private func hide() {
    let wasPerformingAction = performingAction
    performingAction = true
    window.orderOut(nil)
    performingAction = wasPerformingAction
  }

  private func showMainWindow(openSettings: Bool) {
    pasteSession.invalidate()
    hide()
    mainWindow.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    if openSettings { onOpenSettings() }
  }

  private static func presentationID(_ arguments: Any?) -> Int64? {
    guard let arguments = arguments as? [String: Any],
          let value = arguments["presentationId"] as? NSNumber,
          CFGetTypeID(value) != CFBooleanGetTypeID(),
          String(cString: value.objCType) != "d", String(cString: value.objCType) != "f",
          value.int64Value > 0 else { return nil }
    return value.int64Value
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
        guard let id = Self.presentationID(call.arguments) else { result(false); return }
        self.pasteSession.paste(id: id, hide: { self.hide() }, result: { result($0) })
      case "close":
        if let id = Self.presentationID(call.arguments), self.pasteSession.matches(id) {
          self.pasteSession.invalidate()
          self.hide()
        }
        result(true)
      case "openMain":
        self.showMainWindow(openSettings: false)
        result(true)
      case "openSettings":
        self.showMainWindow(openSettings: true)
        result(true)
      case "quit":
        self.pasteSession.invalidate()
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
    let project = FlutterDartProject()
    project.dartEntrypointArguments = ["--route=\(protectedPairingRoutePrefix)\(contextId)"]
    controller = FlutterViewController(project: project)
    super.init()
    window.title = "CopyPaste"
    window.isReleasedWhenClosed = false
    window.delegate = self
    window.contentViewController = controller
    MacosScreenshotProtection.shared.register(window: window, view: controller.view)
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
