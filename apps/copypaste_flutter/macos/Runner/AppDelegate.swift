import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var pendingPairingURLs: [URL] = []

  override func applicationDidFinishLaunching(_ notification: Notification) {
    super.applicationDidFinishLaunching(notification)
    drainPairingURLs()
  }

  override func application(_ application: NSApplication, open urls: [URL]) {
    pendingPairingURLs.append(contentsOf: urls)
    drainPairingURLs()
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return false
  }

  override func applicationShouldHandleReopen(
    _ sender: NSApplication,
    hasVisibleWindows flag: Bool
  ) -> Bool {
    guard !flag, let window = mainFlutterWindow else {
      return false
    }

    window.makeKeyAndOrderFront(self)
    sender.activate(ignoringOtherApps: true)
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }

  private func drainPairingURLs() {
    guard let window = mainFlutterWindow as? MainFlutterWindow else {
      return
    }
    let urls = pendingPairingURLs
    pendingPairingURLs.removeAll()
    for url in urls {
      window.receivePairingURL(url)
    }
  }

  override func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
    return .terminateNow
  }
}
