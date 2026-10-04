import Cocoa
import FlutterMacOS

private let runtimeLifecycleChannel = "com.copypaste.app/runtime_lifecycle"

@main
class AppDelegate: FlutterAppDelegate {
  private var terminationReplyPending = false
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
    return true
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
    guard !terminationReplyPending,
          let controller = mainFlutterWindow?.contentViewController as? FlutterViewController else {
      return terminationReplyPending ? .terminateLater : .terminateNow
    }

    terminationReplyPending = true
    let channel = FlutterMethodChannel(
      name: runtimeLifecycleChannel,
      binaryMessenger: controller.engine.binaryMessenger
    )
    channel.invokeMethod("prepareForTermination") { [weak self] (_: Any?) in
      DispatchQueue.main.async {
        guard self?.terminationReplyPending == true else {
          return
        }
        self?.terminationReplyPending = false
        sender.reply(toApplicationShouldTerminate: true)
      }
    }
    return .terminateLater
  }
}
