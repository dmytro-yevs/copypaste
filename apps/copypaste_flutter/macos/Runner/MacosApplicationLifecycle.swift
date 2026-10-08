import Cocoa
import FlutterMacOS

/// Relaunches the installed bundle after the current app process has exited.
final class MacosApplicationLifecycle {
  private let channel: FlutterMethodChannel
  private var restarting = false

  init(binaryMessenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(
      name: "com.copypaste.app/lifecycle", binaryMessenger: binaryMessenger
    )
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else { return }
      guard call.method == "restart" else {
        result(FlutterMethodNotImplemented)
        return
      }
      guard !self.restarting else {
        result(FlutterError(code: "restart_busy", message: nil, details: nil))
        return
      }
      do {
        _ = try Self.scheduleRelaunch(
          applicationURL: Bundle.main.bundleURL,
          processID: ProcessInfo.processInfo.processIdentifier
        )
        self.restarting = true
        result(nil)
        // Committed macOS exit closes the daemon's app-parent pipe. Do not
        // dismantle Flutter state before native termination is committed.
        DispatchQueue.main.async { NSApplication.shared.terminate(nil) }
      } catch {
        result(FlutterError(code: "restart_failed", message: nil, details: nil))
      }
    }
  }

  deinit { channel.setMethodCallHandler(nil) }

  @discardableResult
  static func scheduleRelaunch(
    applicationURL: URL,
    processID: Int32,
    opener: URL = URL(fileURLWithPath: "/usr/bin/open")
  ) throws -> Process {
    guard processID > 0, applicationURL.isFileURL,
          FileManager.default.fileExists(atPath: applicationURL.path),
          FileManager.default.isExecutableFile(atPath: opener.path) else {
      throw CocoaError(.fileNoSuchFile)
    }
    let helper = Process()
    helper.executableURL = URL(fileURLWithPath: "/bin/sh")
    // Arguments stay positional so bundle paths cannot be interpreted as code.
    helper.arguments = [
      "-c",
      "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.1; done; exec \"$3\" -n -a \"$2\"",
      "copypaste-relaunch", String(processID), applicationURL.path, opener.path,
    ]
    helper.currentDirectoryURL = URL(fileURLWithPath: "/")
    helper.standardInput = FileHandle.nullDevice
    helper.standardOutput = FileHandle.nullDevice
    helper.standardError = FileHandle.nullDevice
    try helper.run()
    return helper
  }
}
