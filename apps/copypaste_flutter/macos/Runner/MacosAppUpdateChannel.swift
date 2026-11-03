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
  private var installingBundle = false

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
        if Self.homebrewInstalled() {
          self.complete(result, value: [
            "available": true,
            "installationType": "homebrew",
          ])
          return
        }
        guard let updater = MacosBundleUpdater.current() else {
          self.complete(result, value: [
            "available": false,
            "reason": "This macOS installation cannot update automatically.",
          ])
          return
        }
        if let problem = updater.locationProblem() {
          self.complete(result, value: [
            "available": false,
            "reason": problem.reason,
          ])
          return
        }
        self.complete(result, value: [
          "available": true,
          "installationType": "bundle",
        ])
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
    if let path = arguments["path"] as? String {
      installBundle(
        image: URL(fileURLWithPath: path),
        sha256: arguments["sha256"] as? String,
        version: expectedVersion,
        result: result
      )
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
      if Self.installUpdate(using: brew, expectedVersion: expectedVersion) {
        self.complete(result, value: "restart_required")
      } else {
        self.complete(
          result,
          error: FlutterError(code: "homebrew_update_failed", message: nil, details: nil)
        )
      }
    }
  }

  private func installBundle(
    image: URL,
    sha256: String?,
    version: String,
    result: @escaping FlutterResult
  ) {
    guard !installingBundle, let sha256, image.path.hasPrefix("/") else {
      result(FlutterError(
        code: installingBundle ? "update_busy" : "invalid_arguments",
        message: nil,
        details: nil
      ))
      return
    }
    installingBundle = true
    workQueue.async {
      let failure: MacosBundleUpdateError?
      // Homebrew must keep ownership of the bundles it installed.
      if Self.homebrewInstalled() {
        failure = .invalidArguments
      } else if let updater = MacosBundleUpdater.current() {
        do {
          try updater.install(image: image, sha256: sha256, version: version)
          failure = nil
        } catch let error as MacosBundleUpdateError {
          failure = error
        } catch {
          failure = .failed
        }
      } else {
        failure = .failed
      }
      DispatchQueue.main.async {
        self.installingBundle = false
        result(failure.map {
          FlutterError(code: $0.code, message: nil, details: nil)
        } ?? "restart_required")
      }
    }
  }

  private static func homebrewInstalled() -> Bool {
    guard let brew = brewExecutable() else { return false }
    return run(
      brew,
      arguments: ["list", "--cask", "--versions", copyPasteCaskToken],
      timeout: 30
    )
  }

  static func installUpdate(using brew: URL, expectedVersion: String) -> Bool {
    let updated = Self.run(
      brew,
      arguments: ["update-if-needed"],
      timeout: 180
    )
    // A named `brew outdated` returns 1 when an update is available.
    // Let upgrade determine whether installation is needed instead.
    let installed = updated && Self.run(
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
    return installedVersion.0 && installedVersion.1
      .split(whereSeparator: { $0.isWhitespace })
      .contains(Substring(expectedVersion))
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

  static func run(
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

  static func runCapturing(
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
