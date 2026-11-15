import AppKit

/// One local policy governs all application engines, including pairing and Quick Paste.
final class MacosScreenshotProtection {
  static let shared = MacosScreenshotProtection()
  private static let preferenceKey = "security.blockScreenshots"

  private final class Client {
    weak var window: NSWindow?
    let layer: CALayer
    var protection: CaptureProtectedLayerTree?
    init(window: NSWindow, layer: CALayer) { self.window = window; self.layer = layer }
    func apply(_ blocked: Bool) -> Bool {
      if blocked {
        if protection == nil { protection = CaptureProtectedLayerTree(layer: layer) }
        if window?.isVisible == true { protection?.resume() }
        else { protection?.suspend() }
        return protection?.healthy == true
      }
      protection?.detach()
      protection = nil
      return true
    }

    func suspend() { protection?.suspend() }
    func resume() { protection?.resume() }
  }

  private var clients: [Client] = []
  private var closeObserver: NSObjectProtocol?
  private(set) var blocked = UserDefaults.standard.bool(forKey: preferenceKey)

  private init() {
    closeObserver = NotificationCenter.default.addObserver(
      forName: NSWindow.willCloseNotification, object: nil, queue: .main
    ) { [weak self] notification in
      // Do not restore capture during a close animation. Retained windows can
      // reopen and must keep following the policy until they are deallocated.
      if let window = notification.object as? NSWindow {
        self?.clients.first(where: { $0.window === window })?.suspend()
      }
      DispatchQueue.main.async { [weak self] in self?.pruneClients() }
    }
  }

  @discardableResult
  func register(window: NSWindow, view: NSView) -> Bool {
    pruneClients()
    window.sharingType = .readOnly
    if let client = clients.first(where: { $0.window === window }) { return client.apply(blocked) }
    view.wantsLayer = true
    guard let layer = view.layer else { return false }
    let client = Client(window: window, layer: layer)
    clients.append(client)
    return client.apply(blocked)
  }

  func applyCurrentPolicy() -> Bool {
    pruneClients()
    return clients.allSatisfy { $0.apply(blocked) }
  }

  func resume(window: NSWindow) {
    clients.first(where: { $0.window === window })?.resume()
  }

  func suspend(window: NSWindow) {
    clients.first(where: { $0.window === window })?.suspend()
  }

  func setBlocked(_ value: Bool) -> Bool {
    pruneClients()
    let before = blocked
    for client in clients {
      if !client.apply(value) {
        for restore in clients { _ = restore.apply(before) }
        return false
      }
    }
    blocked = value
    UserDefaults.standard.set(value, forKey: Self.preferenceKey)
    return true
  }

  private func pruneClients() {
    clients.removeAll { client in
      guard client.window == nil else { return false }
      client.protection?.detach()
      return true
    }
  }
}
