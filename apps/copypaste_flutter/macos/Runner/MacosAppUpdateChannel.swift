import Cocoa
import FlutterMacOS

private let appUpdateChannelName = "com.copypaste.app/app_update"
private let copyPasteCaskToken = "copypaste"
private let copyPasteCask = "dmytro-yevs/copypaste/\(copyPasteCaskToken)"

final class MacosAppUpdateChannel {
  private let channel: FlutterMethodChannel
  private let workQueue = DispatchQueue(
    label: "com.copypaste.app.update",
    qos: .userInitiated
  )

  init(binaryMessenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(
      name: appUpdateChannelName,
      binaryMessenger: binaryMessenger
    )
    channel.setMethodCallHandler(handle)
  }

  deinit {
    channel.setMethodCallHandler(nil)
  }

  private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "currentVersion":
      result(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String)
    case "systemVersion":
      let version = ProcessInfo.processInfo.operatingSystemVersion
      result("\(version.majorVersion).\(version.minorVersion).\(version.patchVersion)")
    case "availability":
      workQueue.async {
        guard let brew = Self.brewExecutable() else {
          self.complete(result, value: [
            "available": false,
            "reason": "Install CopyPaste with Homebrew to update it here.",
          ])
          return
        }
        let installed = Self.run(
          brew,
          arguments: ["list", "--cask", "--versions", copyPasteCaskToken],
          timeout: 30
        )
        let availability: [String: Any] = installed
          ? ["available": true]
          : [
              "available": false,
              "reason": "Install CopyPaste with Homebrew to update it here.",
            ]
        self.complete(result, value: availability)
      }
    case "install":
      install(call, result: result)
    case "openReleasePage":
      openReleasePage(call, result: result)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  private func install(
    _ call: FlutterMethodCall,
    result: @escaping FlutterResult
  ) {
    guard
      let arguments = call.arguments as? [String: Any],
      let expectedVersion = arguments["version"] as? String,
      expectedVersion.range(
        of: #"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"#,
        options: .regularExpression
      ) != nil
    else {
      result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
      return
    }
    workQueue.async {
      guard let brew = Self.brewExecutable() else {
        self.complete(
          result,
          error: FlutterError(code: "homebrew_unavailable", message: nil, details: nil)
        )
        return
      }
      let updated = Self.run(
        brew,
        arguments: ["update-if-needed"],
        timeout: 180
      )
      let inspected = updated && Self.run(
        brew,
        arguments: ["outdated", "--cask", "--json=v2", copyPasteCask],
        timeout: 60
      )
      let installed = inspected && Self.run(
        brew,
        arguments: [
          "upgrade",
          "--cask",
          "--no-ask",
          "--no-quit",
          "--require-sha",
          copyPasteCask,
        ],
        timeout: 600
      )
      let installedVersion = installed
        ? Self.runCapturing(
            brew,
            arguments: ["list", "--cask", "--versions", copyPasteCaskToken],
            timeout: 30
          )
        : (false, "")
      if installedVersion.0 && installedVersion.1
        .split(whereSeparator: { $0.isWhitespace })
        .contains(Substring(expectedVersion)) {
        self.complete(result, value: "restart_required")
      } else {
        self.complete(
          result,
          error: FlutterError(code: "homebrew_update_failed", message: nil, details: nil)
        )
      }
    }
  }

  private func openReleasePage(
    _ call: FlutterMethodCall,
    result: @escaping FlutterResult
  ) {
    guard
      let arguments = call.arguments as? [String: Any],
      let value = arguments["url"] as? String,
      let url = URL(string: value),
      url.scheme == "https",
      url.host == "github.com",
      url.path.hasPrefix("/dmytro-yevs/copypaste/releases/")
    else {
      result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
      return
    }
    if NSWorkspace.shared.open(url) {
      result(nil)
    } else {
      result(FlutterError(code: "open_failed", message: nil, details: nil))
    }
  }

  private func complete(
    _ result: @escaping FlutterResult,
    value: Any? = nil,
    error: FlutterError? = nil
  ) {
    DispatchQueue.main.async {
      result(error ?? value)
    }
  }

  private static func brewExecutable() -> URL? {
    for path in ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"] {
      if FileManager.default.isExecutableFile(atPath: path) {
        return URL(fileURLWithPath: path)
      }
    }
    return nil
  }

  private static func run(
    _ executable: URL,
    arguments: [String],
    timeout: TimeInterval
  ) -> Bool {
    let process = Process()
    process.executableURL = executable
    process.arguments = arguments
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    var environment = ProcessInfo.processInfo.environment
    environment["HOMEBREW_NO_ENV_HINTS"] = "1"
    process.environment = environment
    let finished = DispatchSemaphore(value: 0)
    process.terminationHandler = { _ in finished.signal() }
    do {
      try process.run()
    } catch {
      return false
    }
    if finished.wait(timeout: .now() + timeout) == .timedOut {
      process.terminate()
      _ = finished.wait(timeout: .now() + 5)
      return false
    }
    return process.terminationStatus == 0
  }

  private static func runCapturing(
    _ executable: URL,
    arguments: [String],
    timeout: TimeInterval
  ) -> (Bool, String) {
    let outputURL = FileManager.default.temporaryDirectory
      .appendingPathComponent("copypaste-update-\(UUID().uuidString).log")
    guard FileManager.default.createFile(
      atPath: outputURL.path,
      contents: nil
    ), let output = try? FileHandle(forWritingTo: outputURL) else {
      return (false, "")
    }
    defer {
      try? output.close()
      try? FileManager.default.removeItem(at: outputURL)
    }
    let process = Process()
    process.executableURL = executable
    process.arguments = arguments
    process.standardOutput = output
    process.standardError = FileHandle.nullDevice
    var environment = ProcessInfo.processInfo.environment
    environment["HOMEBREW_NO_ENV_HINTS"] = "1"
    process.environment = environment
    let finished = DispatchSemaphore(value: 0)
    process.terminationHandler = { _ in finished.signal() }
    do {
      try process.run()
    } catch {
      return (false, "")
    }
    if finished.wait(timeout: .now() + timeout) == .timedOut {
      process.terminate()
      _ = finished.wait(timeout: .now() + 5)
      return (false, "")
    }
    try? output.synchronize()
    guard process.terminationStatus == 0,
          let attributes = try? FileManager.default.attributesOfItem(
            atPath: outputURL.path
          ),
          let size = attributes[.size] as? NSNumber,
          size.intValue <= 64 * 1024,
          let data = try? Data(contentsOf: outputURL),
          let value = String(data: data, encoding: .utf8) else {
      return (false, "")
    }
    return (true, value)
  }
}
